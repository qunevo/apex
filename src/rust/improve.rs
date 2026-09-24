//! Budgeted Trainer -> Plus -> direct schedule evolution portfolio.
use crate::{model::*, rules::Customization, search, validate};
use std::time::Instant;

type Outcome = Result<Schedule, Vec<Diagnostic>>;
struct Seed {
    options: Options,
    schedule: Schedule,
}
struct Portfolio {
    start: Instant,
    report: SearchReport,
    best: Option<Schedule>,
    seeds: Vec<Seed>,
    reference: Option<Seed>,
    errors: Vec<Diagnostic>,
    schedules: Vec<Schedule>,
    population_size: usize,
}
impl Portfolio {
    fn accept(&mut self, s: &Schedule, limit: usize) {
        search::archive(&mut self.report, s, limit);
        if !self.schedules.iter().any(|old| {
            old.construction == s.construction
                && old.route_choices == s.route_choices
                && old.replay.as_ref().map(|o| &o.conditional_choices)
                    == s.replay.as_ref().map(|o| &o.conditional_choices)
        }) {
            let mut copy = s.clone();
            copy.search = None;
            self.schedules.push(copy);
            self.schedules
                .sort_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
            self.schedules.truncate(self.population_size);
        }
        if self.best.as_ref().is_none_or(|b| s.score < b.score) {
            search::progress(&mut self.report, s, self.start);
            self.best = Some(s.clone());
        }
    }
    fn observe(&mut self, outcome: &Outcome, limit: usize) {
        self.report.evaluations += 1;
        match outcome {
            Ok(s) => self.accept(s, limit),
            Err(e) => {
                self.report.failed_evaluations += 1;
                self.errors = e.clone();
            }
        }
    }
    fn seed(
        &mut self,
        p: &Problem,
        o: &Options,
        s: &Schedule,
        extension: Option<&dyn Customization>,
    ) {
        // Classic reference plans remain eligible incumbents, but do not validate
        // an unrelated goal-derived Q policy for the handoff.
        if o.strategy != "queues" && o.queue_policy.is_none() && p.queue_policy.is_none() {
            return;
        }
        let mut options = search::queue_options(p, o, extension);
        // Carry the evaluated route configuration, not another random route draw.
        options.route_choices = s.route_choices.clone();
        if self.reference.is_none() {
            self.reference = Some(Seed {
                options: options.clone(),
                schedule: s.clone(),
            });
        }
        if let Some(index) = self.seeds.iter().position(|v| {
            v.options.queue_policy == options.queue_policy
                && v.options.route_choices == options.route_choices
                && v.options.conditional_choices == options.conditional_choices
        }) {
            if s.score >= self.seeds[index].schedule.score {
                return;
            }
            self.seeds.remove(index);
        } else if self.seeds.len() == o.improve.policies
            && self
                .seeds
                .last()
                .is_some_and(|v| s.score >= v.schedule.score)
        {
            return;
        }
        self.seeds.push(Seed {
            options,
            schedule: s.clone(),
        });
        self.seeds
            .sort_by(|a, b| a.schedule.score.partial_cmp(&b.schedule.score).unwrap());
        self.seeds.truncate(o.improve.policies);
    }
}

pub fn improve(p: &Problem, o: &Options) -> Outcome {
    improve_from(p, o, None, None)
}

