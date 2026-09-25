//! XH: hypersearch over dispatch policies and supported model choices.
use crate::{model::*, queues, rules::Customization, search::*, xg};
use std::time::Instant;

pub const ID: &str = "XH";

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
            let modes: Vec<_> = t.modes.iter().filter(|m| xg::allowed(p, t, m)).collect();
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

pub fn search_with(p: &Problem, o: &Options, provided: Option<&dyn Customization>) -> Outcome {
    search_observed(p, o, provided, &mut |_, _| {})
}
pub(crate) fn search_observed(
    p: &Problem,
    o: &Options,
    provided: Option<&dyn Customization>,
    observe: &mut dyn FnMut(&Options, &Outcome),
) -> Outcome {
    if crate::language::needs_lowering(p) {
        return search_observed(&crate::language::lower(p)?, o, provided, observe);
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
    let size = o.xh.population_size.min(if max == usize::MAX {
        usize::MAX
    } else {
        (max / 2).max(2)
    });
    let mut rng = Random(o.seed);
    let mut population: Vec<(Options, Schedule)> = vec![];
    let mut best: Option<Schedule> = None;
    let mut errors = vec![];
    let mut report = SearchReport {
        algorithm: ID.into(),
        workers: o.xh.workers,
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
        for chunk in jobs.chunks(o.xh.workers) {
            if report.evaluations >= max || report.evaluations > 0 && time_up(start, o) {
                break;
            }
            let chunk = &chunk[..chunk.len().min(max - report.evaluations)];
            for (genome, result) in chunk
                .iter()
                .zip(parallel(p, chunk, o.xh.workers, extension))
            {
                report.evaluations += 1;
                observe(genome, &result);
                match result {
                    Ok(s) => {
                        archive(&mut report, &s, o.xh.archive_limit);
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
        if o.xh.selection == "pareto" {
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
        if o.xh.generations.is_some_and(|limit| generation >= limit) {
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
            if rng.chance(o.xh.crossover_rate) {
                let other = queue_options(p, &choose(&mut rng), extension);
                child = crossover(&child, &other, &mut rng);
                report.crossovers += 1;
            }
            if rng.chance(o.xh.mutation_rate) {
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

/// XH hypersearch with the registered customization, if any.
pub fn search(p: &Problem, o: &Options) -> Outcome {
    search_with(p, o, None)
}

/// XH hypersearch using a supplied native customization.
pub fn search_customized(p: &Problem, o: &Options, extension: &dyn Customization) -> Outcome {
    search_with(p, o, Some(extension))
}
