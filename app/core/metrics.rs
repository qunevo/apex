//! Additional KPI contracts: seconds, resource-capacity seconds and explicit fractions.
use crate::model::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const ORIGINAL: &[&str] = &[
    "weighted_tardiness",
    "tardiness",
    "on_time_delivery",
    "makespan",
    "flow_time",
    "transition_work",
    "mode_cost",
    "setup_penalty",
    "job_weighted_tardiness",
    "order_weighted_tardiness",
];
const GROUP: &[&str] = &[
    "tardiness",
    "max_tardiness",
    "late_count",
    "due_count",
    "on_time_delivery",
    "flow_time",
];
const RESOURCE: &[&str] = &[
    "makespan",
    "productive_time",
    "occupied_time",
    "available_time",
    "utilization",
    "utilization_7d",
];
const STAGE: &[&str] = &[
    "tardiness",
    "conditional_time",
    "processing_time",
    "utilization_mean",
    "utilization_stddev",
];
pub fn names(p: &Problem) -> Vec<String> {
    let mut out: Vec<String> = ORIGINAL.iter().map(|s| s.to_string()).collect();
    for group in ["task", "job", "order"] {
        for metric in GROUP {
            out.push(format!("{group}_{metric}"));
        }
    }
    out.extend(
        [
            "conditional_time",
            "conditional_mode_cost",
            "total_mode_cost",
        ]
        .map(String::from),
    );
    for r in &p.resources {
        for metric in RESOURCE {
            out.push(format!("resource:{}:{metric}", r.id));
        }
    }
    for stage in p.tasks.iter().map(|t| &t.stage).collect::<BTreeSet<_>>() {
        for metric in STAGE {
            out.push(format!("stage:{stage}:{metric}"));
        }
    }
    out
}
pub fn supported(p: &Problem, metric: &str) -> bool {
    if ORIGINAL.contains(&metric)
        || [
            "conditional_time",
            "conditional_mode_cost",
            "total_mode_cost",
        ]
        .contains(&metric)
    {
        return true;
    }
    if let Some((prefix, suffix)) = metric.split_once('_')
        && ["task", "job", "order"].contains(&prefix)
        && GROUP.contains(&suffix)
    {
        return true;
    }
    if let Some((id, suffix)) = metric
        .strip_prefix("resource:")
        .and_then(|s| s.rsplit_once(':'))
    {
        return RESOURCE.contains(&suffix) && p.resources.iter().any(|r| r.id == id);
    }
    if let Some((id, suffix)) = metric
        .strip_prefix("stage:")
        .and_then(|s| s.rsplit_once(':'))
    {
        return STAGE.contains(&suffix) && p.tasks.iter().any(|t| t.stage == id);
    }
    false
}
pub fn prefer_high(metric: &str) -> bool {
    metric == "on_time_delivery" || metric.ends_with("_on_time_delivery")
}
pub fn proxy_status(metric: &str) -> &'static str {
    if crate::queues::objective_queues(metric).is_empty() {
        "unmapped"
    } else if (metric.starts_with("job_") || metric.starts_with("order_"))
        && !ORIGINAL.contains(&metric)
        || metric.ends_with("max_tardiness")
        || metric.ends_with("late_count")
    {
        "experimental"
    } else {
        "heuristic"
    }
}
pub fn catalog(p: &Problem) -> Vec<serde_json::Value> {
    names(p).into_iter().map(|id| {
        let unit=if id.contains("utilization") || (id.ends_with("on_time_delivery") && id!="on_time_delivery") {"fraction"}
            else if id.ends_with("count") || id=="on_time_delivery" {"count"}
            else if id.contains("cost") || id=="setup_penalty" {"configured_cost"}
            else if id=="transition_work" {"work_units"}
            else if id.ends_with("weighted_tardiness") {"priority_seconds"}
            else if id.starts_with("resource:") && !id.ends_with("makespan") {"capacity_seconds"}
            else {"seconds"};
        serde_json::json!({"id":id,"unit":unit,"standard_proxies":crate::queues::objective_queues(&id),"proxy_status":proxy_status(&id)})
    }).collect()
}

