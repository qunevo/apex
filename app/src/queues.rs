//! Auditable dispatch proxies. Final objectives always come from complete validated schedules.
use crate::{compile::Compiled, model::*, rules::Customization, xg};
use std::collections::{BTreeMap, HashMap, HashSet};

pub const STANDARD: &[&str] = &[
    "due",
    "earliest_start",
    "earliest_start_binary",
    "earliest_alternative_binary",
    "deadline_interval_fit",
    "conditional_work",
    "downtime",
    "downtime_binary",
    "most_downtime",
    "setup_penalty",
    "shortest_work",
    "longest_work",
    "remaining_work",
    "slack",
    "priority",
    "apparent_tardiness",
    "mode_cost",
    "fewest_alternatives",
    "most_alternatives",
];

pub struct Context<'a> {
    pub problem: &'a Problem,
    pub task: &'a Task,
    pub mode: &'a Mode,
    pub estimated_start: f64,
    pub remaining_work: f64,
    /// Derived chain urgency; task dates and business priorities remain unchanged.
    pub dispatch_urgency: crate::urgency::Urgency,
    pub sequence: &'a [usize],
}
#[derive(Clone)]
pub struct Candidate {
    pub task: usize,
    pub mode: usize,
    pub start: f64,
    pub work: f64,
    pub values: BTreeMap<String, f64>,
}

pub fn definitions(p: &Problem, extension: Option<&dyn Customization>) -> Vec<QueueDefinition> {
    let mut result = p.queue_definitions.clone();
    for rule in &p.rules {
        if let Rule::AttributeObjective {
            id,
            attribute,
            weight,
            ..
        } = rule
        {
            result.push(QueueDefinition {
                id: format!("attribute:{id}"),
                metric: id.clone(),
                attribute: attribute.clone(),
                weight: *weight,
                prefer_high: true,
            });
        }
    }
    // Readiness checks run before lowering. Advertise the same generated Q IDs
    // so an authored model can explicitly select its completion-objective proxy.
    for objective in &p.planning.objectives {
        result.push(QueueDefinition {
            id: format!("attribute:{}", objective.id),
            metric: objective.id.clone(),
            attribute: format!("__planning:{}", objective.id),
            weight: objective.weight,
            prefer_high: true,
        });
    }
    if let Some(e) = extension {
        for definition in e.queue_definitions(p) {
            if !result.iter().any(|d| d.id == definition.id) {
                result.push(definition);
            }
        }
    }
    result
}

pub fn objective_queues(metric: &str) -> &'static [(&'static str, f64)] {
    match metric {
        "weighted_tardiness" | "job_weighted_tardiness" | "order_weighted_tardiness" => &[
            ("apparent_tardiness", 4.0),
            ("slack", 1.0),
            ("earliest_start", 1.0),
        ],
        "tardiness"
        | "on_time_delivery"
        | "task_tardiness"
        | "job_tardiness"
        | "order_tardiness"
        | "task_late_count"
        | "job_late_count"
        | "order_late_count"
        | "task_on_time_delivery"
        | "job_on_time_delivery"
        | "order_on_time_delivery"
        | "task_max_tardiness"
        | "job_max_tardiness"
        | "order_max_tardiness" => &[("due", 1.0), ("slack", 4.0), ("shortest_work", 2.0)],
        "makespan" => &[
            ("earliest_start", 2.0),
            ("downtime", 2.0),
            ("remaining_work", 1.0),
        ],
        "flow_time" | "task_flow_time" | "job_flow_time" | "order_flow_time" => {
            &[("shortest_work", 3.0), ("earliest_start", 1.0)]
        }
        "transition_work" | "conditional_time" => &[("conditional_work", 4.0)],
        "setup_penalty" => &[("setup_penalty", 4.0)],
        "mode_cost" => &[("mode_cost", 4.0)],
        _ => &[],
    }
}

