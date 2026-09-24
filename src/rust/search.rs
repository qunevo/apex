//! Bounded evolutionary policy training and UCT-guided prefix exploration.
use crate::{engine, model::*, queues, rules::Customization};
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
type Outcome = Result<Schedule, Vec<Diagnostic>>;
pub(crate) fn parallel(
    p: &Problem,
    jobs: &[Options],
    workers: usize,
    extension: Option<&dyn Customization>,
) -> Vec<Outcome> {
    if workers == 1 {
        return jobs
            .iter()
            .map(|o| engine::evaluate(p, o, extension))
            .collect();
    }
    let mut ordered = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..workers.min(jobs.len()))
            .map(|worker| {
                scope.spawn(move || {
                    (worker..jobs.len())
                        .step_by(workers)
                        .map(|i| (i, engine::evaluate(p, &jobs[i], extension)))
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
    let t = &o.trainer;
    if t.population_size < 2
        || t.population_size > 256
        || !(1..=64).contains(&t.workers)
        || !(1..=64).contains(&o.plus.workers)
        || !(1..=64).contains(&o.plus.branching)
        || !(1..=10000).contains(&o.plus.depth)
        || !o.plus.exploration.is_finite()
        || o.plus.exploration < 0.0
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
fn time_up(start: Instant, o: &Options) -> bool {
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
pub fn crossover(a: &Options, b: &Options, rng: &mut Random) -> Options {
    let mut child = a.clone();
    if let (Some(ap), Some(bp)) = (&mut child.queue_policy, &b.queue_policy) {
        for (stage, weights) in &mut ap.stages {
            if rng.chance(0.5)
                && let Some(other) = bp.stages.get(stage)
            {
                *weights = other.clone();
            }
        }
        if rng.chance(0.5) {
            ap.normalization = bp.normalization.clone();
        }
    }
    for (task, choices) in &b.conditional_choices {
        if rng.chance(0.5) {
            child
                .conditional_choices
                .insert(task.clone(), choices.clone());
        }
    }
    for (route, alternative) in &b.route_choices {
        if rng.chance(0.5) {
            child
                .route_choices
                .insert(route.clone(), alternative.clone());
        }
    }
    child
}
pub fn mutate(p: &Problem, o: &mut Options, fixed: &Options, rng: &mut Random) {
    let possible = [0.0, 1.0, 2.0, 4.0, 8.0, 16.0];
    if let Some(policy) = &mut o.queue_policy {
        let keys: Vec<_> = policy
            .stages
            .iter()
            .flat_map(|(stage, weights)| {
                weights
                    .keys()
                    .filter(|id| id.as_str() != "deadline_interval_fit")
                    .map(|id| (stage.clone(), id.clone()))
            })
            .collect();
        if !keys.is_empty() {
            let (stage, key) = &keys[rng.index(keys.len())];
            let weight = policy.stages.get_mut(stage).unwrap().get_mut(key).unwrap();
            let index = (0..possible.len())
                .min_by(|a, b| {
                    f64::abs(possible[*a] - *weight).total_cmp(&f64::abs(possible[*b] - *weight))
                })
                .unwrap();
            let next = if rng.chance(0.6) {
                if index == 0 {
                    1
                } else if index == possible.len() - 1 || rng.chance(0.5) {
                    index - 1
                } else {
                    index + 1
                }
            } else {
                (index + 1 + rng.index(possible.len() - 1)) % possible.len()
            };
            *weight = possible[next];
        }
        if rng.chance(0.1) {
            policy.normalization = if policy.normalization == "minmax" {
                "robust"
            } else {
                "minmax"
            }
            .into();
        }
    }
    let routes: Vec<_> = p
        .routes
        .iter()
        .filter(|r| r.selected.is_none() && !fixed.route_choices.contains_key(&r.id))
        .collect();
    if !routes.is_empty() && rng.chance(0.3) {
        let r = routes[rng.index(routes.len())];
        o.route_choices.insert(
            r.id.clone(),
            r.alternatives[rng.index(r.alternatives.len())].id.clone(),
        );
    }
    let conditional = crate::conditionals::catalog(p);
    if p.material_policy.is_some() {
        let tasks: Vec<_> = p
            .tasks
            .iter()
            .filter(|t| {
                crate::material::differing_modes(t) && !fixed.mode_choices.contains_key(&t.id)
            })
            .collect();
        if !tasks.is_empty() && rng.chance(0.4) {
            let t = tasks[rng.index(tasks.len())];
            let modes: Vec<_> = t
                .modes
                .iter()
                .filter(|m| engine::allowed(p, t, m))
                .collect();
            if !modes.is_empty() {
                o.mode_choices
                    .insert(t.id.clone(), modes[rng.index(modes.len())].id.clone());
            }
        }
    }
    let genes: Vec<_> = conditional
        .iter()
        .flat_map(|(task, row)| {
            row.iter().filter_map(move |(id, modes)| {
                (modes.len() > 1
                    && !fixed
                        .conditional_choices
                        .get(task)
                        .is_some_and(|v| v.contains_key(id))
                    && !p
                        .tasks
                        .iter()
                        .any(|t| &t.id == task && t.conditional_modes.contains_key(id)))
                .then_some((task, id, modes))
            })
        })
        .collect();
    if !genes.is_empty() && rng.chance(0.6) {
        let (task, id, modes) = genes[rng.index(genes.len())];
        o.conditional_choices
            .entry(task.clone())
            .or_default()
            .insert(id.clone(), modes[rng.index(modes.len())].clone());
    }
    o.seed = rng.next_u64();
}

pub fn train(p: &Problem, o: &Options, provided: Option<&dyn Customization>) -> Outcome {
    train_observed(p, o, provided, &mut |_, _| {})
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
pub(crate) fn train_observed(
    p: &Problem,
    o: &Options,
    provided: Option<&dyn Customization>,
    observe: &mut dyn FnMut(&Options, &Outcome),
) -> Outcome {
    if crate::language::needs_lowering(p) {
        return train_observed(&crate::language::lower(p)?, o, provided, observe);
    }
    let owned = p
        .customization
        .as_ref()
        .map(crate::extensions::registered)
        .transpose()?;
    let extension = provided.or(owned.as_deref());
    let max = limits(o)?;
    let start = Instant::now();
    let lowered = extension
        .map(|e| crate::extensions::lower(p, e))
        .transpose()?;
    let genome_problem = lowered.as_ref().unwrap_or(p);
    let base = queue_options(p, o, extension);
    queues::check(
        p,
        base.queue_policy.as_ref().unwrap(),
        &queues::definitions(p, extension),
    )
    .map_err(|e| vec![e])?;
    let size = o.trainer.population_size.min(if max == usize::MAX {
        usize::MAX
    } else {
        (max / 2).max(2)
    });
    let mut rng = Random(o.seed);
    let mut population: Vec<(Options, Schedule)> = vec![];
    let mut best: Option<Schedule> = None;
    let mut errors = vec![];
    let mut report = SearchReport {
        algorithm: "evolutionary_queue_policies".into(),
        workers: o.trainer.workers,
        unmapped_objectives: queues::unmapped(p, &queues::definitions(p, extension)),
        ..Default::default()
    };
    let mut jobs = vec![o.clone(), base.clone()];
    for i in jobs.len()..size {
        let mut child = base.clone();
        // Diverse reference policies remain available alongside genetically varied Q policies.
        if i < 6 {
            child.strategy = ["shortest", "priority", "release", "setup"][i - 2].into();
            child.queue_policy = None;
        } else {
            for _ in 0..4 {
                mutate(genome_problem, &mut child, o, &mut rng);
            }
        }
        jobs.push(child);
    }
    let mut generation = 0;
    loop {
        let mut evaluated = vec![];
        for chunk in jobs.chunks(o.trainer.workers) {
            if report.evaluations >= max || report.evaluations > 0 && time_up(start, o) {
                break;
            }
            let chunk = &chunk[..chunk.len().min(max - report.evaluations)];
            for (genome, result) in
                chunk
                    .iter()
                    .zip(parallel(p, chunk, o.trainer.workers, extension))
            {
                report.evaluations += 1;
                observe(genome, &result);
                match result {
                    Ok(s) => {
                        archive(&mut report, &s, o.trainer.archive_limit);
                        if best.as_ref().is_none_or(|b| s.score < b.score) {
                            progress(&mut report, &s, start);
                            best = Some(s.clone());
                        }
                        evaluated.push((genome.clone(), s));
                    }
                    Err(e) => {
                        report.failed_evaluations += 1;
                        errors = e;
                    }
                }
            }
        }
        population.extend(evaluated);
        if o.trainer.selection == "pareto" {
            let values: Vec<_> = population
                .iter()
                .map(|(_, s)| s.objective_values.clone())
                .collect();
            let order = pareto_order(&values);
            population = order
                .into_iter()
                .take(size)
                .map(|i| population[i].clone())
                .collect();
        } else {
            population.sort_by(|a, b| a.1.score.partial_cmp(&b.1.score).unwrap());
            population.truncate(size);
        }
        report.generations = generation;
        if report.evaluations >= max {
            report.stop_reason = "evaluation_limit".into();
            break;
        }
        if time_up(start, o) {
            report.stop_reason = "time_limit".into();
            break;
        }
        if o.trainer
            .generations
            .is_some_and(|limit| generation >= limit)
        {
            report.stop_reason = "generation_limit".into();
            break;
        }
        generation += 1;
        jobs.clear();
        for _ in 0..size {
            let choose = |rng: &mut Random| {
                if population.is_empty() {
                    base.clone()
                } else {
                    population[rng.index(population.len()).min(rng.index(population.len()))]
                        .0
                        .clone()
                }
            };
            let parent = choose(&mut rng);
            let mut child = queue_options(p, &parent, extension);
            if rng.chance(o.trainer.crossover_rate) {
                let other = queue_options(p, &choose(&mut rng), extension);
                child = crossover(&child, &other, &mut rng);
                report.crossovers += 1;
            }
            if rng.chance(o.trainer.mutation_rate) {
                mutate(genome_problem, &mut child, o, &mut rng);
                report.mutations += 1;
            }
            jobs.push(child);
        }
    }
    if let Some(mut s) = best {
        s.evaluations = report.evaluations;
        s.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        s.search = Some(report);
        Ok(s)
    } else {
        Err(errors)
    }
}

struct Node {
    options: Options,
    children: Vec<usize>,
    untried: Vec<Options>,
    expanded: bool,
    visits: usize,
    reward: f64,
}
fn expand(
    p: &Problem,
    node: &Options,
    extension: Option<&dyn Customization>,
    errors: &mut Vec<Diagnostic>,
) -> Vec<Options> {
    match crate::material::branches(p, node, extension).and_then(|branches| {
        if branches.is_some() {
            Ok(branches)
        } else {
            crate::conditionals::branches(p, node, extension)
        }
    }) {
        Ok(Some(mut children)) => {
            children.reverse();
            return children;
        }
        Err(e) => {
            if errors.is_empty() {
                errors.extend(e);
            }
            return vec![];
        }
        Ok(None) => {}
    }
    if node.decision_prefix.len() >= node.plus.depth {
        return vec![];
    }
    let mut children = engine::next_choices(p, node, extension)
        .unwrap_or_else(|e| {
            if errors.is_empty() {
                errors.extend(e);
            }
            vec![]
        })
        .into_iter()
        .map(|d| {
            let mut o = node.clone();
            o.decision_prefix.push(d);
            o
        })
        .collect::<Vec<_>>();
    // Keep route choices in the tree as well; a fixed route or frozen task remains protected by resolution.
    if node.decision_prefix.is_empty() && node.route_choices.is_empty() {
        for route in &p.routes {
            if route.selected.is_none() {
                for alternative in &route.alternatives {
                    let mut o = node.clone();
                    o.route_choices
                        .insert(route.id.clone(), alternative.id.clone());
                    children.push(o);
                }
            }
            if children.len() >= node.plus.branching * 2 {
                break;
            }
        }
    }
    children.reverse();
    children
}
fn reward(reference: &[f64], score: &[f64]) -> f64 {
    for (a, b) in reference.iter().zip(score) {
        if a != b {
            return 1.0 / (1.0 + ((b - a) / (a.abs() + 1.0)).clamp(-30.0, 30.0).exp());
        }
    }
    0.5
}
pub fn plus(p: &Problem, o: &Options) -> Outcome {
    plus_customized(p, o, None)
}
pub fn plus_customized(p: &Problem, o: &Options, provided: Option<&dyn Customization>) -> Outcome {
    plus_observed(p, o, provided, None, &mut |_, _| {})
}
pub(crate) fn plus_observed(
    p: &Problem,
    o: &Options,
    provided: Option<&dyn Customization>,
    initial: Option<Schedule>,
    observe: &mut dyn FnMut(&Options, &Outcome),
) -> Outcome {
    if crate::language::needs_lowering(p) {
        return plus_observed(&crate::language::lower(p)?, o, provided, initial, observe);
    }
    if o.trainer.max_evaluations.unwrap_or(o.iterations) == 0 && o.budget_ms == 0 {
        return Err(vec![Diagnostic::new(
            "SEARCH_OPTIONS",
            "plus",
            "Fast Planner Plus requires an evaluation or time limit",
        )]);
    }
    let owned = p
        .customization
        .as_ref()
        .map(crate::extensions::registered)
        .transpose()?;
    let extension = provided.or(owned.as_deref());
    let max = limits(o)?;
    let start = Instant::now();
    let mut base = queue_options(p, o, extension);
    base.top_k = 2;
    let mut nodes = vec![Node {
        options: base,
        children: vec![],
        untried: vec![],
        expanded: false,
        visits: 0,
        reward: 0.0,
    }];
    // An internal handoff carries an already validated seed. Do not evaluate it twice.
    let seeded = initial.is_some();
    let initial = initial.map_or_else(|| engine::evaluate(p, o, extension), Ok);
    if !seeded {
        observe(o, &initial);
    }
    let mut errors = vec![];
    let mut best = None;
    let mut reference = vec![];
    let mut report = SearchReport {
        algorithm: "uct_prefix_rollouts".into(),
        workers: o.plus.workers,
        evaluations: usize::from(!seeded),
        unmapped_objectives: queues::unmapped(p, &queues::definitions(p, extension)),
        ..Default::default()
    };
    match initial {
        Ok(s) => {
            reference = s.score.clone();
            archive(&mut report, &s, o.trainer.archive_limit);
            progress(&mut report, &s, start);
            best = Some(s);
        }
        Err(e) => {
            errors = e;
            report.failed_evaluations += 1;
        }
    }
    while report.evaluations < max && !time_up(start, o) {
        let mut jobs = vec![];
        let mut paths = vec![];
        for worker in 0..o.plus.workers.min(max - report.evaluations) {
            let mut index = 0;
            let mut path = vec![0];
            loop {
                if !nodes[index].expanded {
                    nodes[index].untried = expand(p, &nodes[index].options, extension, &mut errors);
                    nodes[index].expanded = true;
                }
                if let Some(options) = nodes[index].untried.pop() {
                    let next = nodes.len();
                    nodes.push(Node {
                        options,
                        children: vec![],
                        untried: vec![],
                        expanded: false,
                        visits: 0,
                        reward: 0.0,
                    });
                    nodes[index].children.push(next);
                    index = next;
                    path.push(index);
                    break;
                }
                if nodes[index].children.is_empty() {
                    break;
                }
                let parent_visits = nodes[index].visits.max(1) as f64;
                index = *nodes[index]
                    .children
                    .iter()
                    .max_by(|a, b| {
                        let uct = |i: usize| {
                            if nodes[i].visits == 0 {
                                f64::INFINITY
                            } else {
                                nodes[i].reward / nodes[i].visits as f64
                                    + o.plus.exploration
                                        * (parent_visits.ln() / nodes[i].visits as f64).sqrt()
                            }
                        };
                        uct(**a).total_cmp(&uct(**b)).then(b.cmp(a))
                    })
                    .unwrap();
                path.push(index);
            }
            if nodes[index].visits > 0 {
                report.revisits += 1;
            }
            for i in &path {
                nodes[*i].visits += 1;
            }
            let mut options = nodes[index].options.clone();
            options.seed = o.seed.wrapping_add((report.evaluations + worker) as u64);
            jobs.push(options);
            paths.push(path);
        }
        for ((path, options), result) in
            paths
                .iter()
                .zip(&jobs)
                .zip(parallel(p, &jobs, o.plus.workers, extension))
        {
            report.evaluations += 1;
            observe(options, &result);
            let value = match result {
                Ok(s) => {
                    if reference.is_empty() {
                        reference = s.score.clone();
                    }
                    let value = reward(&reference, &s.score);
                    archive(&mut report, &s, o.trainer.archive_limit);
                    if best.as_ref().is_none_or(|b| s.score < b.score) {
                        progress(&mut report, &s, start);
                        best = Some(s);
                    }
                    value
                }
                Err(e) => {
                    errors = e;
                    report.failed_evaluations += 1;
                    0.0
                }
            };
            for i in path {
                nodes[*i].reward += value;
            }
        }
    }
    report.nodes = nodes.len();
    report.stop_reason = if report.evaluations >= max {
        "evaluation_limit"
    } else {
        "time_limit"
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
