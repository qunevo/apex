//! XG: greedy schedule construction and the shared evaluation/decoding path.
pub use crate::dispatch::decisions;
use crate::dispatch::decisions_with;
use crate::{
    calendar::{self, Book},
    compile::{Compiled, compile},
    model::*,
    rules,
};
use std::{collections::HashMap, time::Instant};

pub const ID: &str = "XG";

#[derive(Clone, Debug)]
pub struct Decisions {
    pub order: Vec<usize>,
    pub modes: Vec<usize>,
    pub next_choices: Vec<Decision>,
    pub evidence: Option<crate::policy::Evidence>,
}
pub(crate) fn allowed(p: &Problem, t: &Task, m: &Mode) -> bool {
    if !crate::conditionals::allows(p, t, m) {
        return false;
    }
    if t.execution.as_ref().is_some_and(|e| e.mode != m.id) {
        return false;
    }
    p.locks.iter().all(|l| match l {
        Lock::Mode { task, mode } if task == &t.id => mode == &m.id,
        Lock::Resource { task, resource } if task == &t.id => resource == &m.primary,
        Lock::Order {
            resource, tasks, ..
        } if tasks.contains(&t.id) => resource == &m.primary,
        _ => true,
    })
}
pub fn transition<'a>(
    p: &'a Problem,
    resource: &str,
    from: &str,
    to: &str,
) -> Result<Option<&'a Transition>, Diagnostic> {
    if !p.transitions.iter().any(|t| t.resource == resource) {
        return Ok(None);
    }
    p.transitions
        .iter()
        .find(|t| t.resource == resource && t.from == from && t.to == to)
        .map(Some)
        .ok_or_else(|| {
            Diagnostic::new(
                "MISSING_TRANSITION",
                resource,
                format!("No transition {from} -> {to}"),
            )
        })
}

pub type ConditionalSpecs<'a> = Vec<(
    Vec<(String, &'a Conditional)>,
    Vec<(String, &'a Conditional)>,
)>;

pub fn conditional_specs<'a>(
    c: &Compiled<'a>,
    d: &Decisions,
) -> Result<ConditionalSpecs<'a>, Diagnostic> {
    conditional_specs_for(c, d, true)
}
pub(crate) fn conditional_specs_for<'a>(
    c: &Compiled<'a>,
    d: &Decisions,
    terminal: bool,
) -> Result<ConditionalSpecs<'a>, Diagnostic> {
    let p = c.problem;
    let mut specs: Vec<_> = p
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let m = &t.modes[d.modes[i]];
            (
                t.pre
                    .iter()
                    .chain(&m.pre)
                    .map(|x| (format!("pre:{}", x.id), x))
                    .collect::<Vec<_>>(),
                t.post
                    .iter()
                    .chain(&m.post)
                    .map(|x| (format!("post:{}", x.id), x))
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let mut last = HashMap::<&str, usize>::new();
    let mut history = HashMap::<&str, Vec<usize>>::new();
    for &i in &d.order {
        let t = &p.tasks[i];
        let m = &t.modes[d.modes[i]];
        let previous = last.get(m.primary.as_str()).copied();
        let from = previous.map_or(
            c.problem.resources[c.resources[m.primary.as_str()]]
                .initial_state
                .as_str(),
            |j| p.tasks[j].family.as_str(),
        );
        if let Some(tr) = transition(p, &m.primary, from, &t.family)? {
            if let Some(j) = previous {
                for x in &tr.previous_post {
                    specs[j]
                        .1
                        .push((format!("transition:{}:post:{}", tr.id, x.id), x));
                }
            } else if !tr.previous_post.is_empty() {
                return Err(Diagnostic::new(
                    "INITIAL_POST",
                    &tr.id,
                    "Initial transition cannot have previous-task post work",
                ));
            }
            for x in &tr.next_pre {
                specs[i]
                    .0
                    .push((format!("transition:{}:pre:{}", tr.id, x.id), x));
            }
        }
        let sequence = history.entry(&m.primary).or_default();
        for rule in &p.rules {
            if let Rule::SequencePattern {
                id,
                resource,
                pattern,
                previous_post,
                next_pre,
                ..
            } = rule
                && resource == &m.primary
                && pattern_matches(p, sequence, i, pattern)
            {
                if let Some(&j) = sequence.last() {
                    for x in previous_post {
                        specs[j].1.push((format!("pattern:{id}:post:{}", x.id), x));
                    }
                }
                for x in next_pre {
                    specs[i].0.push((format!("pattern:{id}:pre:{}", x.id), x));
                }
            }
        }
        sequence.push(i);
        last.insert(&m.primary, i);
    }
    for (resource, i) in last.into_iter().filter(|_| terminal) {
        if let Some(tr) = transition(p, resource, &p.tasks[i].family, "__end__")? {
            for x in &tr.previous_post {
                specs[i]
                    .1
                    .push((format!("transition:{}:post:{}", tr.id, x.id), x));
            }
            if !tr.next_pre.is_empty() {
                return Err(Diagnostic::new(
                    "TERMINAL_PRE",
                    &tr.id,
                    "Terminal transition cannot have next-task pre work",
                ));
            }
        }
    }
    for (i, t) in p.tasks.iter().enumerate() {
        if let Some(e) = &t.execution {
            specs[i].0 = e
                .restart
                .iter()
                .map(|x| (format!("restart:{}", x.id), x))
                .collect();
        }
    }
    for (pre, post) in &specs {
        crate::activities::graph(pre, p.horizon)?;
        crate::activities::graph(post, p.horizon)?;
    }
    Ok(specs)
}

