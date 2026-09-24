//! Statically linked extensions. Data selects a versioned implementation, never source code.
use crate::{engine::Decisions, model::*, rules::Customization};
use std::collections::{BTreeMap, HashMap};

pub struct SequenceContext<'a> {
    pub problem: &'a Problem,
    pub resource: &'a Resource,
    /// Complete selected primary-resource sequence, including both neighbors.
    pub tasks: Vec<&'a Task>,
}
#[derive(Default)]
pub struct TaskDecoration {
    pub task: String,
    pub pre: Vec<Conditional>,
    pub post: Vec<Conditional>,
    pub penalty: f64,
}
pub struct Metric {
    pub id: String,
    pub value: f64,
    pub weight: f64,
    pub priority: u32,
}
pub fn lower(p: &Problem, extension: &dyn Customization) -> Result<Problem, Vec<Diagnostic>> {
    let mut lowered = extension.compile(p)?;
    lowered.native_objectives = extension.objectives(&lowered);
    lowered.native_conditionals = extension.conditional_modes(&lowered);
    for (task, row) in &lowered.native_conditionals {
        if !lowered.tasks.iter().any(|t| &t.id == task)
            || row.iter().any(|(key, modes)| {
                !(key.starts_with("pre:") || key.starts_with("post:"))
                    || key.split_once(':').is_none_or(|(_, id)| id.is_empty())
                    || modes.is_empty()
                    || modes.iter().any(String::is_empty)
                    || modes.iter().collect::<std::collections::HashSet<_>>().len() != modes.len()
            })
        {
            return Err(vec![Diagnostic::new(
                "CUSTOMIZATION_CONDITIONAL_CATALOG",
                task,
                "Declare existing tasks, pre:/post: activity IDs and unique nonempty mode IDs",
            )]);
        }
    }
    for mut definition in extension.queue_definitions(&lowered) {
        if !lowered
            .queue_definitions
            .iter()
            .any(|d| d.id == definition.id)
        {
            definition.attribute.clear();
            lowered.queue_definitions.push(definition);
        }
    }
    lowered.customization = None;
    Ok(lowered)
}

pub fn registered(reference: &CustomizationRef) -> Result<Box<dyn Customization>, Vec<Diagnostic>> {
    match (reference.id.as_str(), reference.version.as_str()) {
        ("dummy_customer", "1") => Ok(Box::new(crate::dummy_customer::Policy)),
        _ => Err(vec![Diagnostic::new(
            "CUSTOMIZATION_VERSION",
            &reference.id,
            "This extension/version is not linked into the executable",
        )]),
    }
}

pub fn decorate(
    p: &Problem,
    decisions: &Decisions,
    extension: &dyn Customization,
) -> Result<Problem, Vec<Diagnostic>> {
    let mut sequences = BTreeMap::<&str, Vec<&Task>>::new();
    for &i in &decisions.order {
        sequences
            .entry(&p.tasks[i].modes[decisions.modes[i]].primary)
            .or_default()
            .push(&p.tasks[i]);
    }
    let mut result = p.clone();
    let indexes: HashMap<_, _> = p
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();
    for (resource, tasks) in sequences {
        let context = SequenceContext {
            problem: p,
            resource: p.resources.iter().find(|r| r.id == resource).unwrap(),
            tasks,
        };
        for (n, decoration) in extension.sequence(&context)?.into_iter().enumerate() {
            if !context.tasks.iter().any(|t| t.id == decoration.task)
                || !decoration.penalty.is_finite()
                || decoration.penalty < 0.0
            {
                return Err(vec![Diagnostic::new(
                    "CUSTOMIZATION_SEQUENCE",
                    extension.id(),
                    "Decoration target must belong to its sequence and penalty must be nonnegative",
                )]);
            }
            let i = indexes[decoration.task.as_str()];
            result.tasks[i].pre.extend(decoration.pre);
            result.tasks[i].post.extend(decoration.post);
            result.rules.push(Rule::SetupCharge {
                id: format!("extension:{}:{resource}:{n}", extension.id()),
                task: decoration.task,
                value: decoration.penalty,
            });
        }
    }
    Ok(result)
}