fn group(
    out: &mut BTreeMap<String, f64>,
    prefix: &str,
    rows: impl Iterator<Item = (Time, Option<Time>, Time)>,
) {
    let (mut late, mut count, mut total, mut max, mut flow) = (0.0, 0.0, 0.0, 0.0f64, 0.0);
    for (end, due, release) in rows {
        flow += (end - release) as f64;
        if let Some(due) = due {
            count += 1.0;
            let delay = (end - due).max(0) as f64;
            total += delay;
            max = max.max(delay);
            late += f64::from(delay > 0.0);
        }
    }
    for (key, value) in [
        ("tardiness", total),
        ("max_tardiness", max),
        ("late_count", late),
        ("due_count", count),
        (
            "on_time_delivery",
            if count > 0.0 {
                (count - late) / count
            } else {
                1.0
            },
        ),
        ("flow_time", flow),
    ] {
        out.insert(format!("{prefix}_{key}"), value);
    }
}

pub fn add(p: &Problem, s: &Schedule, out: &mut BTreeMap<String, f64>) {
    let tasks: HashMap<_, _> = p.tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    let (jobs, orders) = crate::domain::completions(p, s);
    let mut releases = BTreeMap::<&str, Time>::new();
    for t in &p.tasks {
        if let Some(job) = t.job.as_deref() {
            releases
                .entry(job)
                .and_modify(|v| *v = (*v).min(t.release))
                .or_insert(t.release);
        }
    }
    group(
        out,
        "task",
        s.assignments.iter().filter_map(|a| {
            tasks
                .get(a.task.as_str())
                .map(|t| (a.ready, t.due, t.release))
        }),
    );
    group(
        out,
        "job",
        p.jobs.iter().filter_map(|j| {
            jobs.get(&j.id)
                .map(|end| (*end, j.due, *releases.get(j.id.as_str()).unwrap_or(&0)))
        }),
    );
    group(
        out,
        "order",
        p.orders.iter().filter_map(|o| {
            orders.get(&o.id).map(|end| {
                (
                    *end,
                    o.due,
                    o.jobs
                        .iter()
                        .filter_map(|id| releases.get(id.as_str()))
                        .copied()
                        .min()
                        .unwrap_or(0),
                )
            })
        }),
    );
    let mut stage = BTreeMap::<&str, [f64; 3]>::new();
    let mut stage_resources = BTreeMap::<&str, BTreeSet<&str>>::new();
    let mut resources: HashMap<&str, (Time, f64, f64, f64)> = p
        .resources
        .iter()
        .map(|r| (r.id.as_str(), (0, 0.0, 0.0, 0.0)))
        .collect();
    let (mut conditional_time, mut conditional_cost) = (0.0, 0.0);
    for a in &s.assignments {
        let Some(t) = tasks.get(a.task.as_str()) else {
            continue;
        };
        let Some(main) = t.modes.iter().find(|m| m.id == a.mode) else {
            continue;
        };
        let stats = stage.entry(&t.stage).or_default();
        stats[0] += (a.ready - t.due.unwrap_or(p.horizon)).max(0) as f64;
        stage_resources
            .entry(&t.stage)
            .or_default()
            .insert(&a.primary);
        for r in a
            .activities
            .iter()
            .flat_map(|a| &a.reservations)
            .chain(&a.retained)
        {
            if let Some(v) = resources.get_mut(r.resource.as_str()) {
                v.0 = v.0.max(r.end);
                v.3 += (r.end - r.start) as f64 * r.amount;
            }
        }
        for activity in &a.activities {
            let duration = activity
                .segments
                .iter()
                .map(|seg| (seg.end - seg.start) as f64)
                .sum::<f64>();
            if activity.role == "main" || activity.role == "actual" {
                stats[2] += duration;
                for seg in &activity.segments {
                    if let Some(phase) = main.phases.iter().find(|ph| ph.id == seg.phase) {
                        for req in &phase.requirements {
                            if let Some(v) = resources.get_mut(req.resource.as_str()) {
                                v.1 += (seg.end - seg.start) as f64 * req.amount;
                                v.2 += (seg.end.min(604800) - seg.start.min(604800)).max(0) as f64
                                    * req.amount;
                            }
                        }
                    }
                }
            } else {
                conditional_time += duration;
                stats[1] += duration;
                if let Some(cost) = conditional_cost_for(p, t, main, activity) {
                    conditional_cost += cost;
                }
            }
        }
    }
    out.insert("conditional_time".into(), conditional_time);
    out.insert("conditional_mode_cost".into(), conditional_cost);
    out.insert(
        "total_mode_cost".into(),
        out.get("mode_cost").copied().unwrap_or(0.0) + conditional_cost,
    );
    for r in &p.resources {
        let (end, busy, busy7, occupied) = resources[r.id.as_str()];
        let available = |stop: Time| {
            r.calendar
                .iter()
                .filter(|w| w.rate > 0.0)
                .map(|w| (w.end.min(stop) - w.start.min(stop)).max(0) as f64 * w.capacity)
                .sum::<f64>()
        };
        let cap = available(end);
        let cap7 = available(p.horizon.min(604800));
        for (key, value) in [
            ("makespan", end as f64),
            ("productive_time", busy),
            ("occupied_time", occupied),
            ("available_time", cap),
            ("utilization", if cap > 0.0 { busy / cap } else { 0.0 }),
            (
                "utilization_7d",
                if cap7 > 0.0 { busy7 / cap7 } else { 0.0 },
            ),
        ] {
            out.insert(format!("resource:{}:{key}", r.id), value);
        }
    }
    for name in p
        .tasks
        .iter()
        .map(|t| t.stage.as_str())
        .collect::<BTreeSet<_>>()
    {
        let v = stage.get(name).copied().unwrap_or_default();
        let utils: Vec<_> = stage_resources
            .get(name)
            .into_iter()
            .flatten()
            .map(|id| out[&format!("resource:{id}:utilization")])
            .collect();
        let mean = utils.iter().sum::<f64>() / utils.len().max(1) as f64;
        let std = (utils.iter().map(|v| (v - mean).powi(2)).sum::<f64>()
            / utils.len().max(1) as f64)
            .sqrt();
        for (key, value) in [
            ("tardiness", v[0]),
            ("conditional_time", v[1]),
            ("processing_time", v[2]),
            ("utilization_mean", mean),
            ("utilization_stddev", std),
        ] {
            out.insert(format!("stage:{name}:{key}"), value);
        }
    }
}

