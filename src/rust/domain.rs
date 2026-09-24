//! Domain relationships and deterministic lowering to an active planning snapshot.
use crate::model::*;
use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn check(p: &Problem, errors: &mut Vec<Diagnostic>) {
    let tasks: HashMap<_, _> = p.tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut fail = |code, id: &str, message: &str| errors.push(Diagnostic::new(code, id, message));
    let mut route_ids = HashSet::new();
    let mut membership = HashMap::new();
    for choice in &p.routes {
        if choice.id.is_empty() || !route_ids.insert(&choice.id) || choice.alternatives.is_empty() {
            fail(
                "ROUTE",
                &choice.id,
                "Route choices need unique IDs and alternatives",
            );
        }
        let mut alternatives = HashSet::new();
        for alternative in &choice.alternatives {
            let mut local = HashSet::new();
            if alternative.id.is_empty()
                || !alternatives.insert(&alternative.id)
                || alternative.tasks.is_empty()
            {
                fail(
                    "ROUTE",
                    &choice.id,
                    "Alternatives need unique IDs and at least one task",
                );
            }
            for task in &alternative.tasks {
                if !tasks.contains_key(task.as_str()) || !local.insert(task) {
                    fail("ROUTE_TASK", task, "Unknown or duplicate route task");
                }
                if membership
                    .insert(task, &choice.id)
                    .is_some_and(|owner| owner != &choice.id)
                {
                    fail(
                        "ROUTE_TASK",
                        task,
                        "A task may belong to alternatives of only one route choice",
                    );
                }
            }
            for edge in &alternative.dependencies {
                if !tasks.contains_key(edge.before.as_str())
                    || !tasks.contains_key(edge.after.as_str())
                    || edge.min_lag < 0
                    || edge.min_lag > p.horizon
                    || edge
                        .max_lag
                        .is_some_and(|v| v < edge.min_lag || v > p.horizon)
                {
                    fail("ROUTE_EDGE", &choice.id, "Invalid route dependency");
                }
            }
        }
        if choice
            .selected
            .as_ref()
            .is_some_and(|id| !alternatives.contains(id))
        {
            fail(
                "ROUTE_SELECTION",
                &choice.id,
                "Unknown selected alternative",
            );
        }
    }
    let populated_jobs: HashSet<_> = p.tasks.iter().filter_map(|t| t.job.as_deref()).collect();
    let mut jobs = HashSet::new();
    for job in &p.jobs {
        if job.id.is_empty()
            || !jobs.insert(job.id.as_str())
            || !job.quantity.is_finite()
            || job.quantity <= 0.0
            || !job.priority.is_finite()
            || job.priority < 0.0
            || job.due.is_some_and(|x| x < 0)
            || job.deadline.is_some_and(|x| x < 0 || x > p.horizon)
            || !populated_jobs.contains(job.id.as_str())
        {
            fail(
                "JOB",
                &job.id,
                "Invalid or empty job, quantity, priority or bounds",
            );
        }
    }
    for task in &p.tasks {
        if !task.quantity.is_finite()
            || task.quantity <= 0.0
            || task.quantity > 1e12
            || task.job.as_deref().is_some_and(|id| !jobs.contains(id))
        {
            fail("TASK_JOB", &task.id, "Invalid task quantity or unknown job");
        }
        for mode in &task.modes {
            for amounts in [mode.consume.as_ref(), mode.produce.as_ref()]
                .into_iter()
                .flatten()
            {
                if amounts.values().any(|v| !v.is_finite() || *v < 0.0) {
                    fail(
                        "MODE_MATERIAL",
                        &task.id,
                        "Material quantities must be finite and nonnegative",
                    );
                }
            }
            for c in task
                .pre
                .iter()
                .chain(&task.post)
                .chain(&mode.pre)
                .chain(&mode.post)
            {
                if c.owner_resource.as_ref().is_some_and(|r| {
                    !mode
                        .phases
                        .iter()
                        .any(|ph| ph.requirements.iter().any(|q| &q.resource == r))
                }) {
                    fail(
                        "CONDITIONAL_OWNER",
                        &c.id,
                        "Owner resource must be a requirement of the associated main mode",
                    );
                }
            }
        }
    }
    let mut orders = HashSet::new();
    for order in &p.orders {
        let members: HashSet<_> = order.jobs.iter().collect();
        if order.id.is_empty()
            || !orders.insert(&order.id)
            || members.len() != order.jobs.len()
            || members.is_empty()
            || order.jobs.iter().any(|j| !jobs.contains(j.as_str()))
            || !order.priority.is_finite()
            || order.priority < 0.0
            || order.due.is_some_and(|x| x < 0)
            || order.deadline.is_some_and(|x| x < 0 || x > p.horizon)
        {
            fail(
                "ORDER",
                &order.id,
                "Invalid order identity, job membership, priority or bounds",
            );
        }
    }
}

