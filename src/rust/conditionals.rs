//! Conditional choices are first-class commitments and replayable search genes.
use crate::model::*;
use std::collections::BTreeMap;

pub type Catalog = BTreeMap<String, BTreeMap<String, Vec<String>>>;

pub fn allows(p: &Problem, t: &Task, m: &Mode) -> bool {
    t.conditional_modes.iter().all(|(key, selected)| {
        let candidates: Vec<&Conditional> = if let Some(id) = key.strip_prefix("pre:") {
            t.pre.iter().chain(&m.pre).filter(|c| c.id == id).collect()
        } else if let Some(id) = key.strip_prefix("post:") {
            t.post
                .iter()
                .chain(&m.post)
                .filter(|c| c.id == id)
                .collect()
        } else if let Some(id) = key.strip_prefix("restart:") {
            t.execution
                .iter()
                .flat_map(|e| &e.restart)
                .filter(|c| c.id == id)
                .collect()
        } else {
            return true;
        };
        (candidates.is_empty()
            && p.native_conditionals
                .get(&t.id)
                .and_then(|r| r.get(key))
                .is_some_and(|m| m.contains(selected)))
            || candidates
                .iter()
                .any(|c| c.modes.iter().any(|mode| &mode.id == selected))
    })
}

pub fn catalog(p: &Problem) -> Catalog {
    let mut out = p.native_conditionals.clone();
    for t in &p.tasks {
        if t.pre.is_empty()
            && t.post.is_empty()
            && t.modes
                .iter()
                .all(|m| m.pre.is_empty() && m.post.is_empty())
            && t.execution.as_ref().is_none_or(|e| e.restart.is_empty())
            && p.transitions.is_empty()
            && !p
                .rules
                .iter()
                .any(|r| matches!(r, Rule::SequencePattern { .. }))
        {
            continue;
        }
        let row = out.entry(t.id.clone()).or_default();
        let mut add = |key: String, c: &Conditional| {
            let modes = row.entry(key).or_default();
            modes.extend(c.modes.iter().map(|m| m.id.clone()));
            modes.sort();
            modes.dedup();
        };
        if let Some(e) = &t.execution {
            for c in &e.restart {
                add(format!("restart:{}", c.id), c);
            }
        } else {
            for c in &t.pre {
                add(format!("pre:{}", c.id), c);
            }
            for m in &t.modes {
                for c in &m.pre {
                    add(format!("pre:{}", c.id), c);
                }
            }
        }
        for c in &t.post {
            add(format!("post:{}", c.id), c);
        }
        for m in &t.modes {
            for c in &m.post {
                add(format!("post:{}", c.id), c);
            }
        }
        for tr in &p.transitions {
            if !t.modes.iter().any(|m| m.primary == tr.resource) {
                continue;
            }
            if tr.to == t.family && t.execution.is_none() {
                for c in &tr.next_pre {
                    add(format!("transition:{}:pre:{}", tr.id, c.id), c);
                }
            }
            if tr.from == t.family {
                for c in &tr.previous_post {
                    add(format!("transition:{}:post:{}", tr.id, c.id), c);
                }
            }
        }
        for r in &p.rules {
            if let Rule::SequencePattern {
                id,
                resource,
                previous_post,
                next_pre,
                ..
            } = r
                && t.modes.iter().any(|m| &m.primary == resource)
            {
                if t.execution.is_none() {
                    for c in next_pre {
                        add(format!("pattern:{id}:pre:{}", c.id), c);
                    }
                }
                for c in previous_post {
                    add(format!("pattern:{id}:post:{}", c.id), c);
                }
            }
        }
    }
    out
}

pub fn check(p: &Problem, errors: &mut Vec<Diagnostic>) {
    if !p.tasks.iter().any(|t| !t.conditional_modes.is_empty()) {
        return;
    }
    let all = catalog(p);
    for t in &p.tasks {
        for (id, mode) in &t.conditional_modes {
            if !all
                .get(&t.id)
                .and_then(|r| r.get(id))
                .is_some_and(|v| v.contains(mode))
            {
                errors.push(Diagnostic::new(
                    "CONDITIONAL_SELECTION",
                    &t.id,
                    format!("Unknown conditional or mode: {id} -> {mode}"),
                ));
            }
        }
    }
}