fn append_conditional(
    c: &Compiled<'_>,
    book: &mut Book,
    t: &Task,
    id: &str,
    cond: &Conditional,
    role: &str,
    at: Time,
) -> Result<Activity, Diagnostic> {
    let best = cond
        .modes
        .iter()
        .filter(|m| {
            t.conditional_modes
                .get(id)
                .is_none_or(|selected| selected == &m.id)
        })
        .filter_map(|m| {
            calendar::place(
                c,
                book,
                &t.id,
                &format!("{}:{id}", t.id),
                role,
                &crate::domain::scaled_mode(m, t.quantity),
                at,
            )
        })
        .min_by_key(|a| (a.end, a.mode.clone()));
    let a = best.ok_or_else(|| {
        Diagnostic::new(
            "NO_SLOT",
            &t.id,
            format!("No slot found for conditional {id}"),
        )
    })?;
    book.reservations(c, &a.reservations, 1.0);
    Ok(a)
}

pub(crate) fn append_stage(
    c: &Compiled<'_>,
    book: &mut Book,
    t: &Task,
    specs: &[(String, &Conditional)],
    role: &str,
    base: Time,
) -> Result<Vec<Activity>, Diagnostic> {
    let (order, edges) = crate::activities::graph(specs, c.problem.horizon)?;
    let mut done: Vec<Option<Activity>> = vec![None; specs.len()];
    for index in order {
        let (id, conditional) = &specs[index];
        let at = edges[index]
            .iter()
            .map(|(before, min, _)| done[*before].as_ref().unwrap().end + min)
            .max()
            .unwrap_or(base)
            .max(base);
        let activity = append_conditional(c, book, t, id, conditional, role, at)?;
        if edges[index].iter().any(|(before, _, max)| {
            max.is_some_and(|max| activity.start > done[*before].as_ref().unwrap().end + max)
        }) {
            return Err(Diagnostic::new(
                "CONDITIONAL_LAG",
                id,
                "Conditional maximum lag exceeded",
            ));
        }
        done[index] = Some(activity);
    }
    Ok(done.into_iter().flatten().collect())
}