pub fn default_policy(p: &Problem, defs: &[QueueDefinition]) -> QueuePolicy {
    let mut weights: BTreeMap<String, f64> = STANDARD.iter().map(|s| ((*s).into(), 0.0)).collect();
    weights.insert("deadline_interval_fit".into(), 4.0);
    let native = p
        .customization
        .as_ref()
        .and_then(|r| crate::extensions::registered(r).ok());
    let extra = native.as_ref().map(|e| e.objectives(p)).unwrap_or_default();
    let objectives = crate::rules::objectives(p, &extra);
    for o in &objectives {
        if o.weight == 0.0 {
            continue;
        }
        for (q, w) in objective_queues(&o.metric) {
            let key = if o.maximize != crate::metrics::prefer_high(&o.metric) {
                format!("reverse:{q}")
            } else {
                (*q).into()
            };
            *weights.entry(key).or_default() += w * o.weight;
        }
    }
    for d in defs.iter() {
        let objective = objectives.iter().find(|o| o.metric == d.metric);
        let key = if objectives
            .iter()
            .any(|o| o.metric == d.metric && o.maximize)
        {
            format!("reverse:{}", d.id)
        } else {
            d.id.clone()
        };
        weights.insert(key, objective.map_or(d.weight, |o| o.weight));
    }
    QueuePolicy {
        stages: BTreeMap::from([("*".into(), weights)]),
        ..Default::default()
    }
}

pub fn unmapped(p: &Problem, defs: &[QueueDefinition]) -> Vec<String> {
    let native = p
        .customization
        .as_ref()
        .and_then(|r| crate::extensions::registered(r).ok());
    let extra = native.as_ref().map(|e| e.objectives(p)).unwrap_or_default();
    crate::rules::objectives(p, &extra)
        .iter()
        .filter(|o| {
            o.weight > 0.0
                && objective_queues(&o.metric).is_empty()
                && !defs.iter().any(|d| d.metric == o.metric)
        })
        .map(|o| o.metric.clone())
        .collect()
}

pub fn check(
    p: &Problem,
    policy: &QueuePolicy,
    defs: &[QueueDefinition],
) -> Result<(), Diagnostic> {
    let fail = |message: &str| Diagnostic::new("QUEUE_POLICY", &p.id, message);
    if !["minmax", "robust"].contains(&policy.normalization.as_str())
        || policy.candidate_limit > 1000000
    {
        return Err(fail(
            "Normalization must be minmax or robust; candidate_limit must be <= 1000000 (0 means full ready set)",
        ));
    }
    let mut names: HashSet<&str> = STANDARD.iter().copied().collect();
    for d in defs.iter() {
        if d.id.is_empty()
            || !names.insert(&d.id)
            || d.metric.is_empty()
            || !d.weight.is_finite()
            || d.weight < 0.0
        {
            return Err(fail(
                "Queue definitions need unique IDs, a metric and finite nonnegative weights",
            ));
        }
    }
    for weights in policy.stages.values() {
        if weights.is_empty()
            || weights.iter().any(|(id, w)| {
                !names.contains(id.strip_prefix("reverse:").unwrap_or(id))
                    || !w.is_finite()
                    || *w < 0.0
                    || *w > 1e6
            })
        {
            return Err(fail("Unknown queue or invalid stage weight"));
        }
    }
    Ok(())
}