fn conditional_cost_for(p: &Problem, t: &Task, main: &Mode, a: &Activity) -> Option<f64> {
    let key = a.id.strip_prefix(&format!("{}:", t.id))?;
    let mut entries: Vec<(String, &Conditional)> = vec![];
    for (prefix, conds) in [
        ("pre", &t.pre),
        ("post", &t.post),
        ("pre", &main.pre),
        ("post", &main.post),
    ] {
        entries.extend(conds.iter().map(|c| (format!("{prefix}:{}", c.id), c)));
    }
    if let Some(e) = &t.execution {
        entries.extend(e.restart.iter().map(|c| (format!("restart:{}", c.id), c)));
    }
    for tr in &p.transitions {
        for (role, conds) in [("pre", &tr.next_pre), ("post", &tr.previous_post)] {
            entries.extend(
                conds
                    .iter()
                    .map(|c| (format!("transition:{}:{role}:{}", tr.id, c.id), c)),
            );
        }
    }
    for r in &p.rules {
        if let Rule::SequencePattern {
            id,
            previous_post,
            next_pre,
            ..
        } = r
        {
            for (role, conds) in [("pre", next_pre), ("post", previous_post)] {
                entries.extend(
                    conds
                        .iter()
                        .map(|c| (format!("pattern:{id}:{role}:{}", c.id), c)),
                );
            }
        }
    }
    entries
        .into_iter()
        .find(|(id, _)| id == key)
        .and_then(|(_, c)| c.modes.iter().find(|m| m.id == a.mode).map(|m| m.cost))
}