pub fn pattern_matches(p: &Problem, history: &[usize], current: usize, pattern: &[String]) -> bool {
    if pattern.is_empty() || pattern.len() > history.len() + 1 {
        return false;
    }
    pattern.last() == Some(&p.tasks[current].family)
        && pattern[..pattern.len() - 1]
            .iter()
            .zip(&history[history.len() + 1 - pattern.len()..])
            .all(|(family, i)| family == &p.tasks[*i].family)
}
pub(crate) fn pattern_penalty(
    p: &Problem,
    resource: &str,
    history: &[usize],
    current: usize,
) -> f64 {
    p.rules
        .iter()
        .filter_map(|r| match r {
            Rule::SequencePattern {
                resource: r,
                pattern,
                penalty,
                ..
            } if r == resource && pattern_matches(p, history, current, pattern) => Some(*penalty),
            _ => None,
        })
        .sum()
}
pub fn setup_penalty(p: &Problem, s: &Schedule) -> f64 {
    if !p.transitions.iter().any(|t| t.penalty != 0.0)
        && !p
            .rules
            .iter()
            .any(|r| matches!(r, Rule::SetupCharge { .. } | Rule::SequencePattern { .. }))
    {
        return 0.0;
    }
    let indexes: HashMap<_, _> = p
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();
    let mut assignments: Vec<_> = s
        .assignments
        .iter()
        .filter(|a| indexes.contains_key(a.task.as_str()))
        .collect();
    assignments.sort_by_key(|a| (a.start, indexes[a.task.as_str()]));
    let mut history = HashMap::<&str, Vec<usize>>::new();
    let mut cost = 0.0;
    for a in assignments {
        let i = indexes[a.task.as_str()];
        let sequence = history.entry(&a.primary).or_default();
        let initial = p
            .resources
            .iter()
            .find(|r| r.id == a.primary)
            .map_or("", |r| r.initial_state.as_str());
        let from = sequence
            .last()
            .map_or(initial, |i| p.tasks[*i].family.as_str());
        cost += transition(p, &a.primary, from, &p.tasks[i].family)
            .ok()
            .flatten()
            .map_or(0.0, |t| t.penalty);
        cost += pattern_penalty(p, &a.primary, sequence, i);
        sequence.push(i);
    }
    for (resource, sequence) in history {
        if let Some(&i) = sequence.last() {
            cost += transition(p, resource, &p.tasks[i].family, "__end__")
                .ok()
                .flatten()
                .map_or(0.0, |t| t.penalty);
        }
    }
    cost + p
        .rules
        .iter()
        .filter_map(|r| {
            if let Rule::SetupCharge { value, .. } = r {
                Some(value)
            } else {
                None
            }
        })
        .sum::<f64>()
}

pub fn decode(c: &Compiled<'_>, d: &Decisions) -> Result<Schedule, Diagnostic> {
    decode_internal(c, d, true)
}
pub(crate) fn decode_prefix(c: &Compiled<'_>, d: &Decisions) -> Result<Schedule, Diagnostic> {
    decode_internal(c, d, false)
}
fn decode_internal(
    c: &Compiled<'_>,
    d: &Decisions,
    terminal: bool,
) -> Result<Schedule, Diagnostic> {
    let p = c.problem;
    let specs = conditional_specs_for(c, d, terminal)?;
    let mut state = crate::decoding::State::new(c);
    for &i in &d.order {
        let assignment = state.preview(c, i, d.modes[i], &specs[i])?;
        state.commit(c, i, assignment);
    }
    let mut s = Schedule {
        problem_id: p.id.clone(),
        assignments: state.assignments.into_iter().flatten().collect(),
        ..Default::default()
    };
    if !terminal {
        return Ok(s);
    }
    if let Some(error) = crate::conditionals::violations(p, &s).into_iter().next() {
        return Err(error);
    }
    (s.job_completions, s.order_completions) = crate::domain::completions(p, &s);
    s.metrics = rules::metrics(p, &s);
    s.metric_version = 1;
    s.score = rules::score(p, &s.metrics);
    s.objective_values = rules::objective_values(p, &s.metrics, &[]);
    Ok(s)
}

