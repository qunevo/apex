use crate::model::*;
use std::collections::BTreeMap;

/// Compiled customization: lower to the common model so all planners and validation
/// see identical semantics. Unrepresentable rules must return an explicit error.
pub trait Customization: Send + Sync {
    fn id(&self) -> &str;
    fn version(&self) -> &str;
    /// Stable local operator IDs; the core adds extension identity and version.
    fn xe_operators(&self) -> Vec<crate::xe::OperatorDefinition> {
        vec![]
    }
    fn conditional_modes(&self, _p: &Problem) -> crate::conditionals::Catalog {
        Default::default()
    }
    /// Propose one chromosome. The core enforces commitments and validates the decoded result.
    fn evolve(
        &self,
        _context: &crate::xe::OperatorContext<'_>,
    ) -> Result<Option<crate::xe::Genome>, Diagnostic> {
        Ok(None)
    }
    fn compile(&self, input: &Problem) -> Result<Problem, Vec<Diagnostic>>;
    fn sequence(
        &self,
        _context: &crate::extensions::SequenceContext<'_>,
    ) -> Result<Vec<crate::extensions::TaskDecoration>, Vec<Diagnostic>> {
        Ok(vec![])
    }
    /// Opt in to mandatory filtering of the complete ready task/mode pool.
    fn has_dispatch_policy(&self) -> bool {
        false
    }
    fn dispatch_needs_placements(&self) -> bool {
        false
    }
    /// Sequence decorations may affect the previous neighbor, but must be valid on prefixes.
    fn supports_prefix_decoration(&self) -> bool {
        false
    }
    fn filter_candidates(
        &self,
        _context: &crate::policy::Context<'_>,
    ) -> Result<Vec<crate::policy::Rejection>, Diagnostic> {
        Ok(vec![])
    }
    fn dispatch_rank(&self, _p: &Problem, _t: &Task) -> f64 {
        0.0
    }
    fn queue_definitions(&self, _p: &Problem) -> Vec<QueueDefinition> {
        vec![]
    }
    /// Declare evaluated metrics so readiness and proxy inspection share their contract.
    fn objectives(&self, _p: &Problem) -> Vec<Objective> {
        vec![]
    }
    fn queue_value(&self, _id: &str, _context: &crate::queues::Context<'_>) -> Option<f64> {
        None
    }
    fn metrics(
        &self,
        _p: &Problem,
        _s: &Schedule,
    ) -> Result<Vec<crate::extensions::Metric>, Vec<Diagnostic>> {
        Ok(vec![])
    }
    fn validate(&self, _p: &Problem, _s: &Schedule) -> Vec<Diagnostic> {
        vec![]
    }
}

