//! Direct schedule evolution. Operators propose priorities/choices, never relax feasibility.
use crate::{
    engine,
    model::*,
    rules::Customization,
    search::{self, Random},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    time::Instant,
};

#[derive(Clone, Debug, JsonSchema, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub selection: String,
    pub discount: f64,
    pub exploration: f64,
    pub random_share: f64,
    /// None preserves the caller's existing Trainer population setting.
    pub population_size: Option<usize>,
    /// None lets all eligible operator kinds compete in the bandit.
    pub crossover_share: Option<f64>,
    pub parent_selection: String,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            selection: "discounted_ucb".into(),
            discount: 0.99,
            exploration: 1.0,
            random_share: 0.1,
            population_size: None,
            crossover_share: Some(0.5),
            parent_selection: "tournament".into(),
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq, JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Genome {
    pub order: Vec<String>,
    pub routes: BTreeMap<String, String>,
    pub modes: BTreeMap<String, String>,
    pub conditionals: BTreeMap<String, BTreeMap<String, String>>,
}
#[derive(Clone, Debug, Default, JsonSchema, Serialize, Deserialize)]
pub struct OperatorReport {
    pub id: String,
    pub kind: OperatorKind,
    pub attempts: usize,
    pub rejected: usize,
    pub unchanged: usize,
    /// Unchanged or duplicate proposals discarded before an evaluation.
    #[serde(default)]
    pub skipped: usize,
    pub reordered: usize,
    pub improvements: usize,
    pub reward: f64,
    pub evaluation_ms: f64,
    pub discounted_count: f64,
    pub discounted_reward: f64,
    pub rejection_codes: BTreeMap<String, usize>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, JsonSchema, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperatorKind {
    #[default]
    Mutation,
    Crossover,
}
pub struct OperatorDefinition {
    pub id: String,
    pub kind: OperatorKind,
}
/// Linked native code receives immutable context and one deterministic seed per proposal.
pub struct OperatorContext<'a> {
    pub id: &'a str,
    pub problem: &'a Problem,
    pub parent: &'a Genome,
    pub other: &'a Genome,
    pub schedule: &'a Schedule,
    pub seed: u64,
}
type Outcome = Result<Schedule, Vec<Diagnostic>>;
const BUILTINS: [&str; 10] = [
    "insert",
    "swap",
    "job_block",
    "resource_block",
    "tardy_insert",
    "main_mode",
    "route",
    "conditional_mode",
    "job_crossover",
    "conditional_reset",
];