/// XG constructs and independently validates one schedule (`schedule.create`).
pub fn create(p: &Problem, options: &Options) -> Result<Schedule, Vec<Diagnostic>> {
    if let Some(reference) = &p.customization {
        let extension = crate::extensions::registered(reference)?;
        return create_customized(p, options, extension.as_ref()).map(|(_, s)| s);
    }
    create_internal(p, options, None)
}
fn create_internal(
    p: &Problem,
    options: &Options,
    extension: Option<&dyn rules::Customization>,
) -> Result<Schedule, Vec<Diagnostic>> {
    let started = Instant::now();
    if ![
        "due",
        "shortest",
        "priority",
        "release",
        "objective",
        "random",
        "weighted",
        "setup",
        "queues",
    ]
    .contains(&options.strategy.as_str())
        || options.strategy == "weighted" && options.weights.is_none()
        || options.weights.as_ref().is_some_and(|w| {
            [w.due, w.work, w.priority, w.release, w.urgency]
                .iter()
                .any(|x| !x.is_finite() || *x < 0.0 || *x > 1e6)
        })
    {
        return Err(vec![Diagnostic::new(
            "HEURISTIC_OPTIONS",
            &p.id,
            "Unknown strategy or invalid dispatch weights",
        )]);
    }
    crate::xe::check_order(p, &options.decision_order).map_err(|e| vec![e])?;
    let original = compile(p)?;
    let (resolved, choices) = crate::domain::resolve(p, options)?;
    let c = if matches!(&resolved, std::borrow::Cow::Borrowed(_)) {
        original
    } else {
        compile(&resolved)?
    };
    let d = decisions_with(&c, options, extension).map_err(|e| vec![e])?;
    let decorated = if let Some(extension) = extension {
        Some(crate::extensions::decorate(&resolved, &d, extension)?)
    } else {
        None
    };
    let evaluation = decorated.as_ref().unwrap_or(&resolved);
    let mut s = if decorated.is_some() {
        decode(&compile(evaluation)?, &d)
    } else {
        decode(&c, &d)
    }
    .map_err(|e| vec![e])?;
    s.route_choices = choices;
    s.construction = d
        .order
        .iter()
        .map(|&i| Decision {
            task: c.problem.tasks[i].id.clone(),
            mode: c.problem.tasks[i].modes[d.modes[i]].id.clone(),
        })
        .collect();
    s.dispatch = d.evidence;
    s.replay = Some(Box::new(options.clone()));
    s.material_report = resolved.material_report.clone();
    if let Some(extension) = extension {
        crate::extensions::apply_metrics(evaluation, &mut s, extension)?;
    }
    if s.metrics
        .values()
        .chain(&s.score)
        .chain(&s.objective_values)
        .any(|v| !v.is_finite())
    {
        return Err(vec![Diagnostic::new(
            "NUMERIC_OVERFLOW",
            &p.id,
            "Metrics and objective scores must stay finite; reduce weights or adjust scales",
        )]);
    }
    let validation = if let Some(extension) = extension {
        crate::extensions::validate_lowered(p, &s, extension)
    } else {
        crate::validate::validate(p, &s)
    };
    if !validation.valid {
        return Err(validation.diagnostics);
    }
    s.elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
    s.evaluations = 1;
    s.strategy = options.strategy.clone();
    s.seed = options.seed;
    s.dispatch_weights = options.weights.clone();
    s.replay = Some(Box::new(options.clone()));
    if rules::objectives(evaluation, &[])
        .iter()
        .any(|o| !s.metrics.contains_key(&o.metric))
    {
        return Err(vec![Diagnostic::new(
            "MISSING_OBJECTIVE_VALUE",
            &p.id,
            "Selected objective has no evaluator",
        )]);
    }
    Ok(s)
}
pub(crate) fn evaluate(
    p: &Problem,
    options: &Options,
    extension: Option<&dyn rules::Customization>,
) -> Result<Schedule, Vec<Diagnostic>> {
    if let Some(e) = extension {
        create_customized(p, options, e).map(|(_, s)| s)
    } else {
        create(p, options)
    }
}
pub(crate) fn next_choices(
    p: &Problem,
    options: &Options,
    extension: Option<&dyn rules::Customization>,
) -> Result<Vec<Decision>, Vec<Diagnostic>> {
    let mut lowered = if let Some(e) = extension {
        crate::extensions::lower(p, e)?
    } else {
        p.clone()
    };
    lowered.customization = None;
    let (resolved, _) = crate::domain::resolve(&lowered, options)?;
    let c = compile(&resolved)?;
    crate::dispatch::choices(&c, options, extension).map_err(|e| vec![e])
}

pub fn create_customized(
    p: &Problem,
    options: &Options,
    extension: &dyn rules::Customization,
) -> Result<(Problem, Schedule), Vec<Diagnostic>> {
    let mut lowered = crate::extensions::lower(p, extension)?;
    lowered.assumptions.push(format!(
        "Compiled customization {}@{}",
        extension.id(),
        extension.version()
    ));
    lowered.customization = None;
    let result = create_internal(&lowered, options, Some(extension))?;
    Ok((lowered, result))
}