pub fn bounds(p: &Problem, t: &Task) -> (Time, Time) {
    let mut bounds = (t.release, t.deadline.unwrap_or(p.horizon));
    for rule in &p.rules {
        if let Rule::TaskWindow {
            task,
            earliest,
            latest,
            ..
        } = rule
            && task == &t.id
        {
            bounds.0 = bounds.0.max(*earliest);
            bounds.1 = bounds.1.min(*latest);
        }
    }
    bounds
}
/// Adding an attribute objective also creates its dispatch signal automatically.
pub fn rank(p: &Problem, t: &Task, options: &Options) -> f64 {
    rank_urgent(p, t, options, t.due, t.priority)
}
pub fn rank_urgent(
    p: &Problem,
    t: &Task,
    options: &Options,
    due: Option<Time>,
    priority: f64,
) -> f64 {
    let strategy = options.strategy.as_str();
    let seed = options.seed;
    let work = t
        .modes
        .iter()
        .map(|m| m.phases.iter().map(|p| p.work.unwrap_or(0.0)).sum::<f64>())
        .fold(f64::INFINITY, f64::min);
    let urgency: f64 = p
        .rules
        .iter()
        .map(|r| match r {
            Rule::AttributeObjective {
                attribute, weight, ..
            } => t.attributes.get(attribute).copied().unwrap_or(0.0) * weight,
            _ => 0.0,
        })
        .sum();
    match strategy {
        "weighted" => {
            let w = options
                .weights
                .as_ref()
                .expect("validated weighted dispatch");
            w.due * due.unwrap_or(p.horizon) as f64 / p.horizon as f64
                + w.work * work / p.horizon as f64
                + w.release * t.release as f64 / p.horizon as f64
                - w.priority * priority
                - w.urgency * urgency
        }
        "shortest" => work,
        "priority" => -priority,
        "release" => t.release as f64,
        "random" => {
            let mut x = seed ^ 0xcbf29ce484222325;
            for b in t.id.bytes() {
                x = (x ^ b as u64).wrapping_mul(0x100000001b3);
            }
            (x % 1_000_000) as f64
        }
        "objective" => {
            -p.rules
                .iter()
                .map(|r| match r {
                    Rule::AttributeObjective {
                        attribute, weight, ..
                    } => t.attributes.get(attribute).copied().unwrap_or(0.0) * weight,
                    _ => 0.0,
                })
                .sum::<f64>()
                - priority
        }
        _ => due.unwrap_or(p.horizon) as f64 / priority.max(0.001),
    }
}
pub fn metrics(p: &Problem, s: &Schedule) -> BTreeMap<String, f64> {
    let tasks: BTreeMap<_, _> = p.tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut m: BTreeMap<String, f64> = BTreeMap::from([
        ("makespan".into(), 0.0),
        ("weighted_tardiness".into(), 0.0),
        ("tardiness".into(), 0.0),
        ("on_time_delivery".into(), 0.0),
        ("flow_time".into(), 0.0),
        ("transition_work".into(), 0.0),
        ("mode_cost".into(), 0.0),
    ]);
    for a in &s.assignments {
        let Some(t) = tasks.get(a.task.as_str()) else {
            continue;
        };
        *m.get_mut("makespan").unwrap() =
            m["makespan"].max(a.activities.iter().map(|x| x.end).max().unwrap_or(a.ready) as f64);
        *m.get_mut("weighted_tardiness").unwrap() +=
            (a.ready - t.due.unwrap_or(p.horizon)).max(0) as f64 * t.priority;
        *m.get_mut("tardiness").unwrap() += (a.ready - t.due.unwrap_or(p.horizon)).max(0) as f64;
        if t.due.is_some_and(|due| a.ready <= due) {
            *m.get_mut("on_time_delivery").unwrap() += 1.0;
        }
        *m.get_mut("flow_time").unwrap() += (a.ready - t.release) as f64;
        *m.get_mut("mode_cost").unwrap() += t
            .modes
            .iter()
            .find(|m| m.id == a.mode)
            .map_or(0.0, |m| m.cost);
        *m.get_mut("transition_work").unwrap() += a
            .activities
            .iter()
            .filter(|a| a.role != "main" && a.role != "actual")
            .flat_map(|a| &a.segments)
            .map(|s| s.work)
            .sum::<f64>();
        for rule in &p.rules {
            if let Rule::AttributeObjective { id, attribute, .. } = rule {
                *m.entry(id.clone()).or_default() +=
                    a.ready as f64 * t.attributes.get(attribute).copied().unwrap_or(0.0);
            }
        }
    }
    m.insert("setup_penalty".into(), crate::xg::setup_penalty(p, s));
    let (jobs, orders) = crate::domain::completions(p, s);
    m.insert(
        "job_weighted_tardiness".into(),
        p.jobs
            .iter()
            .map(|j| {
                (jobs.get(&j.id).copied().unwrap_or(0) - j.due.unwrap_or(p.horizon)).max(0) as f64
                    * j.priority
            })
            .sum(),
    );
    m.insert(
        "order_weighted_tardiness".into(),
        p.orders
            .iter()
            .map(|o| {
                (orders.get(&o.id).copied().unwrap_or(0) - o.due.unwrap_or(p.horizon)).max(0) as f64
                    * o.priority
            })
            .sum(),
    );
    crate::metrics::add(p, s, &mut m);
    m
}
pub fn score(p: &Problem, metrics: &BTreeMap<String, f64>) -> Vec<f64> {
    score_with(p, metrics, &[])
}
pub fn score_with(p: &Problem, metrics: &BTreeMap<String, f64>, extra: &[Objective]) -> Vec<f64> {
    let mut levels = BTreeMap::<u32, f64>::new();
    for o in objectives(p, extra).iter().filter(|o| o.weight > 0.0) {
        *levels.entry(o.priority).or_default() +=
            metrics.get(&o.metric).copied().unwrap_or(0.0) * o.weight / o.scale
                * if o.maximize { -1.0 } else { 1.0 };
    }
    levels.into_values().collect()
}

pub fn objectives(p: &Problem, extra: &[Objective]) -> Vec<Objective> {
    let mut objectives = p.objectives.clone();
    if objectives.is_empty() {
        objectives.push(Objective {
            metric: if !p.orders.is_empty() {
                "order_weighted_tardiness"
            } else if !p.jobs.is_empty() {
                "job_weighted_tardiness"
            } else {
                "weighted_tardiness"
            }
            .into(),
            priority: 100,
            ..Default::default()
        });
        objectives.push(Objective {
            metric: "makespan".into(),
            priority: 101,
            ..Default::default()
        });
    }
    for rule in &p.rules {
        if let Rule::AttributeObjective {
            id,
            weight,
            priority,
            ..
        } = rule
            && !objectives.iter().any(|o| o.metric == *id)
        {
            objectives.push(Objective {
                metric: id.clone(),
                weight: *weight,
                priority: *priority,
                ..Default::default()
            });
        }
    }
    for o in p.native_objectives.iter().chain(extra) {
        if !objectives.iter().any(|x| x.metric == o.metric) {
            objectives.push(o.clone());
        }
    }
    objectives.sort_by(|a, b| a.priority.cmp(&b.priority).then(a.metric.cmp(&b.metric)));
    objectives
}
pub fn objective_values(
    p: &Problem,
    metrics: &BTreeMap<String, f64>,
    extra: &[Objective],
) -> Vec<f64> {
    objectives(p, extra)
        .iter()
        .filter(|o| o.weight > 0.0)
        .map(|o| {
            metrics.get(&o.metric).copied().unwrap_or(0.0) / o.scale
                * if o.maximize { -1.0 } else { 1.0 }
        })
        .collect()
}