fn lower_modes(modes: &mut [Mode], quantity: f64) {
    for mode in modes {
        for phase in &mut mode.phases {
            if let Some(per_unit) = phase.work_per_unit.take() {
                phase.work = Some(phase.work.unwrap_or(0.0) + quantity * per_unit);
            }
        }
        for conditional in mode.pre.iter_mut().chain(&mut mode.post) {
            lower_modes(&mut conditional.modes, quantity);
        }
    }
}

pub type Resolved<'a> = (Cow<'a, Problem>, BTreeMap<String, String>);

pub fn resolve<'a>(p: &'a Problem, options: &Options) -> Result<Resolved<'a>, Vec<Diagnostic>> {
    if crate::language::needs_lowering(p) {
        let lowered = crate::language::lower(p)?;
        let (resolved, choices) = resolve(&lowered, options)?;
        return Ok((Cow::Owned(resolved.into_owned()), choices));
    }
    if !requires_lowering(p)
        && options.route_choices.is_empty()
        && options.mode_choices.is_empty()
        && options.conditional_choices.is_empty()
    {
        return Ok((Cow::Borrowed(p), BTreeMap::new()));
    }
    let mut errors = Vec::new();
    check(p, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    for id in options.route_choices.keys() {
        if !p.routes.iter().any(|r| &r.id == id) {
            return Err(vec![Diagnostic::new(
                "ROUTE_SELECTION",
                id,
                "Unknown route choice",
            )]);
        }
    }
    let tasks: HashMap<_, _> = p.tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    let required: HashSet<&str> = p
        .locks
        .iter()
        .flat_map(|l| match l {
            Lock::Mode { task, .. } | Lock::Resource { task, .. } | Lock::Start { task, .. } => {
                vec![task.as_str()]
            }
            Lock::Order { tasks, .. } => tasks.iter().map(String::as_str).collect(),
        })
        .chain(
            p.tasks
                .iter()
                .filter(|t| t.execution.is_some())
                .map(|t| t.id.as_str()),
        )
        .chain(options.mode_choices.keys().map(String::as_str))
        .chain(options.conditional_choices.keys().map(String::as_str))
        .chain(
            p.tasks
                .iter()
                .filter(|t| !t.conditional_modes.is_empty())
                .map(|t| t.id.as_str()),
        )
        .chain(options.decision_prefix.iter().map(|d| d.task.as_str()))
        .collect();
    for (task, mode) in &options.mode_choices {
        if tasks
            .get(task.as_str())
            .is_none_or(|t| !t.modes.iter().any(|m| &m.id == mode))
        {
            return Err(vec![Diagnostic::new(
                "MODE_SELECTION",
                task,
                "Unknown task or execution mode",
            )]);
        }
    }
    let mut selections = BTreeMap::new();
    let mut inactive = HashSet::new();
    let mut edges = Vec::new();
    for choice in &p.routes {
        let members: HashSet<_> = choice
            .alternatives
            .iter()
            .flat_map(|a| a.tasks.iter())
            .collect();
        let pinned = options
            .route_choices
            .get(&choice.id)
            .or(choice.selected.as_ref());
        if options
            .route_choices
            .get(&choice.id)
            .zip(choice.selected.as_ref())
            .is_some_and(|(a, b)| a != b)
        {
            return Err(vec![Diagnostic::new(
                "ROUTE_LOCK",
                &choice.id,
                "Selected workplan cannot be overridden",
            )]);
        }
        let mut candidates: Vec<_> = choice
            .alternatives
            .iter()
            .filter(|a| {
                pinned.is_none_or(|id| id == &a.id)
                    && members
                        .iter()
                        .all(|id| !required.contains(id.as_str()) || a.tasks.contains(id))
            })
            .collect();
        candidates.sort_by(|a, b| {
            let work = |a: &RouteAlternative| {
                a.tasks
                    .iter()
                    .map(|id| {
                        let t = tasks[id.as_str()];
                        t.modes
                            .iter()
                            .map(|m| {
                                m.phases
                                    .iter()
                                    .map(|ph| {
                                        ph.work.unwrap_or(0.0)
                                            + ph.work_per_unit.unwrap_or(0.0) * t.quantity
                                    })
                                    .sum::<f64>()
                            })
                            .fold(f64::INFINITY, f64::min)
                    })
                    .sum::<f64>()
            };
            work(a).total_cmp(&work(b)).then(a.id.cmp(&b.id))
        });
        if candidates.is_empty() {
            return Err(vec![Diagnostic::new(
                "ROUTE_SELECTION",
                &choice.id,
                "No alternative satisfies selection and fixed/executed tasks",
            )]);
        }
        let position =
            if pinned.is_some() || !matches!(options.strategy.as_str(), "random" | "weighted") {
                0
            } else {
                (mixed_seed(options.seed, &choice.id) % candidates.len() as u64) as usize
            };
        let selected = candidates[position];
        selections.insert(choice.id.clone(), selected.id.clone());
        inactive.extend(
            members
                .into_iter()
                .filter(|id| !selected.tasks.contains(id))
                .map(String::as_str),
        );
        edges.extend(selected.dependencies.clone());
    }
    let formulas = p.tasks.iter().any(|t| {
        t.modes.iter().any(mode_has_formula)
            || t.pre
                .iter()
                .chain(&t.post)
                .chain(t.execution.iter().flat_map(|e| &e.restart))
                .any(|c| c.modes.iter().any(mode_has_formula))
    }) || p.transitions.iter().any(|tr| {
        tr.next_pre
            .iter()
            .chain(&tr.previous_post)
            .any(|c| c.modes.iter().any(mode_has_formula))
    }) || !p.jobs.is_empty()
        || !p.orders.is_empty();
    if p.routes.is_empty()
        && !formulas
        && p.material_policy.is_none()
        && options.conditional_choices.is_empty()
    {
        return Ok((Cow::Borrowed(p), selections));
    }
    let mut active = p.clone();
    active.routes.clear();
    active.tasks.retain(|t| !inactive.contains(t.id.as_str()));
    active.dependencies.extend(edges);
    active
        .dependencies
        .retain(|d| !inactive.contains(d.before.as_str()) && !inactive.contains(d.after.as_str()));
    active.rules.retain(|r| match r {
        Rule::TaskWindow { task, .. } | Rule::SetupCharge { task, .. } => {
            !inactive.contains(task.as_str())
        }
        _ => true,
    });
    let job_index: HashMap<_, _> = active.jobs.iter().map(|j| (j.id.as_str(), j)).collect();
    let mut order_bounds = HashMap::<&str, (Option<Time>, Option<Time>)>::new();
    for order in &active.orders {
        for job in &order.jobs {
            let entry = order_bounds.entry(job).or_default();
            if let Some(due) = order.due {
                entry.0 = Some(entry.0.unwrap_or(due).min(due));
            }
            if let Some(deadline) = order.deadline {
                entry.1 = Some(entry.1.unwrap_or(deadline).min(deadline));
            }
        }
    }
    for t in &mut active.tasks {
        lower_modes(&mut t.modes, t.quantity);
        for c in t
            .pre
            .iter_mut()
            .chain(&mut t.post)
            .chain(t.execution.iter_mut().flat_map(|e| &mut e.restart))
        {
            lower_modes(&mut c.modes, t.quantity);
        }
        if let Some(job) = t.job.as_deref().and_then(|id| job_index.get(id)) {
            if t.due.is_none() {
                t.due = job.due;
            }
            t.priority = job.priority;
            if let Some(deadline) = job.deadline {
                t.deadline = Some(t.deadline.unwrap_or(deadline).min(deadline));
            }
            if let Some((due, deadline)) = order_bounds.get(job.id.as_str()) {
                if t.due.is_none() {
                    t.due = *due;
                }
                if let Some(deadline) = deadline {
                    t.deadline = Some(t.deadline.unwrap_or(*deadline).min(*deadline));
                }
            }
        }
    }
    crate::conditionals::apply(&mut active, options)?;
    if active.material_policy.is_some() {
        let dispatch = crate::material::DispatchOptions {
            route_choices: selections.clone(),
            mode_choices: crate::material::select_modes(&active, options)?,
        };
        let (mut prepared, report) =
            crate::material::allocate_active(active, &dispatch, selections.clone())?;
        prepared.material_report = Some(report);
        active = prepared;
    }
    Ok((Cow::Owned(active), selections))
}
fn mode_has_formula(m: &Mode) -> bool {
    m.phases.iter().any(|p| p.work_per_unit.is_some())
        || m.pre
            .iter()
            .chain(&m.post)
            .any(|c| c.modes.iter().any(mode_has_formula))
}

