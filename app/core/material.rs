//! Order-bound material pegging. Allocates existing supply; never creates orders.
use crate::{compile, domain, model::*};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

const PREFIX: &str = "@apex/pegging/";
const EPS: f64 = 1e-8;

#[derive(Clone, Debug, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaterialPolicy {
    ReallocateRoutes,
}

#[derive(Default, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchOptions {
    #[serde(default)]
    pub route_choices: BTreeMap<String, String>,
    #[serde(default)]
    pub mode_choices: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, JsonSchema, Serialize, Deserialize)]
pub struct Allocation {
    pub consumer: String,
    pub item: String,
    pub quantity: f64,
    pub source: String,
    pub source_task: Option<String>,
    pub receipt_index: Option<usize>,
    pub available_at: Option<Time>,
    pub token: String,
}

#[derive(Clone, Debug, Default, PartialEq, JsonSchema, Serialize, Deserialize)]
pub struct DispatchReport {
    #[serde(default)]
    pub preview: bool,
    #[serde(default)]
    pub preview_diagnostics: Vec<String>,
    pub policy: String,
    pub route_choices: BTreeMap<String, String>,
    pub mode_choices: BTreeMap<String, String>,
    pub allocations: Vec<Allocation>,
    pub added_dependencies: usize,
    pub started_inputs_already_consumed: Vec<String>,
    pub candidate_checks: usize,
}

struct Supply {
    left: f64,
    task: Option<usize>,
    receipt: Option<usize>,
    at: Time,
}

fn fail(code: &str, id: &str, message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::new(code, id, message)]
}

/// Preserve free workplans and reallocate supply for every actual route selection.
/// Fully selected inputs retain the existing materialized-snapshot representation.
pub fn prepare(
    problem: &Problem,
    options: &DispatchOptions,
) -> Result<(Problem, DispatchReport), Vec<Diagnostic>> {
    compile::compile(problem)?;
    if problem.material_policy.is_some() {
        return Err(fail(
            "MATERIAL_ALREADY_PREPARED",
            &problem.id,
            "Prepare the original input, not a prepared scenario.",
        ));
    }
    let selection = Options {
        route_choices: options.route_choices.clone(),
        mode_choices: options.mode_choices.clone(),
        ..Default::default()
    };
    let (resolved, routes) = domain::resolve(problem, &selection)?;
    let flexible = problem.routes.iter().any(|r| {
        r.selected.is_none()
            && r.alternatives.len() > 1
            && !options.route_choices.contains_key(&r.id)
    }) || problem.tasks.iter().any(|t| {
        differing_modes(t)
            && !options.mode_choices.contains_key(&t.id)
            && t.execution.is_none()
            && !problem
                .locks
                .iter()
                .any(|l| matches!(l, Lock::Mode{task,..} if task == &t.id))
    });
    let selected_modes = select_modes(&resolved, &selection)?;
    let selected = DispatchOptions {
        route_choices: options.route_choices.clone(),
        mode_choices: selected_modes,
    };
    let result = allocate_active(resolved.into_owned(), &selected, routes);
    if !flexible {
        return result;
    }
    let mut report = match result {
        Ok((_, report)) => report,
        Err(errors) if errors.iter().all(|e| e.code == "MATERIAL_UNRESOLVED") => DispatchReport {
            preview_diagnostics: errors
                .iter()
                .map(|e| format!("{}: {}", e.entity, e.message))
                .collect(),
            ..Default::default()
        },
        Err(errors) => return Err(errors),
    };
    report.preview = true;
    report.policy = "reallocate_existing_supply_after_route_selection".into();
    let mut flexible = problem.clone();
    for route in &mut flexible.routes {
        if let Some(chosen) = options.route_choices.get(&route.id) {
            route.selected = Some(chosen.clone());
        }
    }
    for (task, mode) in &options.mode_choices {
        flexible.locks.push(Lock::Mode {
            task: task.clone(),
            mode: mode.clone(),
        });
    }
    flexible.material_policy = Some(MaterialPolicy::ReallocateRoutes);
    compile::compile(&flexible)?;
    Ok((flexible, report))
}