/// Optional incumbents must belong to this exact model and satisfy requested commitments.
/// The service additionally enforces scenario identity and revision.
pub fn improve_from(
    p: &Problem,
    o: &Options,
    provided: Option<&dyn Customization>,
    incumbent: Option<&Schedule>,
) -> Outcome {
    let start = Instant::now();
    let max = search::limits(o)?;
    crate::evolution::check_config(o)?;
    if !(0.0..1.0).contains(&o.improve.trainer_share)
        || o.improve.trainer_share == 0.0
        || !o.improve.evolution_share.is_finite()
        || o.improve.evolution_share < 0.0
        || o.improve.trainer_share + o.improve.evolution_share >= 1.0
        || !(1..=8).contains(&o.improve.policies)
        || (max == usize::MAX && o.budget_ms == 0)
    {
        return Err(vec![Diagnostic::new(
            "IMPROVE_OPTIONS",
            "improve",
            "Require trainer_share > 0, evolution_share >= 0, their sum < 1, 1..8 policies, and an evaluation or time limit",
        )]);
    }
    let owned = p
        .customization
        .as_ref()
        .map(crate::extensions::registered)
        .transpose()?;
    let extension = provided.or(owned.as_deref());
    let mut state = Portfolio {
        start,
        report: SearchReport {
            algorithm: if o.improve.evolution_share > 0.0 {
                "trainer_plus_ga"
            } else {
                "trainer_plus_portfolio"
            }
            .into(),
            workers: o.trainer.workers.max(o.plus.workers),
            ..Default::default()
        },
        best: None,
        seeds: vec![],
        reference: None,
        errors: vec![],
        schedules: vec![],
        population_size: o.trainer.population_size,
    };
    let mut base = o.clone();
    if let Some(s) = incumbent {
        let v = if let Some(e) = extension {
            validate::validate_customized(p, s, e)
        } else {
            validate::validate(p, s)
        };
        if !v.valid {
            return Err(v.diagnostics);
        }
        let replay = s.replay.as_deref().ok_or_else(|| {
            vec![Diagnostic::new(
                "IMPROVE_BASELINE",
                "improve",
                "Incumbent needs replay options",
            )]
        })?;
        if o.route_choices
            .iter()
            .any(|(id, route)| s.route_choices.get(id) != Some(route))
            || o.mode_choices.iter().any(|(id, mode)| {
                !s.assignments
                    .iter()
                    .any(|a| &a.task == id && &a.mode == mode)
            })
            || o.conditional_choices.iter().any(|(task, row)| {
                row.iter().any(|(key, mode)| {
                    !s.assignments.iter().any(|a| {
                        &a.task == task
                            && a.activities
                                .iter()
                                .any(|v| v.id == format!("{task}:{key}") && &v.mode == mode)
                    })
                })
            })
            || !replay.decision_prefix.starts_with(&o.decision_prefix)
        {
            return Err(vec![Diagnostic::new(
                "IMPROVE_BASELINE",
                "improve",
                "Incumbent conflicts with requested route, mode or prefix options",
            )]);
        }
        if base.queue_policy.is_none() {
            base.queue_policy = replay.queue_policy.clone();
        }
        state.accept(s, o.trainer.archive_limit);
        // Use requested search settings and the incumbent's construction policy.
        let mut seed_options = base.clone();
        seed_options.queue_policy = replay.queue_policy.clone();
        seed_options.conditional_choices = replay.conditional_choices.clone();
        if replay.decision_order.is_empty() {
            state.seed(p, &seed_options, s, extension);
        }
    }
    let lowered = crate::language::needs_lowering(p)
        .then(|| crate::language::lower(p))
        .transpose()?;
    let p = lowered.as_ref().unwrap_or(p);
    let normalized = search::queue_options(p, &base, extension);
    crate::queues::check(
        p,
        normalized.queue_policy.as_ref().unwrap(),
        &crate::queues::definitions(p, extension),
    )
    .map_err(|e| vec![e])?;
    state.report.unmapped_objectives =
        crate::queues::unmapped(p, &crate::queues::definitions(p, extension));
    let exhausted = |state: &Portfolio| {
        state.report.evaluations >= max
            || (o.budget_ms > 0 && start.elapsed().as_millis() >= o.budget_ms as u128)
    };
    if !exhausted(&state) {
        let mut training = base.clone();
        training.trainer.max_evaluations = Some(if max == usize::MAX {
            0
        } else {
            ((max as f64 * o.improve.trainer_share).floor() as usize)
                .max(1)
                .min(max)
        });
        if o.budget_ms > 0 {
            let remaining = o
                .budget_ms
                .saturating_sub(start.elapsed().as_millis() as u64);
            training.budget_ms =
                ((remaining as f64 * o.improve.trainer_share).floor() as u64).max(1);
        }
        let phase_start = Instant::now();
        let before = state.report.evaluations;
        let failed = state.report.failed_evaluations;
        let result = search::train_observed(p, &training, extension, &mut |genome, result| {
            state.observe(result, o.trainer.archive_limit);
            if let Ok(s) = result {
                state.seed(p, genome, s, extension);
            }
        });
        let report = result.as_ref().ok().and_then(|s| s.search.as_ref());
        if let Some(r) = report {
            state.report.generations = r.generations;
            state.report.mutations = r.mutations;
            state.report.crossovers = r.crossovers;
        } else if let Err(e) = &result {
            state.errors = e.clone();
        }
        state.report.phases.push(SearchPhase {
            name: "trainer".into(),
            evaluations: state.report.evaluations - before,
            failed_evaluations: state.report.failed_evaluations - failed,
            elapsed_ms: phase_start.elapsed().as_secs_f64() * 1000.0,
            stop_reason: report.map_or("no_valid_result", |r| &r.stop_reason).into(),
            queue_policy: None,
            route_choices: Default::default(),
            best_score: state.best.as_ref().map(|s| s.score.clone()),
        });
    }
    let mut pool = std::mem::take(&mut state.seeds);
    // A better greedy score does not imply a better rollout policy. Preserve
    // the first evaluated Q as an exploration anchor, alongside evolved elites.
    if o.improve.policies > 1
        && let Some(reference) = state.reference.take()
    {
        pool.retain(|s| {
            s.options.queue_policy != reference.options.queue_policy
                || s.options.route_choices != reference.options.route_choices
        });
        pool.insert(0, reference);
        pool.truncate(o.improve.policies);
    }
    let mut seeds: Vec<_> = pool
        .into_iter()
        .map(|seed| (seed.options, Some(seed.schedule)))
        .collect();
    // A failed greedy population is not proof of infeasibility: Plus can backtrack.
    if seeds.is_empty() && !exhausted(&state) {
        seeds.push((base.clone(), state.best.clone()));
    }
    let count = seeds.len();
    let reserve = if max == usize::MAX {
        0
    } else {
        (max as f64 * o.improve.evolution_share).floor() as usize
    };
    let reserve_ms = (o.budget_ms as f64 * o.improve.evolution_share).floor() as u64;
    for (index, (mut options, initial)) in seeds.into_iter().enumerate() {
        if exhausted(&state) {
            break;
        }
        let slots = count - index;
        if max != usize::MAX && max.saturating_sub(state.report.evaluations) <= reserve {
            break;
        }
        if o.budget_ms > 0
            && o.budget_ms
                .saturating_sub(start.elapsed().as_millis() as u64)
                <= reserve_ms
        {
            break;
        }
        options.trainer.max_evaluations = Some(if max == usize::MAX {
            0
        } else {
            (max - state.report.evaluations - reserve).div_ceil(slots)
        });
        if o.budget_ms > 0 {
            options.budget_ms = o
                .budget_ms
                .saturating_sub(start.elapsed().as_millis() as u64)
                .saturating_sub(reserve_ms)
                .div_ceil(slots as u64)
                .max(1);
        }
        let before = state.report.evaluations;
        let failed = state.report.failed_evaluations;
        let phase_start = Instant::now();
        let result = search::plus_observed(p, &options, extension, initial, &mut |_, result| {
            state.observe(result, o.trainer.archive_limit);
        });
        let report = result.as_ref().ok().and_then(|s| s.search.as_ref());
        if let Some(r) = report {
            state.report.nodes += r.nodes;
            state.report.revisits += r.revisits;
        } else if let Err(e) = &result {
            state.errors = e.clone();
        }
        state.report.phases.push(SearchPhase {
            name: format!("plus:{}", index + 1),
            evaluations: state.report.evaluations - before,
            failed_evaluations: state.report.failed_evaluations - failed,
            elapsed_ms: phase_start.elapsed().as_secs_f64() * 1000.0,
            stop_reason: report.map_or("no_valid_result", |r| &r.stop_reason).into(),
            queue_policy: options.queue_policy,
            route_choices: options.route_choices,
            best_score: state.best.as_ref().map(|s| s.score.clone()),
        });
    }
    if o.improve.evolution_share > 0.0 && !exhausted(&state) {
        let mut options = base.clone();
        options.trainer.max_evaluations = Some(if max == usize::MAX {
            0
        } else {
            max - state.report.evaluations
        });
        if o.budget_ms > 0 {
            options.budget_ms = o
                .budget_ms
                .saturating_sub(start.elapsed().as_millis() as u64)
                .max(1);
        }
        let phase_start = Instant::now();
        let before = state.report.evaluations;
        let failed = state.report.failed_evaluations;
        let seeds = std::mem::take(&mut state.schedules);
        let result =
            crate::evolution::evolve_observed(p, &options, extension, &seeds, &mut |_, result| {
                state.observe(result, o.trainer.archive_limit)
            });
        let report = result.as_ref().ok().and_then(|s| s.search.as_ref());
        if let Some(r) = report {
            state.report.generations += r.generations;
            state.report.mutations += r.mutations;
            state.report.crossovers += r.crossovers;
            state.report.operators = r.operators.clone();
        } else if let Err(e) = &result {
            state.errors = e.clone();
        }
        state.report.phases.push(SearchPhase {
            name: "direct_ga".into(),
            evaluations: state.report.evaluations - before,
            failed_evaluations: state.report.failed_evaluations - failed,
            elapsed_ms: phase_start.elapsed().as_secs_f64() * 1000.0,
            stop_reason: report.map_or("no_valid_result", |r| &r.stop_reason).into(),
            queue_policy: None,
            route_choices: Default::default(),
            best_score: state.best.as_ref().map(|s| s.score.clone()),
        });
    }
    state.report.stop_reason = if state.report.evaluations >= max {
        "evaluation_limit"
    } else if o.budget_ms > 0 && start.elapsed().as_millis() >= o.budget_ms as u128 {
        "time_limit"
    } else {
        "phases_complete"
    }
    .into();
    if let Some(mut s) = state.best {
        s.evaluations = state.report.evaluations;
        s.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        s.search = Some(state.report);
        Ok(s)
    } else if state.errors.is_empty() {
        Err(vec![Diagnostic::new(
            "IMPROVE_BUDGET",
            "improve",
            "Budget ended before a valid plan was found",
        )])
    } else {
        Err(state.errors)
    }
}
