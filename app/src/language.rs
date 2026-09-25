//! Versioned, bounded scheduling vocabulary. No expressions or executable input.
use crate::model::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PlanningModel {
    pub version: String,
    pub constraints: Vec<Constraint>,
    pub policies: Vec<crate::policy::Policy>,
    pub objectives: Vec<CompletionObjective>,
}
impl Default for PlanningModel {
    fn default() -> Self {
        Self {
            version: "apex.planning.v1".into(),
            constraints: vec![],
            policies: vec![],
            objectives: vec![],
        }
    }
}
/// Empty fields are wildcards; populated fields are ANDed, values within a field ORed.
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Selector {
    pub tasks: Vec<String>,
    pub families: Vec<String>,
    pub stages: Vec<String>,
}
impl Selector {
    pub fn matches(&self, t: &Task) -> bool {
        (self.tasks.is_empty() || self.tasks.contains(&t.id))
            && (self.families.is_empty() || self.families.contains(&t.family))
            && (self.stages.is_empty() || self.stages.contains(&t.stage))
    }
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Constraint {
    EligibleResources {
        id: String,
        select: Selector,
        resources: Vec<String>,
    },
    Window {
        id: String,
        select: Selector,
        earliest: Time,
        latest: Time,
    },
    Sequence {
        id: String,
        resource: String,
        tasks: Vec<String>,
        consecutive: bool,
    },
}
/// Minimize sum(attribute * completion), with a unit coefficient when omitted.
/// The existing attribute objective supplies both the metric and its documented Q.
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompletionObjective {
    pub id: String,
    #[serde(default)]
    pub select: Selector,
    #[serde(default)]
    pub attribute: Option<String>,
    pub weight: f64,
    #[serde(default)]
    pub priority: u32,
}
pub fn needs_lowering(p: &Problem) -> bool {
    !p.planning.constraints.is_empty() || !p.planning.objectives.is_empty()
}
pub fn check(p: &Problem, errors: &mut Vec<Diagnostic>) {
    if p.planning.version != "apex.planning.v1" {
        errors.push(Diagnostic::new(
            "PLANNING_VERSION",
            &p.id,
            "Expected apex.planning.v1",
        ));
    }
    let mut ids = HashSet::new();
    let mut fail = |id: &str, message| errors.push(Diagnostic::new("PLANNING_MODEL", id, message));
    for rule in &p.planning.constraints {
        let selector = match rule {
            Constraint::EligibleResources { select, .. } | Constraint::Window { select, .. } => {
                Some(select)
            }
            _ => None,
        };
        if selector.is_some_and(|s| {
            s.tasks
                .iter()
                .any(|id| !p.tasks.iter().any(|t| &t.id == id))
        }) {
            fail(&p.id, "Constraint selector references an unknown task");
        }
        let (id, valid) = match rule {
            Constraint::EligibleResources {
                id,
                select,
                resources,
            } => (
                id,
                p.tasks.iter().any(|t| select.matches(t))
                    && !resources.is_empty()
                    && resources
                        .iter()
                        .all(|id| p.resources.iter().any(|r| &r.id == id))
                    && p.tasks
                        .iter()
                        .filter(|t| select.matches(t))
                        .all(|t| t.modes.iter().any(|m| resources.contains(&m.primary))),
            ),
            Constraint::Window {
                id,
                earliest,
                latest,
                ..
            } => (
                id,
                *earliest >= 0 && latest >= earliest && *latest <= p.horizon,
            ),
            Constraint::Sequence {
                id,
                resource,
                tasks,
                ..
            } => (
                id,
                p.resources.iter().any(|r| &r.id == resource)
                    && !tasks.is_empty()
                    && tasks.iter().all(|id| p.tasks.iter().any(|t| &t.id == id))
                    && tasks.iter().collect::<HashSet<_>>().len() == tasks.len(),
            ),
        };
        if id.is_empty() || !ids.insert(id) || !valid {
            fail(
                id,
                "Invalid or duplicate constraint; check scope, resources, sequence and bounds",
            );
        }
    }
    for o in &p.planning.objectives {
        if crate::metrics::supported(p, &o.id)
            || o.select
                .tasks
                .iter()
                .any(|id| !p.tasks.iter().any(|t| &t.id == id))
            || o.id.is_empty()
            || !ids.insert(&o.id)
            || !o.weight.is_finite()
            || o.weight < 0.0
            || p.rules
                .iter()
                .any(|r| matches!(r, Rule::AttributeObjective { id, .. } if id == &o.id))
            || p.tasks.iter().filter(|t| o.select.matches(t)).any(|t| {
                o.attribute.as_ref().is_some_and(|a| {
                    t.attributes
                        .get(a)
                        .is_none_or(|v| !v.is_finite() || *v < 0.0)
                })
            })
        {
            fail(
                &o.id,
                "Completion objective needs a unique ID, nonnegative weight and explicit nonnegative coefficients on every selected task",
            );
        }
    }
    crate::policy::check(p, errors);
}
pub fn lower(p: &Problem) -> Result<Problem, Vec<Diagnostic>> {
    let mut errors = vec![];
    check(p, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }
    let mut result = p.clone();
    for rule in &p.planning.constraints {
        match rule {
            Constraint::EligibleResources {
                select, resources, ..
            } => {
                for t in result.tasks.iter_mut().filter(|t| select.matches(t)) {
                    t.modes.retain(|m| resources.contains(&m.primary));
                }
            }
            Constraint::Window {
                id,
                select,
                earliest,
                latest,
            } => {
                for t in p.tasks.iter().filter(|t| select.matches(t)) {
                    result.rules.push(Rule::TaskWindow {
                        id: format!("{id}:{}", t.id),
                        task: t.id.clone(),
                        earliest: *earliest,
                        latest: *latest,
                    });
                }
            }
            Constraint::Sequence {
                resource,
                tasks,
                consecutive,
                ..
            } => result.locks.push(Lock::Order {
                resource: resource.clone(),
                tasks: tasks.clone(),
                consecutive: *consecutive,
            }),
        }
    }
    for o in &p.planning.objectives {
        let attribute = format!("__planning:{}", o.id);
        if p.tasks
            .iter()
            .any(|t| t.attributes.contains_key(&attribute))
        {
            return Err(vec![Diagnostic::new(
                "PLANNING_MODEL",
                &o.id,
                "Generated coefficient name collides with an input attribute",
            )]);
        }
        for t in &mut result.tasks {
            let coefficient = if o.select.matches(t) {
                o.attribute.as_ref().map_or(1.0, |a| t.attributes[a])
            } else {
                0.0
            };
            t.attributes.insert(attribute.clone(), coefficient);
        }
        result.rules.push(Rule::AttributeObjective {
            id: o.id.clone(),
            attribute,
            weight: o.weight,
            priority: o.priority,
        });
    }
    result.planning.constraints.clear();
    result.planning.objectives.clear();
    Ok(result)
}