pub fn materials<'a>(
    t: &'a Task,
    m: &'a Mode,
) -> (&'a BTreeMap<String, f64>, &'a BTreeMap<String, f64>) {
    (
        m.consume.as_ref().unwrap_or(&t.consume),
        m.produce.as_ref().unwrap_or(&t.produce),
    )
}

pub fn completions(p: &Problem, s: &Schedule) -> (BTreeMap<String, Time>, BTreeMap<String, Time>) {
    if p.jobs.is_empty() && p.orders.is_empty() {
        return (BTreeMap::new(), BTreeMap::new());
    }
    let tasks: HashMap<_, _> = p.tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut jobs = BTreeMap::<String, Time>::new();
    for a in &s.assignments {
        if let Some(job) = tasks.get(a.task.as_str()).and_then(|t| t.job.as_ref()) {
            jobs.entry(job.clone())
                .and_modify(|v| *v = (*v).max(a.ready))
                .or_insert(a.ready);
        }
    }
    let orders = p
        .orders
        .iter()
        .filter_map(|o| {
            let ends: Option<Vec<_>> = o.jobs.iter().map(|id| jobs.get(id)).collect();
            ends.and_then(|v| v.into_iter().max().map(|end| (o.id.clone(), *end)))
        })
        .collect();
    (jobs, orders)
}

pub fn scaled_mode(mode: &Mode, quantity: f64) -> Mode {
    let mut mode = mode.clone();
    lower_modes(std::slice::from_mut(&mut mode), quantity);
    mode
}

pub fn mixed_seed(seed: u64, key: &str) -> u64 {
    let mut x = key.bytes().fold(seed, |x, b| {
        x.wrapping_mul(0x100000001b3).wrapping_add(b as u64)
    });
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111eb);
    x ^ (x >> 31)
}
pub fn requires_lowering(p: &Problem) -> bool {
    crate::language::needs_lowering(p)
        || p.material_policy.is_some()
        || !p.routes.is_empty()
        || !p.jobs.is_empty()
        || !p.orders.is_empty()
        || p.tasks.iter().any(|t| {
            t.modes.iter().any(mode_has_formula)
                || t.pre
                    .iter()
                    .chain(&t.post)
                    .chain(t.execution.iter().flat_map(|e| &e.restart))
                    .any(|c| c.modes.iter().any(mode_has_formula))
        })
        || p.transitions.iter().any(|tr| {
            tr.next_pre
                .iter()
                .chain(&tr.previous_post)
                .any(|c| c.modes.iter().any(mode_has_formula))
        })
}