pub fn normalize(values: &[f64], method: &str) -> Vec<f64> {
    if values.is_empty() {
        return vec![];
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let min = sorted[0];
    let max = *sorted.last().unwrap();
    if max == min {
        return vec![0.0; values.len()];
    }
    if method == "robust" {
        let quantile = |q: f64| {
            let i = q * (sorted.len() - 1) as f64;
            let lo = i.floor() as usize;
            let hi = i.ceil() as usize;
            sorted[lo] + (sorted[hi] - sorted[lo]) * (i - lo as f64)
        };
        let median = quantile(0.5);
        let iqr = quantile(0.75) - quantile(0.25);
        if iqr > 0.0 && (max - min) / iqr >= 1.5 {
            return values
                .iter()
                .map(|v| 1.0 / (1.0 + (-((v - median) / iqr)).exp()))
                .collect();
        }
    }
    values.iter().map(|v| (v - min) / (max - min)).collect()
}

/// Maximum remaining path work, computed once in reverse topological order.
pub fn remaining(c: &Compiled<'_>) -> Vec<f64> {
    let mut counts: Vec<_> = c.successors.iter().map(Vec::len).collect();
    let mut stack: Vec<_> = counts
        .iter()
        .enumerate()
        .filter_map(|(i, n)| (*n == 0).then_some(i))
        .collect();
    let work: Vec<_> = c
        .problem
        .tasks
        .iter()
        .map(|t| {
            t.modes
                .iter()
                .map(|m| m.phases.iter().map(|p| p.work.unwrap_or(0.0)).sum::<f64>())
                .fold(f64::INFINITY, f64::min)
        })
        .collect();
    let mut result = work.clone();
    while let Some(i) = stack.pop() {
        for &(pred, lag, _) in &c.predecessors[i] {
            result[pred] = result[pred].max(work[pred] + lag as f64 + result[i]);
            counts[pred] -= 1;
            if counts[pred] == 0 {
                stack.push(pred);
            }
        }
    }
    result
}
fn cond_work(items: &[Conditional]) -> f64 {
    items
        .iter()
        .map(|c| {
            c.modes
                .iter()
                .map(|m| m.phases.iter().map(|p| p.work.unwrap_or(0.0)).sum::<f64>())
                .fold(f64::INFINITY, f64::min)
        })
        .sum()
}

pub struct DispatchState<'a> {
    pub tails: &'a [f64],
    pub ready: &'a [f64],
    pub remaining: &'a [f64],
    pub history: &'a HashMap<String, Vec<usize>>,
    pub definitions: &'a [QueueDefinition],
    pub extension: Option<&'a dyn Customization>,
}
pub fn candidate(
    c: &Compiled<'_>,
    i: usize,
    mi: usize,
    state: &DispatchState<'_>,
) -> Result<Candidate, Diagnostic> {
    let DispatchState {
        tails,
        ready,
        remaining,
        history,
        definitions: defs,
        extension,
    } = state;
    let p = c.problem;
    let t = &p.tasks[i];
    let m = &t.modes[mi];
    let tail = tails[c.resources[m.primary.as_str()]];
    let pred = c.predecessors[i]
        .iter()
        .map(|(j, lag, _)| ready[*j] + *lag as f64)
        .fold(t.release as f64, f64::max);
    let mut start = tail.max(pred);
    // Calendar-aware lower estimate; exact allocation remains the decoder's responsibility.
    for phase in &m.phases {
        for req in &phase.requirements {
            let resource = &p.resources[c.resources[req.resource.as_str()]];
            if let Some(w) = resource
                .calendar
                .iter()
                .find(|w| w.end as f64 > start && w.capacity >= req.amount && w.rate > 0.0)
            {
                start = start.max(w.start as f64);
            } else {
                start = p.horizon as f64;
            }
        }
    }
    let work = m.phases.iter().map(|p| p.work.unwrap_or(0.0)).sum::<f64>();
    let sequence = history.get(&m.primary).map(Vec::as_slice).unwrap_or(&[]);
    let from = sequence.last().map_or(
        p.resources[c.resources[m.primary.as_str()]]
            .initial_state
            .as_str(),
        |i| p.tasks[*i].family.as_str(),
    );
    let transition = xg::transition(p, &m.primary, from, &t.family)?;
    let conditional = cond_work(&t.pre)
        + cond_work(&t.post)
        + cond_work(&m.pre)
        + cond_work(&m.post)
        + transition.map_or(0.0, |tr| {
            cond_work(&tr.next_pre) + cond_work(&tr.previous_post)
        });
    let penalty =
        transition.map_or(0.0, |tr| tr.penalty) + xg::pattern_penalty(p, &m.primary, sequence, i);
    let slack = c.urgency[i].due.unwrap_or(p.horizon) as f64 - start - remaining[i] - conditional;
    let latest = crate::rules::bounds(p, t).1;
    let deadline = if latest == p.horizon {
        p.horizon as f64
    } else {
        latest as f64 - start - remaining[i] - conditional
    };
    let mut values = BTreeMap::from([
        ("due".into(), c.urgency[i].due.unwrap_or(p.horizon) as f64),
        ("earliest_start".into(), start),
        ("deadline_interval_fit".into(), deadline),
        ("conditional_work".into(), conditional),
        ("downtime".into(), (start - tail).max(0.0)),
        ("most_downtime".into(), -(start - tail).max(0.0)),
        ("setup_penalty".into(), penalty),
        ("shortest_work".into(), work + conditional),
        ("longest_work".into(), -work - conditional),
        ("remaining_work".into(), -remaining[i]),
        ("slack".into(), slack),
        ("priority".into(), -c.urgency[i].priority),
        ("mode_cost".into(), m.cost),
        ("fewest_alternatives".into(), t.modes.len() as f64),
        ("most_alternatives".into(), -(t.modes.len() as f64)),
    ]);
    let context = Context {
        problem: p,
        task: t,
        mode: m,
        estimated_start: start,
        remaining_work: remaining[i],
        dispatch_urgency: c.urgency[i],
        sequence,
    };
    for d in defs.iter() {
        let value = extension
            .and_then(|e| e.queue_value(&d.id, &context))
            .or_else(|| t.attributes.get(&d.attribute).copied())
            .ok_or_else(|| {
                Diagnostic::new(
                    "QUEUE_VALUE",
                    &d.id,
                    "No native signal or declared task attribute is available",
                )
            })?;
        if !value.is_finite() {
            return Err(Diagnostic::new(
                "QUEUE_VALUE",
                &d.id,
                "Queue signal must be finite",
            ));
        }
        // Weighted completion has the exact Smith ratio on a single available machine.
        // With calendars, precedence or additional resources it remains a local proxy.
        let value = if d.id.starts_with("attribute:") {
            value / (work + conditional).max(1e-9)
        } else {
            value
        };
        values.insert(d.id.clone(), if d.prefer_high { -value } else { value });
    }
    Ok(Candidate {
        task: i,
        mode: mi,
        start,
        work: work + conditional,
        values,
    })
}

