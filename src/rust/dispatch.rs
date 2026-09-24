//! Shared construction path for greedy planning, evolution and prefix search.
use crate::{
    compile::Compiled,
    engine::{Decisions, allowed},
    model::*,
    rules,
};
use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
};

#[derive(PartialEq)]
struct Rank(f64, usize, usize);
impl Eq for Rank {}
impl Ord for Rank {
    fn cmp(&self, rhs: &Self) -> Ordering {
        rhs.2
            .cmp(&self.2)
            .then_with(|| rhs.0.total_cmp(&self.0))
            .then_with(|| rhs.1.cmp(&self.1))
    }
}
impl PartialOrd for Rank {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

pub fn decisions(c: &Compiled<'_>, options: &Options) -> Result<Decisions, Diagnostic> {
    decisions_with(c, options, None)
}
pub(crate) fn decisions_with(
    c: &Compiled<'_>,
    options: &Options,
    extension: Option<&dyn rules::Customization>,
) -> Result<Decisions, Diagnostic> {
    run(c, options, extension, false)
}
pub(crate) fn choices(
    c: &Compiled<'_>,
    options: &Options,
    extension: Option<&dyn rules::Customization>,
) -> Result<Vec<Decision>, Diagnostic> {
    run(c, options, extension, true).map(|d| d.next_choices)
}
fn run(
    c: &Compiled<'_>,
    options: &Options,
    extension: Option<&dyn rules::Customization>,
    stop_at_choices: bool,
) -> Result<Decisions, Diagnostic> {
    let p = c.problem;
    let governed = crate::policy::enabled(p, extension);
    let mut oracle = governed.then(|| crate::placement::Oracle::new(c, extension.is_some()));
    let direct: HashMap<_, _> = options
        .decision_order
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), i))
        .collect();
    let use_queues = direct.is_empty()
        && (options.strategy == "queues"
            || options.queue_policy.is_some()
            || p.queue_policy.is_some());
    let definitions = crate::queues::definitions(p, extension);
    let policy = options
        .queue_policy
        .clone()
        .or_else(|| p.queue_policy.clone())
        .unwrap_or_else(|| crate::queues::default_policy(p, &definitions));
    crate::queues::check(p, &policy, &definitions)?;
    let remaining = if use_queues {
        crate::queues::remaining(c)
    } else {
        vec![0.0; p.tasks.len()]
    };
    let mut counts: Vec<_> = c.dispatch_predecessors.iter().map(Vec::len).collect();
    let mut stages: Vec<_> = p.tasks.iter().map(|t| t.stage.as_str()).collect();
    stages.sort_by_key(|s| (s.parse::<i64>().unwrap_or(i64::MAX), *s));
    stages.dedup();
    let stage_ranks: HashMap<_, _> = stages
        .into_iter()
        .enumerate()
        .map(|(i, s)| (s, i + 1))
        .collect();
    let rank = |i: usize| {
        let t = &p.tasks[i];
        let fixed = p.locks.iter().find_map(|l| {
            if let Lock::Start { task, at } = l {
                (task == &t.id).then_some(*at)
            } else {
                None
            }
        });
        Rank(
            if t.execution.is_some() {
                -2e15
            } else if let Some(at) = fixed {
                -1e15 + at as f64
            } else if !direct.is_empty() {
                direct
                    .get(t.id.as_str())
                    .copied()
                    .unwrap_or(direct.len() + i) as f64
            } else {
                rules::rank_urgent(p, t, options, c.urgency[i].due, c.urgency[i].priority)
                    + extension.map_or(0.0, |e| e.dispatch_rank(p, t))
            },
            i,
            if !use_queues || t.execution.is_some() || fixed.is_some() {
                0
            } else {
                stage_ranks[t.stage.as_str()]
            },
        )
    };
    let mut heap = BinaryHeap::new();
    for (i, n) in counts.iter().enumerate() {
        if *n == 0 {
            heap.push(rank(i));
        }
    }
    let mut result = Decisions {
        order: Vec::with_capacity(p.tasks.len()),
        modes: vec![0; p.tasks.len()],
        next_choices: vec![],
        evidence: None,
    };
    if governed {
        let mut pinned = options.clone();
        pinned.decision_prefix.clear();
        result.evidence = Some(crate::policy::Evidence {
            version: "apex.dispatch.v1".into(),
            model_hash: crate::policy::fingerprint(p, extension),
            options: Box::new(pinned),
            decisions: vec![],
            steps: vec![],
            omitted_steps: 0,
            exact_probes: 0,
            direct_placements: 0,
        });
    }
    let mut tails = vec![0.0; p.resources.len()];
    let mut ready_times = vec![0.0; p.tasks.len()];
    let mut active = HashMap::<&str, &str>::new();
    let mut history = HashMap::<String, Vec<usize>>::new();
    while !heap.is_empty() {
        let forced = options.decision_prefix.get(result.order.len());
        let eligible = |i: usize, mi: usize| {
            let t = &p.tasks[i];
            let m = &t.modes[mi];
            allowed(p, t, m)
                && options.mode_choices.get(&t.id).is_none_or(|id| id == &m.id)
                && active.get(m.primary.as_str()).is_none_or(|id| *id == t.id)
        };
        let mut selected = None;
        let mut candidate_estimate = None;
        let mut removed = vec![];
        let mut step = None;
        if governed {
            let mut candidates = vec![];
            while let Some(r) = heap.pop() {
                let i = r.1;
                removed.push(r);
                for mi in 0..p.tasks[i].modes.len() {
                    if eligible(i, mi) {
                        candidates.push(crate::queues::candidate(
                            c,
                            i,
                            mi,
                            &crate::queues::DispatchState {
                                tails: &tails,
                                ready: &ready_times,
                                remaining: &remaining,
                                history: &history,
                                definitions: &definitions,
                                extension,
                            },
                        )?);
                    }
                }
            }
            step = Some(crate::policy::filter(
                c,
                &result,
                &mut candidates,
                extension,
                oracle.as_mut().unwrap(),
            )?);
            // Filter the complete ready pool before applying construction stages or Q windows.
            if use_queues {
                let stage = candidates.iter().map(|v| rank(v.task).2).min().unwrap();
                candidates.retain(|v| rank(v.task).2 == stage);
            }
            let scores = if use_queues {
                crate::queues::scores(p, &mut candidates, &policy)
            } else {
                candidates
                    .iter()
                    .map(|v| {
                        if direct.is_empty() && options.strategy == "setup" {
                            v.values["setup_penalty"] * 1e6 + rank(v.task).0
                        } else {
                            rank(v.task).0
                        }
                    })
                    .collect()
            };
            let mut indexes: Vec<_> = (0..candidates.len()).collect();
            indexes.sort_by(|&a, &b| {
                scores[a]
                    .total_cmp(&scores[b])
                    .then(candidates[a].task.cmp(&candidates[b].task))
                    .then_with(|| {
                        (candidates[a].start + candidates[a].work)
                            .total_cmp(&(candidates[b].start + candidates[b].work))
                    })
                    .then(candidates[a].mode.cmp(&candidates[b].mode))
            });
            let to_decision = |v: &crate::queues::Candidate| Decision {
                task: p.tasks[v.task].id.clone(),
                mode: p.tasks[v.task].modes[v.mode].id.clone(),
            };
            if result.order.len() == options.decision_prefix.len() {
                result.next_choices = indexes
                    .iter()
                    .take(options.plus.branching)
                    .map(|&i| to_decision(&candidates[i]))
                    .collect();
                if stop_at_choices {
                    return Ok(result);
                }
            }
            let index = if let Some(forced) = forced {
                indexes.iter().copied().find(|&i| &to_decision(&candidates[i]) == forced).ok_or_else(|| Diagnostic::new("DISPATCH_PREFIX", &forced.task, "Prefix decision violates the active policy, stage, readiness or mode/resource commitments"))?
            } else {
                let offset = if use_queues && options.top_k > 1 {
                    crate::domain::mixed_seed(options.seed, &result.order.len().to_string())
                        as usize
                        % options.top_k.min(indexes.len())
                } else {
                    0
                };
                indexes[offset]
            };
            let best = &candidates[index];
            selected = Some((best.task, best.mode));
            candidate_estimate = Some((best.start, best.work));
        } else if let Some(forced) = forced {
            while let Some(r) = heap.pop() {
                let matches = p.tasks[r.1].id == forced.task;
                removed.push(r);
                if matches {
                    break;
                }
            }
            if let Some(r) = removed.last().filter(|r| p.tasks[r.1].id == forced.task)
                && let Some(mi) = p.tasks[r.1]
                    .modes
                    .iter()
                    .position(|m| m.id == forced.mode)
                    .filter(|mi| eligible(r.1, *mi))
            {
                selected = Some((r.1, mi));
            }
            if selected.is_none() {
                return Err(Diagnostic::new(
                    "DECISION_PREFIX",
                    &forced.task,
                    "Prefix decision is not ready or violates resource/mode/block choices",
                ));
            }
        } else if use_queues || (direct.is_empty() && options.strategy == "setup") {
            let limit = if policy.candidate_limit == 0 {
                usize::MAX
            } else {
                policy.candidate_limit
            };
            let mut candidates = vec![];
            while let Some(r) = heap.pop() {
                let i = r.1;
                let is_fixed = r.0 < -1e14;
                if use_queues
                    && !candidates.is_empty()
                    && removed.first().is_some_and(|first: &Rank| first.2 != r.2)
                {
                    heap.push(r);
                    break;
                }
                removed.push(r);
                for mi in 0..p.tasks[i].modes.len() {
                    if eligible(i, mi) {
                        let candidate = crate::queues::candidate(
                            c,
                            i,
                            mi,
                            &crate::queues::DispatchState {
                                tails: &tails,
                                ready: &ready_times,
                                remaining: &remaining,
                                history: &history,
                                definitions: &definitions,
                                extension,
                            },
                        )?;
                        candidates.push(candidate);
                    }
                }
                if !candidates.is_empty() && (is_fixed || removed.len() >= limit || heap.is_empty())
                {
                    break;
                }
            }
            if !candidates.is_empty() {
                // Stages are explicit construction phases; dependency readiness remains authoritative.
                let stage_key =
                    |stage: &str| (stage.parse::<i64>().unwrap_or(i64::MAX), stage.to_owned());
                let stage = candidates
                    .iter()
                    .map(|v| stage_key(&p.tasks[v.task].stage))
                    .min()
                    .unwrap();
                candidates.retain(|v| stage_key(&p.tasks[v.task].stage) == stage);
                let scores = if use_queues {
                    crate::queues::scores(p, &mut candidates, &policy)
                } else {
                    candidates
                        .iter()
                        .map(|v| v.values["setup_penalty"] * 1e6 + rank(v.task).0)
                        .collect()
                };
                let mut indexes: Vec<_> = (0..candidates.len()).collect();
                indexes.sort_by(|a, b| {
                    scores[*a]
                        .total_cmp(&scores[*b])
                        .then(candidates[*a].task.cmp(&candidates[*b].task))
                        .then(candidates[*a].mode.cmp(&candidates[*b].mode))
                });
                if result.order.len() == options.decision_prefix.len() {
                    result.next_choices = indexes
                        .iter()
                        .take(options.plus.branching)
                        .map(|j| {
                            let c = &candidates[*j];
                            Decision {
                                task: p.tasks[c.task].id.clone(),
                                mode: p.tasks[c.task].modes[c.mode].id.clone(),
                            }
                        })
                        .collect();
                }
                if stop_at_choices && result.order.len() == options.decision_prefix.len() {
                    return Ok(result);
                }
                let offset = if use_queues && options.top_k > 1 {
                    (crate::domain::mixed_seed(options.seed, &result.order.len().to_string())
                        as usize)
                        % options.top_k.min(indexes.len())
                } else {
                    0
                };
                let best = &candidates[indexes[offset]];
                selected = Some((best.task, best.mode));
                candidate_estimate = Some((best.start, best.work));
            }
        } else {
            while let Some(r) = heap.pop() {
                let i = r.1;
                let t = &p.tasks[i];
                removed.push(r);
                let best = (0..t.modes.len())
                    .filter(|mi| eligible(i, *mi))
                    .min_by(|a, b| {
                        let cost = |mi: usize| {
                            tails[c.resources[t.modes[mi].primary.as_str()]].max(t.release as f64)
                                + t.modes[mi]
                                    .phases
                                    .iter()
                                    .map(|p| p.work.unwrap_or(0.0))
                                    .sum::<f64>()
                        };
                        if matches!(options.strategy.as_str(), "random" | "weighted")
                            && !options.mode_choices.contains_key(&t.id)
                        {
                            let key = |mi: usize| {
                                crate::domain::mixed_seed(
                                    options.seed,
                                    &format!("{}:{}", t.id, t.modes[mi].id),
                                )
                            };
                            key(*a).cmp(&key(*b))
                        } else {
                            cost(*a).total_cmp(&cost(*b))
                        }
                    });
                if let Some(mi) = best {
                    selected = Some((i, mi));
                    break;
                }
            }
        }
        let Some((i, mi)) = selected else {
            return Err(Diagnostic::new(
                "NO_CANDIDATE",
                &p.id,
                "No eligible ready task/mode for this construction",
            ));
        };
        for r in removed {
            if r.1 != i {
                heap.push(r);
            }
        }
        if let Some(e) = &mut result.evidence {
            let decision = Decision {
                task: p.tasks[i].id.clone(),
                mode: p.tasks[i].modes[mi].id.clone(),
            };
            e.decisions.push(decision.clone());
            e.exact_probes = oracle.as_ref().unwrap().decoded_prefixes;
            e.direct_placements = oracle.as_ref().unwrap().direct_placements;
            if let Some(mut step) = step {
                step.selected = Some(decision);
                if e.steps.len() < 256 {
                    e.steps.push(step);
                } else {
                    e.omitted_steps += 1;
                }
            }
        }
        let t = &p.tasks[i];
        let m = &t.modes[mi];
        result.order.push(i);
        result.modes[i] = mi;
        history.entry(m.primary.clone()).or_default().push(i);
        let (start, work) = candidate_estimate.unwrap_or_else(|| {
            let pred = c.predecessors[i]
                .iter()
                .map(|(j, lag, _)| ready_times[*j] + *lag as f64)
                .fold(t.release as f64, f64::max);
            (
                tails[c.resources[m.primary.as_str()]].max(pred),
                m.phases.iter().map(|p| p.work.unwrap_or(0.0)).sum(),
            )
        });
        tails[c.resources[m.primary.as_str()]] = start + work;
        ready_times[i] = start + work;
        for lock in &p.locks {
            if let Lock::Order {
                resource,
                tasks,
                consecutive: true,
            } = lock
                && let Some(pos) = tasks.iter().position(|id| id == &t.id)
            {
                if let Some(next) = tasks.get(pos + 1) {
                    active.insert(resource, next);
                } else {
                    active.remove(resource.as_str());
                }
            }
        }
        for &next in &c.dispatch_successors[i] {
            counts[next] -= 1;
            if counts[next] == 0 {
                heap.push(rank(next));
            }
        }
    }
    if result.order.len() != p.tasks.len() || options.decision_prefix.len() > result.order.len() {
        return Err(Diagnostic::new(
            "DECISION_PREFIX",
            &p.id,
            "Incomplete or oversized decision prefix",
        ));
    }
    Ok(result)
}