pub fn check_order(p: &Problem, order: &[String]) -> Result<(), Diagnostic> {
    if order.is_empty() {
        return Ok(());
    }
    let ids: HashSet<_> = p.tasks.iter().map(|t| &t.id).collect();
    let mut seen = HashSet::new();
    if order.len() > p.tasks.len() || order.iter().any(|id| !ids.contains(id) || !seen.insert(id)) {
        return Err(Diagnostic::new(
            "DECISION_ORDER",
            &p.id,
            "Operation priorities contain unknown or duplicate tasks",
        ));
    }
    Ok(())
}
pub fn check_config(o: &Options) -> Result<(), Vec<Diagnostic>> {
    let c = &o.evolution;
    if !["uniform", "discounted_ucb"].contains(&c.selection.as_str())
        || !c.discount.is_finite()
        || c.discount <= 0.0
        || c.discount > 1.0
        || !c.exploration.is_finite()
        || c.exploration < 0.0
        || !c.random_share.is_finite()
        || !(0.0..=1.0).contains(&c.random_share)
        || c.population_size.is_some_and(|n| !(2..=256).contains(&n))
        || c.crossover_share
            .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
        || !["uniform", "tournament"].contains(&c.parent_selection.as_str())
    {
        return Err(vec![Diagnostic::new(
            "EVOLUTION_OPTIONS",
            "evolution",
            "Invalid operator selection, discount, exploration, population, crossover share or parent selection",
        )]);
    }
    Ok(())
}
fn from_schedule(p: &Problem, s: &Schedule) -> Genome {
    let mut order: Vec<_> = s.construction.iter().map(|d| d.task.clone()).collect();
    if order.is_empty() {
        let mut assignments: Vec<_> = s.assignments.iter().collect();
        assignments.sort_by_key(|a| (a.start, &a.task));
        order = assignments.iter().map(|a| a.task.clone()).collect();
    }
    let present: HashSet<_> = order.iter().cloned().collect();
    order.extend(
        p.tasks
            .iter()
            .filter(|t| !present.contains(&t.id))
            .map(|t| t.id.clone()),
    );
    let mut g = Genome {
        order,
        routes: s.route_choices.clone(),
        modes: s
            .assignments
            .iter()
            .map(|a| (a.task.clone(), a.mode.clone()))
            .collect(),
        ..Default::default()
    };
    // Inactive transition choices are intentionally absent. A changed sequence may
    // reject an inherited active choice; it must never silently turn a lock off.
    let catalog = crate::conditionals::catalog(p);
    for a in &s.assignments {
        if let Some(row) = catalog.get(&a.task) {
            for (key, modes) in row {
                if let Some(activity) = a
                    .activities
                    .iter()
                    .find(|v| v.id == format!("{}:{key}", a.task))
                    && modes.contains(&activity.mode)
                {
                    g.conditionals
                        .entry(a.task.clone())
                        .or_default()
                        .insert(key.clone(), activity.mode.clone());
                }
            }
        }
    }
    g
}
/// Preserve caller commitments separately from inherited mutable genes.
fn options(p: &Problem, base: &Options, g: &Genome) -> Result<Options, Vec<Diagnostic>> {
    check_order(p, &g.order).map_err(|e| vec![e])?;
    if g.order.len() != p.tasks.len() {
        return Err(vec![Diagnostic::new(
            "GENOME_COVERAGE",
            &p.id,
            "A chromosome must cover every task, including inactive alternatives",
        )]);
    }
    let mut o = base.clone();
    o.strategy = "due".into();
    o.weights = None;
    o.queue_policy = None;
    o.decision_order = g.order.clone();
    o.route_choices = g.routes.clone();
    o.route_choices.extend(base.route_choices.clone());
    let mut inactive = HashSet::new();
    for route in &p.routes {
        let selected = o.route_choices.get(&route.id).or(route.selected.as_ref());
        if let Some(alt) = route.alternatives.iter().find(|a| Some(&a.id) == selected) {
            for id in route.alternatives.iter().flat_map(|a| &a.tasks) {
                if !alt.tasks.contains(id) {
                    inactive.insert(id.clone());
                }
            }
        }
    }
    o.mode_choices = g
        .modes
        .iter()
        .filter(|(id, _)| !inactive.contains(*id))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    o.mode_choices.extend(base.mode_choices.clone());
    o.conditional_choices = g
        .conditionals
        .iter()
        .filter(|(id, _)| !inactive.contains(*id))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    for (task, row) in &base.conditional_choices {
        o.conditional_choices
            .entry(task.clone())
            .or_default()
            .extend(row.clone());
    }
    for d in &base.decision_prefix {
        o.mode_choices.insert(d.task.clone(), d.mode.clone());
    }
    Ok(o)
}
fn move_block(order: &mut Vec<String>, block: &HashSet<String>, rng: &mut Random) {
    let selected: Vec<_> = order
        .iter()
        .filter(|id| block.contains(*id))
        .cloned()
        .collect();
    order.retain(|id| !block.contains(id));
    let at = rng.index(order.len() + 1);
    order.splice(at..at, selected);
}
/// Job-order crossover keeps the selected job positions from A and fills other
/// positions in B's order. The shared ready-set decoder supplies topological repair.
pub fn crossover(p: &Problem, a: &Genome, b: &Genome, rng: &mut Random) -> Genome {
    let mut child = a.clone();
    let mut groups = BTreeMap::new();
    let keep: HashSet<_> = p
        .tasks
        .iter()
        .filter(|t| {
            *groups
                .entry(t.job.as_ref().unwrap_or(&t.id))
                .or_insert_with(|| rng.chance(0.5))
        })
        .map(|t| &t.id)
        .collect();
    let mut donor = b.order.iter().filter(|id| !keep.contains(id));
    for id in &mut child.order {
        if !keep.contains(id) {
            *id = donor.next().unwrap().clone();
        }
    }
    for (id, choice) in &b.routes {
        if rng.chance(0.5) {
            child.routes.insert(id.clone(), choice.clone());
        }
    }
    for (id, mode) in &b.modes {
        if !keep.contains(id) {
            child.modes.insert(id.clone(), mode.clone());
        }
    }
    for (id, row) in &b.conditionals {
        if !keep.contains(id) {
            child.conditionals.insert(id.clone(), row.clone());
        }
    }
    child
}
/// Parent-specific admissible moves. This is an optimization filter, not a
/// replacement for the shared decoder or independent hard-rule validator.
struct Moves {
    order: Vec<String>,
    job_blocks: bool,
    resource_blocks: bool,
    modes: Vec<(String, Vec<String>)>,
    routes: Vec<(String, Vec<String>)>,
    conditionals: Vec<(String, String, Vec<String>)>,
    resets: Vec<(String, String)>,
    late: Option<String>,
}
impl Moves {
    fn new(p: &Problem, o: &Options, a: &Genome, s: &Schedule) -> Self {
        let protected: HashSet<_> = o
            .decision_prefix
            .iter()
            .map(|d| d.task.as_str())
            .chain(
                p.tasks
                    .iter()
                    .filter(|t| t.execution.is_some())
                    .map(|t| t.id.as_str()),
            )
            .chain(p.locks.iter().filter_map(|l| match l {
                Lock::Start { task, .. } => Some(task.as_str()),
                _ => None,
            }))
            .collect();
        let active: HashSet<_> = s.assignments.iter().map(|v| v.task.as_str()).collect();
        let order: Vec<_> = a
            .order
            .iter()
            .filter(|id| active.contains(id.as_str()) && !protected.contains(id.as_str()))
            .cloned()
            .collect();
        let movable: HashSet<_> = order.iter().map(String::as_str).collect();
        let job_blocks = p
            .tasks
            .iter()
            .filter(|t| movable.contains(t.id.as_str()))
            .map(|t| t.job.as_deref().unwrap_or(&t.id))
            .collect::<HashSet<_>>()
            .len()
            > 1;
        let resource_blocks = s
            .assignments
            .iter()
            .filter(|v| movable.contains(v.task.as_str()))
            .map(|v| &v.primary)
            .collect::<HashSet<_>>()
            .len()
            > 1;
        let modes = p
            .tasks
            .iter()
            .filter(|t| {
                active.contains(t.id.as_str())
                    && !o.mode_choices.contains_key(&t.id)
                    && !o.decision_prefix.iter().any(|d| d.task == t.id)
            })
            .filter_map(|t| {
                let choices: Vec<_> = t
                    .modes
                    .iter()
                    .filter(|m| a.modes.get(&t.id) != Some(&m.id) && engine::allowed(p, t, m))
                    .map(|m| m.id.clone())
                    .collect();
                (!choices.is_empty()).then(|| (t.id.clone(), choices))
            })
            .collect();
        let routes = p
            .routes
            .iter()
            .filter(|r| r.selected.is_none() && !o.route_choices.contains_key(&r.id))
            .filter_map(|r| {
                let choices: Vec<_> = r
                    .alternatives
                    .iter()
                    .filter(|v| a.routes.get(&r.id) != Some(&v.id))
                    .map(|v| v.id.clone())
                    .collect();
                (!choices.is_empty()).then(|| (r.id.clone(), choices))
            })
            .collect();
        let fixed_conditional = |id: &str, key: &str| {
            o.conditional_choices
                .get(id)
                .is_some_and(|r| r.contains_key(key))
                || p.tasks
                    .iter()
                    .find(|t| t.id == id)
                    .is_some_and(|t| t.conditional_modes.contains_key(key))
        };
        let mut conditionals = vec![];
        for (id, row) in crate::conditionals::catalog(p) {
            if !active.contains(id.as_str()) {
                continue;
            }
            for (key, modes) in row {
                if fixed_conditional(&id, &key) {
                    continue;
                }
                let current = a.conditionals.get(&id).and_then(|r| r.get(&key));
                let alternatives: Vec<_> =
                    modes.into_iter().filter(|m| Some(m) != current).collect();
                if !alternatives.is_empty() {
                    conditionals.push((id.clone(), key, alternatives));
                }
            }
        }
        let resets = a
            .conditionals
            .iter()
            .flat_map(|(id, row)| row.keys().map(move |key| (id.clone(), key.clone())))
            .filter(|(id, key)| !fixed_conditional(id, key))
            .collect();
        let (resolved, _) = crate::domain::resolve(p, s.replay.as_deref().unwrap_or(o))
            .unwrap_or_else(|_| (std::borrow::Cow::Borrowed(p), BTreeMap::new()));
        let due: BTreeMap<_, _> = resolved
            .tasks
            .iter()
            .zip(crate::urgency::derive(&resolved))
            .filter_map(|(t, u)| u.due.map(|d| (t.id.as_str(), d)))
            .collect();
        let late = s
            .assignments
            .iter()
            .filter(|v| {
                due.get(v.task.as_str()).is_some_and(|d| v.ready > *d)
                    && order
                        .iter()
                        .position(|id| id == &v.task)
                        .is_some_and(|i| i > 0)
            })
            .max_by_key(|v| v.ready - due[v.task.as_str()])
            .map(|v| v.task.clone());
        Self {
            order,
            job_blocks,
            resource_blocks,
            modes,
            routes,
            conditionals,
            resets,
            late,
        }
    }
    fn eligible(&self, index: usize, a: &Genome, b: &Genome) -> bool {
        match index {
            0..=1 => self.order.len() > 1,
            2 => self.job_blocks,
            3 => self.resource_blocks,
            4 => self.late.is_some(),
            5 => !self.modes.is_empty(),
            6 => !self.routes.is_empty(),
            7 => !self.conditionals.is_empty(),
            8 => a != b,
            9 => !self.resets.is_empty(),
            _ => true, // Native hooks retain authority over their own applicability.
        }
    }
}
fn mutate(
    p: &Problem,
    a: &Genome,
    s: &Schedule,
    index: usize,
    moves: &Moves,
    rng: &mut Random,
) -> Genome {
    let mut child = a.clone();
    let mut order = moves.order.clone();
    match index {
        0 => {
            let i = rng.index(order.len());
            let id = order.remove(i);
            let at = (i + 1 + rng.index(order.len())) % (order.len() + 1);
            order.insert(at, id);
        }
        1 => {
            let i = rng.index(order.len());
            let j = (i + 1 + rng.index(order.len() - 1)) % order.len();
            order.swap(i, j);
        }
        2 | 3 => {
            let id = &order[rng.index(order.len())];
            let task = p.tasks.iter().find(|t| &t.id == id).unwrap();
            let block = if index == 2 {
                p.tasks
                    .iter()
                    .filter(|t| t.id == task.id || task.job.is_some() && t.job == task.job)
                    .map(|t| t.id.clone())
                    .collect()
            } else {
                let primary = s
                    .assignments
                    .iter()
                    .find(|v| v.task == task.id)
                    .map(|v| &v.primary);
                s.assignments
                    .iter()
                    .filter(|v| Some(&v.primary) == primary)
                    .map(|v| v.task.clone())
                    .collect()
            };
            move_block(&mut order, &block, rng);
        }
        4 => {
            let at = order
                .iter()
                .position(|id| Some(id) == moves.late.as_ref())
                .unwrap();
            let id = order.remove(at);
            order.insert(rng.index(at), id);
        }
        5 => {
            let (id, choices) = &moves.modes[rng.index(moves.modes.len())];
            child
                .modes
                .insert(id.clone(), choices[rng.index(choices.len())].clone());
            child.conditionals.remove(id);
        }
        6 => {
            let (id, choices) = &moves.routes[rng.index(moves.routes.len())];
            let choice = &choices[rng.index(choices.len())];
            child.routes.insert(id.clone(), choice.clone());
            let alt = p
                .routes
                .iter()
                .find(|r| &r.id == id)
                .unwrap()
                .alternatives
                .iter()
                .find(|v| &v.id == choice)
                .unwrap();
            for id in &alt.tasks {
                if !child.modes.contains_key(id) {
                    let t = p.tasks.iter().find(|t| &t.id == id).unwrap();
                    let modes: Vec<_> = t
                        .modes
                        .iter()
                        .filter(|m| engine::allowed(p, t, m))
                        .collect();
                    if !modes.is_empty() {
                        child
                            .modes
                            .insert(id.clone(), modes[rng.index(modes.len())].id.clone());
                    }
                }
            }
        }
        7 => {
            let (id, key, choices) = &moves.conditionals[rng.index(moves.conditionals.len())];
            child
                .conditionals
                .entry(id.clone())
                .or_default()
                .insert(key.clone(), choices[rng.index(choices.len())].clone());
        }
        9 => {
            for (id, key) in &moves.resets {
                if let Some(row) = child.conditionals.get_mut(id) {
                    row.remove(key);
                }
            }
            child.conditionals.retain(|_, row| !row.is_empty());
        }
        _ => {}
    }
    if index <= 4 {
        let movable: HashSet<_> = moves.order.iter().collect();
        let mut changed = order.into_iter();
        for id in &mut child.order {
            if movable.contains(id) {
                *id = changed.next().unwrap();
            }
        }
    }
    child
}
fn arm(
    reports: &[OperatorReport],
    pending: &[usize],
    eligible: &[usize],
    c: &Config,
    rng: &mut Random,
) -> usize {
    let mut pool = eligible.to_vec();
    if let Some(share) = c.crossover_share {
        let crossover = rng.chance(share);
        let preferred: Vec<_> = pool
            .iter()
            .copied()
            .filter(|&i| (reports[i].kind == OperatorKind::Crossover) == crossover)
            .collect();
        if !preferred.is_empty() {
            pool = preferred;
        }
    }
    if c.selection == "uniform" || rng.chance(c.random_share) {
        return pool[rng.index(pool.len())];
    }
    if let Some(&i) = pool
        .iter()
        .find(|&&i| reports[i].attempts + reports[i].skipped + pending[i] == 0)
    {
        return i;
    }
    let total = pool
        .iter()
        .map(|&i| reports[i].discounted_count + pending[i] as f64)
        .sum::<f64>();
    pool.into_iter()
        .map(|i| {
            let r = &reports[i];
            let n = (r.discounted_count + pending[i] as f64).max(1e-9);
            (
                i,
                r.discounted_reward / n + c.exploration * (2.0 * total.max(1.0).ln() / n).sqrt(),
            )
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .unwrap()
        .0
}
fn parent_index(size: usize, c: &Config, rng: &mut Random) -> usize {
    let a = rng.index(size);
    if c.parent_selection == "tournament" {
        // Population order is nondominated rank/crowding or priority score.
        a.min(rng.index(size))
    } else {
        a
    }
}
fn credit(reports: &mut [OperatorReport], index: usize, c: &Config, reward: f64) {
    for r in reports.iter_mut() {
        r.discounted_count *= c.discount;
        r.discounted_reward *= c.discount;
    }
    reports[index].discounted_count += 1.0;
    reports[index].discounted_reward += reward;
    reports[index].reward += reward;
}
fn select(population: &mut Vec<(Genome, Schedule)>, o: &Options) {
    let mut unique = HashSet::new();
    population.retain(|(g, _)| unique.insert(serde_json::to_string(g).unwrap()));
    let order = if o.trainer.selection == "pareto" {
        search::pareto_order(
            &population
                .iter()
                .map(|(_, s)| s.objective_values.clone())
                .collect::<Vec<_>>(),
        )
    } else {
        let mut indexes: Vec<_> = (0..population.len()).collect();
        indexes.sort_by(|&a, &b| {
            population[a]
                .1
                .score
                .partial_cmp(&population[b].1.score)
                .unwrap()
        });
        indexes
    };
    *population = order
        .into_iter()
        .take(
            o.evolution
                .population_size
                .unwrap_or(o.trainer.population_size),
        )
        .map(|i| population[i].clone())
        .collect();
}
pub fn evolve(p: &Problem, o: &Options) -> Outcome {
    evolve_observed(p, o, None, &[], &mut |_, _| {})
}
/// Diagnostic standalone refinement with the same incumbent checks as improve.
pub fn evolve_from(p: &Problem, o: &Options, s: &Schedule) -> Outcome {
    let start = Instant::now();
    search::limits(o)?;
    check_config(o)?;
    let v = crate::validate::validate(p, s);
    if !v.valid {
        return Err(v.diagnostics);
    }
    let replay = s.replay.as_deref().ok_or_else(|| {
        vec![Diagnostic::new(
            "EVOLUTION_BASELINE",
            &p.id,
            "Incumbent needs replay metadata",
        )]
    })?;
    if o.route_choices
        .iter()
        .any(|(k, v)| s.route_choices.get(k) != Some(v))
        || o.mode_choices.iter().any(|(id, mode)| {
            !s.assignments
                .iter()
                .any(|a| &a.task == id && &a.mode == mode)
        })
        || o.conditional_choices.iter().any(|(id, row)| {
            row.iter().any(|(k, m)| {
                !s.assignments.iter().any(|a| {
                    &a.task == id
                        && a.activities
                            .iter()
                            .any(|v| v.id == format!("{id}:{k}") && &v.mode == m)
                })
            })
        })
        || !replay.decision_prefix.starts_with(&o.decision_prefix)
    {
        return Err(vec![Diagnostic::new(
            "EVOLUTION_BASELINE",
            &p.id,
            "Incumbent conflicts with requested commitments",
        )]);
    }
    let mut bounded = o.clone();
    if o.budget_ms > 0 {
        let remaining = o
            .budget_ms
            .saturating_sub(start.elapsed().as_millis() as u64);
        if remaining == 0 {
            let mut result = s.clone();
            result.evaluations = 0;
            result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
            result.search = Some(SearchReport {
                algorithm: "direct_schedule_ga".into(),
                stop_reason: "time_limit".into(),
                ..Default::default()
            });
            return Ok(result);
        }
        bounded.budget_ms = remaining;
    }
    let mut result = evolve_observed(p, &bounded, None, std::slice::from_ref(s), &mut |_, _| {})?;
    result.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    Ok(result)
}
pub fn evolve_customized(p: &Problem, o: &Options, extension: &dyn Customization) -> Outcome {
    evolve_observed(p, o, Some(extension), &[], &mut |_, _| {})
}

/// Internal seeds are already evaluated/validated by the portfolio. They consume
/// no additional decode evaluation; external callers use the validated improve API.
pub(crate) fn evolve_observed(
    p: &Problem,
    o: &Options,
    provided: Option<&dyn Customization>,
    seeds: &[Schedule],
    observe: &mut dyn FnMut(&Options, &Outcome),
) -> Outcome {
    let start = Instant::now();
    let max = search::limits(o)?;
    check_config(o)?;
    let language = crate::language::needs_lowering(p)
        .then(|| crate::language::lower(p))
        .transpose()?;
    let p = language.as_ref().unwrap_or(p);
    let owned = p
        .customization
        .as_ref()
        .map(crate::extensions::registered)
        .transpose()?;
    let extension = provided.or(owned.as_deref());
    let lowered = extension
        .map(|e| crate::extensions::lower(p, e))
        .transpose()?;
    let gp = lowered.as_ref().unwrap_or(p);
    crate::compile::compile(gp)?;
    let native = extension
        .map(|e| e.evolution_operators())
        .unwrap_or_default();
    let mut unique = HashSet::new();
    if native.len() > 32
        || native
            .iter()
            .any(|d| d.id.is_empty() || !unique.insert(&d.id))
    {
        return Err(vec![Diagnostic::new(
            "EVOLUTION_OPERATORS",
            &p.id,
            "Native operator IDs must be nonempty, unique and bounded to 32",
        )]);
    }
    let ids: Vec<String> = BUILTINS
        .iter()
        .map(|id| format!("apex:{id}@2"))
        .chain(native.iter().map(|d| {
            format!(
                "{}:{}@{}",
                extension.unwrap().id(),
                d.id,
                extension.unwrap().version()
            )
        }))
        .collect();
    let mut report = SearchReport {
        algorithm: "direct_schedule_ga".into(),
        workers: o.trainer.workers,
        operators: ids
            .into_iter()
            .enumerate()
            .map(|(index, id)| OperatorReport {
                id,
                kind: if index == 8 {
                    OperatorKind::Crossover
                } else if index < BUILTINS.len() {
                    OperatorKind::Mutation
                } else {
                    native[index - BUILTINS.len()].kind.clone()
                },
                ..Default::default()
            })
            .collect(),
        ..Default::default()
    };
    let mut population: Vec<_> = seeds
        .iter()
        .map(|s| {
            let mut s = s.clone();
            s.search = None;
            (from_schedule(gp, &s), s)
        })
        .collect();
    let mut best = population
        .iter()
        .map(|(_, s)| s.clone())
        .min_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
    for (_, s) in &population {
        search::archive(&mut report, s, o.trainer.archive_limit);
    }
    select(&mut population, o);
    let mut errors = vec![];
    let mut rng = Random(o.seed);
    let mut seen: HashSet<_> = population
        .iter()
        .map(|(g, _)| serde_json::to_string(g).unwrap())
        .collect();
    let mut exhausted = false;
    let time_up = || o.budget_ms > 0 && start.elapsed().as_millis() >= o.budget_ms as u128;
    loop {
        if report.evaluations >= max
            || report.evaluations > 0 && time_up()
            || report.generations > 0
                && o.trainer
                    .generations
                    .is_some_and(|g| report.generations >= g)
        {
            break;
        }
        let count = o
            .evolution
            .population_size
            .unwrap_or(o.trainer.population_size)
            .min(max - report.evaluations);
        let moves: Vec<_> = population
            .iter()
            .map(|(g, s)| Moves::new(gp, o, g, s))
            .collect();
        let mut proposals = vec![];
        let mut pending = vec![0; report.operators.len()];
        // Bound retries even for a singleton, exhausted space or no-op native hook.
        for _ in 0..count.saturating_mul(32) {
            if proposals.len() == count || report.evaluations > 0 && time_up() {
                break;
            }
            if population.is_empty() {
                let mut initial = o.clone();
                if report.evaluations + proposals.len() > 0 {
                    initial.seed = rng.next_u64();
                    initial.strategy = "random".into();
                    initial.queue_policy = None;
                }
                proposals.push((Ok(initial), None, None, false));
                continue;
            }
            let ai = parent_index(population.len(), &o.evolution, &mut rng);
            let mut bi = parent_index(population.len(), &o.evolution, &mut rng);
            if ai == bi && population.len() > 1 {
                bi = (ai + 1 + rng.index(population.len() - 1)) % population.len();
            }
            let (a, b) = (&population[ai], &population[bi]);
            let eligible: Vec<_> = (0..report.operators.len())
                .filter(|&i| moves[ai].eligible(i, &a.0, &b.0))
                .collect();
            if eligible.is_empty() {
                continue;
            }
            let index = arm(
                &report.operators,
                &pending,
                &eligible,
                &o.evolution,
                &mut rng,
            );
            let proposed = if index == 8 {
                Ok(crossover(gp, &a.0, &b.0, &mut rng))
            } else if index < BUILTINS.len() {
                Ok(mutate(gp, &a.0, &a.1, index, &moves[ai], &mut rng))
            } else {
                extension
                    .unwrap()
                    .evolve(&OperatorContext {
                        id: &native[index - BUILTINS.len()].id,
                        problem: gp,
                        parent: &a.0,
                        other: &b.0,
                        schedule: &a.1,
                        seed: rng.next_u64(),
                    })
                    .map(|g| g.unwrap_or_else(|| a.0.clone()))
                    .map_err(|e| vec![e])
            };
            let unchanged = proposed.as_ref().is_ok_and(|g| g == &a.0);
            let key = proposed
                .as_ref()
                .ok()
                .map(|g| serde_json::to_string(g).unwrap());
            let candidate = proposed.and_then(|g| options(gp, o, &g));
            if candidate.is_ok() && (unchanged || key.as_ref().is_some_and(|k| seen.contains(k))) {
                report.operators[index].skipped += 1;
                credit(&mut report.operators, index, &o.evolution, 0.0);
                continue;
            }
            if let Some(key) = key {
                seen.insert(key);
            }
            pending[index] += 1;
            let reference = if report.operators[index].kind == OperatorKind::Crossover
                && b.1.score < a.1.score
            {
                &b.1
            } else {
                &a.1
            };
            proposals.push((
                candidate,
                Some(index),
                Some((reference.score.clone(), reference.objective_values.clone())),
                unchanged,
            ));
        }
        if proposals.is_empty() {
            exhausted = !time_up();
            break;
        }
        // Generate the whole generation before workers run; results and UCB updates
        // are consumed in proposal order, independent of completion timing.
        for chunk in proposals.chunks(o.trainer.workers) {
            if report.evaluations > 0 && time_up() {
                break;
            }
            let jobs: Vec<_> = chunk
                .iter()
                .filter_map(|(o, _, _, _)| o.as_ref().ok().cloned())
                .collect();
            let mut results = search::parallel(p, &jobs, o.trainer.workers, extension).into_iter();
            for (candidate, index, parent, unchanged) in chunk {
                let result = match candidate {
                    Ok(_) => results.next().unwrap(),
                    Err(e) => Err(e.clone()),
                };
                report.evaluations += 1;
                observe(candidate.as_ref().unwrap_or(o), &result);
                if let Some(index) = index {
                    report.operators[*index].attempts += 1;
                    report.operators[*index].unchanged += usize::from(*unchanged);
                    if report.operators[*index].kind == OperatorKind::Crossover {
                        report.crossovers += 1;
                    } else {
                        report.mutations += 1;
                    }
                }
                let mut reward = 0.0;
                match result {
                    Ok(mut s) => {
                        if let Some(index) = index {
                            let r = &mut report.operators[*index];
                            r.evaluation_ms += s.elapsed_ms;
                            let active: HashSet<_> =
                                s.assignments.iter().map(|a| &a.task).collect();
                            let requested: Vec<_> = candidate
                                .as_ref()
                                .unwrap()
                                .decision_order
                                .iter()
                                .filter(|id| active.contains(id))
                                .collect();
                            r.reordered += usize::from(
                                requested
                                    != s.construction.iter().map(|d| &d.task).collect::<Vec<_>>(),
                            );
                            if let Some((score, objectives)) = parent {
                                let better = if o.trainer.selection == "pareto" {
                                    search::dominates(&s.objective_values, objectives)
                                } else {
                                    s.score < *score
                                };
                                r.improvements += usize::from(better);
                                if !unchanged {
                                    reward = if better {
                                        1.0
                                    } else if o.trainer.selection == "pareto"
                                        && !search::dominates(objectives, &s.objective_values)
                                        && objectives != &s.objective_values
                                    {
                                        0.5
                                    } else {
                                        0.0
                                    };
                                }
                            }
                        }
                        search::archive(&mut report, &s, o.trainer.archive_limit);
                        if best.as_ref().is_none_or(|b| s.score < b.score) {
                            search::progress(&mut report, &s, start);
                            best = Some(s.clone());
                        }
                        s.search = None;
                        let genome = from_schedule(gp, &s);
                        seen.insert(serde_json::to_string(&genome).unwrap());
                        population.push((genome, s));
                    }
                    Err(e) => {
                        report.failed_evaluations += 1;
                        errors = e;
                        if let Some(i) = index {
                            report.operators[*i].rejected += 1;
                            for e in &errors {
                                let codes = &mut report.operators[*i].rejection_codes;
                                let key = if codes.len() < 8 || codes.contains_key(&e.code) {
                                    e.code.clone()
                                } else {
                                    "OTHER".into()
                                };
                                *codes.entry(key).or_default() += 1;
                            }
                        }
                    }
                }
                if let Some(index) = index {
                    credit(&mut report.operators, *index, &o.evolution, reward);
                }
            }
        }
        report.generations += 1;
        select(&mut population, o);
    }
    report.stop_reason = if exhausted {
        "no_new_candidates"
    } else if report.evaluations >= max {
        "evaluation_limit"
    } else if time_up() {
        "time_limit"
    } else {
        "generation_limit"
    }
    .into();
    if let Some(mut s) = best {
        s.evaluations = report.evaluations;
        s.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        s.search = Some(report);
        Ok(s)
    } else {
        Err(errors)
    }
}
