//! Shared budgets, evaluation, archives and deterministic randomness for XH, XT and XE.
use crate::{model::*, queues, rules::Customization, xg};
use std::{collections::BTreeMap, time::Instant};

pub struct Random(pub u64);
impl Random {
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        crate::domain::mixed_seed(self.0, "search")
    }
    pub fn index(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
    pub fn chance(&mut self, p: f64) -> bool {
        (self.next_u64() >> 11) as f64 / ((1u64 << 53) as f64) < p
    }
}
pub(crate) type Outcome = Result<Schedule, Vec<Diagnostic>>;
pub(crate) fn parallel(
    p: &Problem,
    jobs: &[Options],
    workers: usize,
    extension: Option<&dyn Customization>,
) -> Vec<Outcome> {
    if workers == 1 {
        return jobs.iter().map(|o| xg::evaluate(p, o, extension)).collect();
    }
    let mut ordered = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers.min(jobs.len()))
            .map(|worker| {
                scope.spawn(move || {
                    (worker..jobs.len())
                        .step_by(workers)
                        .map(|i| (i, xg::evaluate(p, &jobs[i], extension)))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("Scheduling worker panicked"))
            .collect::<Vec<_>>()
    });
    ordered.sort_by_key(|v| v.0);
    ordered.into_iter().map(|v| v.1).collect()
}
pub(crate) fn limits(o: &Options) -> Result<usize, Vec<Diagnostic>> {
    let t = &o.xh;
    if t.population_size < 2
        || t.population_size > 256
        || !(1..=64).contains(&t.workers)
        || !(1..=64).contains(&o.xt.workers)
        || !(1..=64).contains(&o.xt.branching)
        || !(1..=10000).contains(&o.xt.depth)
        || !o.xt.exploration.is_finite()
        || o.xt.exploration < 0.0
        || !t.mutation_rate.is_finite()
        || !(0.0..=1.0).contains(&t.mutation_rate)
        || !t.crossover_rate.is_finite()
        || !(0.0..=1.0).contains(&t.crossover_rate)
        || !["lexicographic", "pareto"].contains(&t.selection.as_str())
        || !(1..=256).contains(&t.archive_limit)
        || o.top_k == 0
        || o.top_k > 64
    {
        return Err(vec![Diagnostic::new(
            "SEARCH_OPTIONS",
            "search",
            "Invalid population, workers, probabilities, selection or exploration limits",
        )]);
    }
    let max = t.max_evaluations.unwrap_or(o.iterations);
    if max == 0 && o.budget_ms == 0 && t.generations.is_none() {
        return Err(vec![Diagnostic::new(
            "SEARCH_OPTIONS",
            "search",
            "At least one evaluation, time or generation limit is required",
        )]);
    }
    Ok(if max == 0 { usize::MAX } else { max })
}
pub(crate) fn time_up(start: Instant, o: &Options) -> bool {
    o.budget_ms > 0 && start.elapsed().as_millis() >= o.budget_ms as u128
}
pub fn dominates(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| a <= b)
        && a.iter().zip(b).any(|(a, b)| a < b)
}
fn crowding(values: &[Vec<f64>], front: &[usize]) -> BTreeMap<usize, f64> {
    let mut distances: BTreeMap<_, _> = front.iter().map(|i| (*i, 0.0)).collect();
    if front.is_empty() {
        return distances;
    }
    for (axis, _) in values[front[0]].iter().enumerate() {
        let mut sorted = front.to_vec();
        sorted.sort_by(|a, b| values[*a][axis].total_cmp(&values[*b][axis]).then(a.cmp(b)));
        let span = values[*sorted.last().unwrap()][axis] - values[sorted[0]][axis];
        if span == 0.0 {
            continue;
        }
        distances.insert(sorted[0], f64::INFINITY);
        distances.insert(*sorted.last().unwrap(), f64::INFINITY);
        for group in sorted.windows(3) {
            *distances.get_mut(&group[1]).unwrap() +=
                (values[group[2]][axis] - values[group[0]][axis]) / span;
        }
    }
    distances
}
pub(crate) fn pareto_order(values: &[Vec<f64>]) -> Vec<usize> {
    let mut remaining: Vec<_> = (0..values.len()).collect();
    let mut result = vec![];
    while !remaining.is_empty() {
        let mut front: Vec<_> = remaining
            .iter()
            .copied()
            .filter(|i| {
                !remaining
                    .iter()
                    .any(|j| dominates(&values[*j], &values[*i]))
            })
            .collect();
        let distance = crowding(values, &front);
        front.sort_by(|a, b| distance[b].total_cmp(&distance[a]).then(a.cmp(b)));
        remaining.retain(|i| !front.contains(i));
        result.extend(front);
    }
    result
}
pub(crate) fn archive(report: &mut SearchReport, s: &Schedule, limit: usize) {
    if report.archive.iter().any(|a| {
        a.objectives == s.objective_values || dominates(&a.objectives, &s.objective_values)
    }) {
        return;
    }
    report
        .archive
        .retain(|a| !dominates(&s.objective_values, &a.objectives));
    report.archive.push(SearchCandidate {
        metrics: s.metrics.clone(),
        score: s.score.clone(),
        objectives: s.objective_values.clone(),
        options: s.replay.as_deref().unwrap().clone(),
    });
    if report.archive.len() > limit {
        let values: Vec<_> = report
            .archive
            .iter()
            .map(|a| a.objectives.clone())
            .collect();
        let order = pareto_order(&values);
        report.archive = order
            .into_iter()
            .take(limit)
            .map(|i| report.archive[i].clone())
            .collect();
    }
}
pub(crate) fn queue_options(
    p: &Problem,
    o: &Options,
    extension: Option<&dyn Customization>,
) -> Options {
    let mut result = o.clone();
    result.strategy = "queues".into();
    result.weights = None;
    let defs = queues::definitions(p, extension);
    let mut policy = o
        .queue_policy
        .clone()
        .or_else(|| p.queue_policy.clone())
        .unwrap_or_else(|| queues::default_policy(p, &defs));
    let fallback = policy.stages.get("*").cloned().unwrap_or_default();
    for task in &p.tasks {
        policy
            .stages
            .entry(task.stage.clone())
            .or_insert_with(|| fallback.clone());
    }
    result.queue_policy = Some(policy);
    result
}
pub(crate) fn progress(report: &mut SearchReport, s: &Schedule, start: Instant) {
    let point = SearchProgress {
        evaluations: report.evaluations,
        elapsed_ms: start.elapsed().as_secs_f64() * 1000.0,
        score: s.score.clone(),
    };
    // Keep the first 127 improvements and the most recent incumbent.
    if report.progress.len() == 128 {
        report.progress.pop();
    }
    report.progress.push(point);
}