pub(crate) fn differing_modes(t: &Task) -> bool {
    t.modes.first().is_some_and(|first| {
        t.modes
            .iter()
            .any(|m| domain::materials(t, m) != domain::materials(t, first))
    })
}

pub(crate) fn branches(
    p: &Problem,
    o: &Options,
    extension: Option<&dyn crate::rules::Customization>,
) -> Result<Option<Vec<Options>>, Vec<Diagnostic>> {
    if p.material_policy.is_none() {
        return Ok(None);
    }
    let mut p = extension
        .map(|e| crate::extensions::lower(p, e))
        .transpose()?
        .unwrap_or_else(|| p.clone());
    if p.routes
        .iter()
        .any(|r| r.selected.is_none() && !o.route_choices.contains_key(&r.id))
    {
        return Ok(None);
    }
    p.material_policy = None;
    let (active, routes) = domain::resolve(&p, o)?;
    for t in &active.tasks {
        if !differing_modes(t) || o.mode_choices.contains_key(&t.id) {
            continue;
        }
        let modes: Vec<_> = t
            .modes
            .iter()
            .filter(|m| crate::xg::allowed(&active, t, m))
            .collect();
        if modes.len() < 2 {
            continue;
        }
        return Ok(Some(
            modes
                .iter()
                .map(|m| {
                    let mut child = o.clone();
                    child.route_choices = routes.clone();
                    child.mode_choices.insert(t.id.clone(), m.id.clone());
                    child
                })
                .collect(),
        ));
    }
    Ok(None)
}
/// Select differing-material modes before allocation. This is a proposal, not a
/// feasibility guarantee; search can change the gene and allocate again.
pub(crate) fn select_modes(
    p: &Problem,
    o: &Options,
) -> Result<BTreeMap<String, String>, Vec<Diagnostic>> {
    let mut selected = o.mode_choices.clone();
    for t in &p.tasks {
        if !differing_modes(t) || selected.contains_key(&t.id) {
            continue;
        }
        let mode = t
            .modes
            .iter()
            .filter(|m| crate::xg::allowed(p, t, m))
            .min_by(|a, b| {
                if matches!(o.strategy.as_str(), "random" | "weighted") {
                    domain::mixed_seed(o.seed, &format!("{}:{}", t.id, a.id))
                        .cmp(&domain::mixed_seed(o.seed, &format!("{}:{}", t.id, b.id)))
                } else {
                    let work =
                        |m: &Mode| m.phases.iter().map(|p| p.work.unwrap_or(0.0)).sum::<f64>();
                    work(a).total_cmp(&work(b)).then(a.id.cmp(&b.id))
                }
            })
            .ok_or_else(|| {
                fail(
                    "MATERIAL_MODE_SELECTION",
                    &t.id,
                    "No permitted material mode",
                )
            })?;
        selected.insert(t.id.clone(), mode.id.clone());
    }
    Ok(selected)
}