pub fn apply(p: &mut Problem, o: &Options) -> Result<(), Vec<Diagnostic>> {
    for (task, choices) in &o.conditional_choices {
        let Some(t) = p.tasks.iter_mut().find(|t| &t.id == task) else {
            return Err(vec![Diagnostic::new(
                "CONDITIONAL_SELECTION",
                task,
                "Unknown or inactive task",
            )]);
        };
        for (id, mode) in choices {
            if t.conditional_modes.get(id).is_some_and(|m| m != mode) {
                return Err(vec![Diagnostic::new(
                    "CONDITIONAL_LOCK",
                    task,
                    format!("Cannot override {id}"),
                )]);
            }
            t.conditional_modes.insert(id.clone(), mode.clone());
        }
    }
    let mut errors = vec![];
    check(p, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

pub fn violations(p: &Problem, s: &Schedule) -> Vec<Diagnostic> {
    let tasks: BTreeMap<_, _> = p.tasks.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut errors = vec![];
    for a in &s.assignments {
        if let Some(t) = tasks.get(a.task.as_str()) {
            for (key, mode) in &t.conditional_modes {
                let id = format!("{}:{key}", t.id);
                if !a.activities.iter().any(|v| v.id == id && &v.mode == mode) {
                    errors.push(Diagnostic::new(
                        "CONDITIONAL_LOCK",
                        &t.id,
                        format!("Missing or changed conditional choice {key} -> {mode}"),
                    ));
                }
            }
        }
    }
    errors
}

/// Reconstruct only semantic activity references, not an uncounted schedule evaluation.
pub fn branches(
    p: &Problem,
    o: &Options,
    extension: Option<&dyn crate::rules::Customization>,
) -> Result<Option<Vec<Options>>, Vec<Diagnostic>> {
    let lowered = extension
        .map(|e| crate::extensions::lower(p, e))
        .transpose()?;
    let p = lowered.as_ref().unwrap_or(p);
    if o.decision_prefix.is_empty()
        || !catalog(p)
            .values()
            .any(|row| row.values().any(|m| m.len() > 1))
    {
        return Ok(None);
    }
    let (p, _) = crate::domain::resolve(p, o)?;
    let c = crate::compile::compile(&p)?;
    let mut d = crate::engine::Decisions {
        order: vec![],
        modes: vec![0; p.tasks.len()],
        next_choices: vec![],
        evidence: None,
    };
    for step in &o.decision_prefix {
        let Some(&i) = c.tasks.get(step.task.as_str()) else {
            return Ok(None);
        };
        let Some(mi) = p.tasks[i].modes.iter().position(|m| m.id == step.mode) else {
            return Ok(None);
        };
        d.order.push(i);
        d.modes[i] = mi;
    }
    let decorated = extension
        .filter(|e| e.supports_prefix_decoration() || d.order.len() == p.tasks.len())
        .map(|e| crate::extensions::decorate(&p, &d, e))
        .transpose()?;
    let decorated_compiled = decorated
        .as_ref()
        .map(crate::compile::compile)
        .transpose()?;
    let specs = crate::engine::conditional_specs_for(
        decorated_compiled.as_ref().unwrap_or(&c),
        &d,
        d.order.len() == p.tasks.len(),
    )
    .map_err(|e| vec![e])?;
    for &i in &d.order {
        for (key, cond) in specs[i].0.iter().chain(&specs[i].1) {
            if cond.modes.len() < 2 || p.tasks[i].conditional_modes.contains_key(key) {
                continue;
            }
            return Ok(Some(
                cond.modes
                    .iter()
                    .map(|m| {
                        let mut child = o.clone();
                        child
                            .conditional_choices
                            .entry(p.tasks[i].id.clone())
                            .or_default()
                            .insert(key.clone(), m.id.clone());
                        child
                    })
                    .collect(),
            ));
        }
    }
    Ok(None)
}

/// Persisted options may restrict alternatives, but cannot relax model commitments.
pub fn validation_options(s: &Schedule) -> Options {
    Options {
        route_choices: s.route_choices.clone(),
        mode_choices: s
            .material_report
            .as_ref()
            .map(|r| r.mode_choices.clone())
            .or_else(|| s.replay.as_deref().map(|o| o.mode_choices.clone()))
            .unwrap_or_default(),
        conditional_choices: s
            .replay
            .as_deref()
            .map(|o| o.conditional_choices.clone())
            .or_else(|| {
                s.dispatch
                    .as_ref()
                    .map(|e| e.options.conditional_choices.clone())
            })
            .unwrap_or_default(),
        ..Default::default()
    }
}