pub fn apply_metrics(
    p: &Problem,
    s: &mut Schedule,
    extension: &dyn Customization,
) -> Result<(), Vec<Diagnostic>> {
    let extra = extension.metrics(p, s)?;
    let mut objectives = vec![];
    for metric in extra {
        if metric.id.is_empty()
            || !metric.value.is_finite()
            || !metric.weight.is_finite()
            || metric.weight < 0.0
            || s.metrics.contains_key(&metric.id)
        {
            return Err(vec![Diagnostic::new(
                "CUSTOMIZATION_METRIC",
                extension.id(),
                "Custom metrics need unique names and finite values/weights",
            )]);
        }
        s.metrics.insert(metric.id.clone(), metric.value);
        objectives.push(Objective {
            metric: metric.id,
            weight: metric.weight,
            priority: metric.priority,
            ..Default::default()
        });
    }
    s.score = crate::rules::score_with(p, &s.metrics, &objectives);
    s.objective_values = crate::rules::objective_values(p, &s.metrics, &objectives);
    Ok(())
}

pub fn validate_lowered(p: &Problem, s: &Schedule, extension: &dyn Customization) -> Validation {
    let run = || -> Result<(), Vec<Diagnostic>> {
        if s.route_choices.len() != p.routes.len() {
            return Err(vec![Diagnostic::new(
                "ROUTE_COVERAGE",
                &p.id,
                "Missing route decisions",
            )]);
        }
        let (resolved, _) = crate::domain::resolve(p, &crate::conditionals::validation_options(s))?;
        if resolved.material_report != s.material_report {
            return Err(vec![Diagnostic::new(
                "MATERIAL_REPORT",
                &p.id,
                "Allocation report differs from the selected route",
            )]);
        }
        crate::policy::verify(&resolved, s, Some(extension))?;
        let c = crate::compile::compile(&resolved)?;
        let mut modes = vec![0; resolved.tasks.len()];
        let mut order = vec![];
        for a in &s.assignments {
            let Some(&i) = c.tasks.get(a.task.as_str()) else {
                return Err(vec![Diagnostic::new(
                    "UNKNOWN_TASK",
                    &a.task,
                    "Unexpected assignment",
                )]);
            };
            let Some(mi) = resolved.tasks[i].modes.iter().position(|m| m.id == a.mode) else {
                return Err(vec![Diagnostic::new("MODE", &a.task, "Unknown mode")]);
            };
            modes[i] = mi;
            order.push((a.start, i));
        }
        order.sort_unstable();
        let decorated = decorate(
            &resolved,
            &Decisions {
                order: order.into_iter().map(|(_, i)| i).collect(),
                modes,
                next_choices: vec![],
                evidence: None,
            },
            extension,
        )?;
        let mut base = s.clone();
        base.metrics = crate::rules::metrics(&decorated, s);
        base.score = crate::rules::score(&decorated, &base.metrics);
        base.objective_values = crate::rules::objective_values(&decorated, &base.metrics, &[]);
        let report = crate::validate::validate_active(&decorated, &base);
        if !report.valid {
            return Err(report.diagnostics);
        }
        let diagnostics = extension.validate(&decorated, &base);
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        apply_metrics(&decorated, &mut base, extension)?;
        if s.metric_version == 0 {
            // Old persisted results predate the additive KPI catalog.
            base.metrics.retain(|id, _| {
                !crate::metrics::supported(&decorated, id)
                    || crate::metrics::ORIGINAL.contains(&id.as_str())
                    || s.metrics.contains_key(id)
                    || crate::rules::objectives(&decorated, &[])
                        .iter()
                        .any(|o| &o.metric == id)
            });
        }
        if base.metrics != s.metrics
            || base.score != s.score
            || base.objective_values != s.objective_values
        {
            return Err(vec![Diagnostic::new(
                "CUSTOMIZATION_METRICS",
                extension.id(),
                "Metrics differ from independent extension evaluation",
            )]);
        }
        Ok(())
    };
    match run() {
        Ok(()) => Validation {
            valid: true,
            diagnostics: vec![],
        },
        Err(diagnostics) => Validation {
            valid: false,
            diagnostics,
        },
    }
}