pub(crate) fn allocate_active(
    mut p: Problem,
    options: &DispatchOptions,
    routes: BTreeMap<String, String>,
) -> Result<(Problem, DispatchReport), Vec<Diagnostic>> {
    p.material_policy = None;
    p.material_report = None;
    for task in &mut p.tasks {
        let pinned = options
            .mode_choices
            .get(&task.id)
            .or_else(|| {
                p.locks.iter().find_map(|l| match l {
                    Lock::Mode { task: id, mode } if id == &task.id => Some(mode),
                    _ => None,
                })
            })
            .or_else(|| task.execution.as_ref().map(|e| &e.mode));
        if let Some(mode) = pinned {
            if p.locks.iter().any(|l| matches!(l, Lock::Mode { task: id, mode: fixed } if id == &task.id && fixed != mode))
                || task.execution.as_ref().is_some_and(|e| &e.mode != mode)
            {
                return Err(fail("MATERIAL_MODE_SELECTION", &task.id, "Selected material mode conflicts with a fixed or started mode"));
            }
            task.modes.retain(|m| &m.id == mode);
        }
        let first = task.modes.first().ok_or_else(|| {
            fail(
                "MATERIAL_MODE_SELECTION",
                &task.id,
                "No permitted material mode",
            )
        })?;
        let (consume, produce) = domain::materials(task, first);
        if task
            .modes
            .iter()
            .any(|m| domain::materials(task, m) != (consume, produce))
        {
            return Err(fail(
                "MATERIAL_MODE_SELECTION",
                &task.id,
                "Modes use different materials. Select a mode before pegging.",
            ));
        }
        // Canonicalize equal material maps before attaching supply reservation tokens.
        let maps = (consume.clone(), produce.clone());
        task.consume = maps.0;
        task.produce = maps.1;
        for mode in &mut task.modes {
            mode.consume = None;
            mode.produce = None;
        }
    }
    if p.inventory
        .keys()
        .chain(p.receipts.iter().map(|r| &r.item))
        .chain(
            p.tasks
                .iter()
                .flat_map(|t| t.consume.keys().chain(t.produce.keys())),
        )
        .any(|key| key.starts_with(PREFIX))
    {
        return Err(fail(
            "MATERIAL_ALREADY_PREPARED",
            &p.id,
            "Reserved pegging namespace present. Prepare the original scenario, not its prepared snapshot.",
        ));
    }
    let mut report = DispatchReport {
        preview: false,
        preview_diagnostics: vec![],
        policy: "existing_supply_due_order_stock_early_receipts_production_then_late_receipts"
            .into(),
        route_choices: routes,
        mode_choices: options.mode_choices.clone(),
        allocations: vec![],
        added_dependencies: 0,
        started_inputs_already_consumed: p
            .tasks
            .iter()
            .filter(|t| t.execution.is_some())
            .map(|t| t.id.clone())
            .collect(),
        candidate_checks: 0,
    };
    let ids: HashMap<_, _> = p
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.clone(), i))
        .collect();
    let mut predecessors = vec![vec![]; p.tasks.len()];
    let mut edges: std::collections::HashSet<_> = p
        .dependencies
        .iter()
        .map(|d| (d.before.clone(), d.after.clone()))
        .collect();
    for d in &p.dependencies {
        predecessors[ids[&d.after]].push(ids[&d.before]);
    }
    let mut supply: BTreeMap<String, Vec<Supply>> = BTreeMap::new();
    for (item, amount) in &p.inventory {
        supply.entry(item.clone()).or_default().push(Supply {
            left: *amount,
            task: None,
            receipt: None,
            at: 0,
        });
    }
    for (i, receipt) in p.receipts.iter().enumerate() {
        supply
            .entry(receipt.item.clone())
            .or_default()
            .push(Supply {
                left: receipt.amount,
                task: None,
                receipt: Some(i),
                at: receipt.at,
            });
    }
    let urgency = crate::urgency::derive(&p);
    let mut order: Vec<_> = (0..p.tasks.len()).collect();
    order.sort_by(|&a, &b| {
        urgency[a]
            .due
            .unwrap_or(Time::MAX)
            .cmp(&urgency[b].due.unwrap_or(Time::MAX))
            .then_with(|| urgency[b].priority.total_cmp(&urgency[a].priority))
            .then(a.cmp(&b))
    });
    let mut done = vec![false; p.tasks.len()];
    let mut pending: std::collections::BTreeSet<_> = order.iter().copied().enumerate().collect();
    let mut remaining = done.len();
    while remaining > 0 {
        let mut completed = None;
        // Late inbound supply is a fallback after all currently possible production.
        for allow_late in [false, true] {
            for &(rank, i) in &pending {
                report.candidate_checks += 1;
                if report.candidate_checks > 10_000_000 {
                    return Err(fail(
                        "MATERIAL_DISPATCH_LIMIT",
                        &p.id,
                        "Preparation exceeded 10 million candidate checks; provide explicit supply links or partition independent material networks.",
                    ));
                }
                if predecessors[i].iter().any(|&j| !done[j]) {
                    continue;
                }
                let task = &p.tasks[i];
                let due = urgency[i].due.unwrap_or(Time::MAX);
                let consumes = if task.execution.is_some() {
                    BTreeMap::new()
                } else {
                    task.consume.clone()
                };
                let eligible = |s: &Supply| allow_late || s.receipt.is_none() || s.at <= due;
                if consumes.iter().any(|(item, need)| {
                    supply.get(item).map_or(0.0, |sources| {
                        sources
                            .iter()
                            .filter(|s| eligible(s))
                            .map(|s| s.left)
                            .sum::<f64>()
                    }) + EPS
                        < *need
                }) {
                    continue;
                }
                let consumer = task.id.clone();
                for (item, need) in consumes {
                    if need <= 0.0 {
                        continue;
                    }
                    let token = format!("{PREFIX}{}", report.allocations.len());
                    let mut left = need;
                    let sources = supply.entry(item.clone()).or_default();
                    sources.sort_by_key(|s| {
                        (
                            if s.task.is_some() {
                                2
                            } else if s.receipt.is_none() {
                                0
                            } else if s.at <= due {
                                1
                            } else {
                                3
                            },
                            s.at,
                            s.task,
                            s.receipt,
                        )
                    });
                    for source in sources.iter_mut().filter(|s| eligible(s)) {
                        if left <= EPS {
                            break;
                        }
                        let quantity = left.min(source.left);
                        if quantity <= 0.0 {
                            continue;
                        }
                        source.left -= quantity;
                        left -= quantity;
                        let source_task = source.task.map(|j| p.tasks[j].id.clone());
                        if let Some(j) = source.task {
                            *p.tasks[j].produce.entry(token.clone()).or_default() += quantity;
                            if edges.insert((p.tasks[j].id.clone(), consumer.clone())) {
                                p.dependencies.push(Dependency {
                                    before: p.tasks[j].id.clone(),
                                    after: consumer.clone(),
                                    min_lag: 0,
                                    max_lag: None,
                                });
                                report.added_dependencies += 1;
                            }
                        } else if source.receipt.is_some() {
                            p.receipts.push(Receipt {
                                item: token.clone(),
                                at: source.at,
                                amount: quantity,
                            });
                        } else {
                            *p.inventory.entry(token.clone()).or_default() += quantity;
                        }
                        report.allocations.push(Allocation {
                            consumer: consumer.clone(),
                            item: item.clone(),
                            quantity,
                            source: if source.task.is_some() {
                                "production"
                            } else if source.receipt.is_some() {
                                "receipt"
                            } else {
                                "stock"
                            }
                            .into(),
                            source_task,
                            receipt_index: source.receipt,
                            available_at: source.task.is_none().then_some(source.at),
                            token: token.clone(),
                        });
                    }
                    p.tasks[i].consume.insert(token, need);
                }
                for (item, amount) in &p.tasks[i].produce {
                    if !item.starts_with(PREFIX) {
                        supply.entry(item.clone()).or_default().push(Supply {
                            left: *amount,
                            task: Some(i),
                            receipt: None,
                            at: 0,
                        });
                    }
                }
                done[i] = true;
                remaining -= 1;
                completed = Some((rank, i));
                // Reconsider earlier due tasks as soon as new production is available.
                break;
            }
            if completed.is_some() {
                break;
            }
        }
        if let Some(key) = completed {
            pending.remove(&key);
        } else {
            let errors = order.iter().filter(|&&i| !done[i]).take(100).map(|&i| {
                let missing: Vec<_> = p.tasks[i].consume.iter().filter_map(|(item, need)| {
                    let available = supply.get(item).map_or(0.0, |s| s.iter().map(|v| v.left).sum::<f64>());
                    (available + EPS < *need).then(|| format!("{item}: need {need}, currently allocatable {available}"))
                }).collect();
                Diagnostic::new("MATERIAL_UNRESOLVED", &p.tasks[i].id, format!("Supply shortage or cyclic/blocked production network: {}. No orders generated; this is not a proof of global infeasibility.", missing.join("; ")))
            }).collect();
            return Err(errors);
        }
    }
    p.assumptions.push("Material pegging binds this snapshot to selected routes and supply. Reprepare the original scenario after material/routing changes. Started inputs were consumed before the input snapshot.".into());
    compile::compile(&p)?;
    Ok((p, report))
}