pub fn scores(p: &Problem, candidates: &mut [Candidate], policy: &QueuePolicy) -> Vec<f64> {
    let average_work =
        candidates.iter().map(|c| c.work).sum::<f64>() / candidates.len().max(1) as f64;
    let earliest = candidates
        .iter()
        .map(|c| c.start)
        .fold(f64::INFINITY, f64::min);
    let downtime = candidates
        .iter()
        .map(|c| c.values["downtime"])
        .fold(f64::INFINITY, f64::min);
    let mut per_job = HashMap::<&str, f64>::new();
    for c in candidates.iter() {
        let t = &p.tasks[c.task];
        let key = t.job.as_deref().unwrap_or(&t.id);
        per_job
            .entry(key)
            .and_modify(|v| *v = v.min(c.start))
            .or_insert(c.start);
    }
    for c in candidates.iter_mut() {
        let t = &p.tasks[c.task];
        let slack = (c.values["due"] - c.start - c.work).max(0.0);
        c.values.insert(
            "apparent_tardiness".into(),
            c.values["priority"] / c.work.max(1e-9)
                * (-slack / (2.0 * average_work.max(1e-9))).exp(),
        );
        let key = t.job.as_deref().unwrap_or(&t.id);
        c.values.insert(
            "earliest_start_binary".into(),
            if c.start == earliest { 0.0 } else { 1.0 },
        );
        c.values.insert(
            "earliest_alternative_binary".into(),
            if c.start == per_job[key] { 0.0 } else { 1.0 },
        );
        c.values.insert(
            "downtime_binary".into(),
            if c.values["downtime"] == downtime {
                0.0
            } else {
                1.0
            },
        );
    }
    let keys: std::collections::BTreeSet<_> = policy
        .stages
        .values()
        .flat_map(|w| {
            w.iter()
                .filter(|(_, v)| **v > 0.0)
                .map(|(id, _)| id.clone())
        })
        .collect();
    let mut totals = vec![0.0; candidates.len()];
    for key in keys {
        let raw = key.strip_prefix("reverse:").unwrap_or(&key);
        let sign = if raw == key { 1.0 } else { -1.0 };
        let values: Vec<_> = candidates.iter().map(|c| c.values[raw] * sign).collect();
        let normalized = normalize(&values, &policy.normalization);
        for (i, c) in candidates.iter().enumerate() {
            let weights = policy
                .stages
                .get(&p.tasks[c.task].stage)
                .or_else(|| policy.stages.get("*"));
            totals[i] += normalized[i] * weights.and_then(|w| w.get(&key)).copied().unwrap_or(0.0);
        }
    }
    totals
}
