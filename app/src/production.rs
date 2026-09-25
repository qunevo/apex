//! Explicit production-order expansion. Quantity units must be normalized by the source adapter.
use crate::model::*;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionInput {
    pub problem: Problem,
    pub workplans: Vec<Workplan>,
    pub demands: Vec<Demand>,
}
#[derive(JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workplan {
    pub id: String,
    pub item: String,
    /// Work can be fixed per lot plus work_per_unit. Material amounts are per unit.
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}
#[derive(JsonSchema, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Demand {
    pub id: String,
    pub item: String,
    pub quantity: f64,
    #[serde(default)]
    pub max_lot: Option<f64>,
    pub workplans: Vec<String>,
    #[serde(default)]
    pub selected_workplan: Option<String>,
    #[serde(default)]
    pub due: Option<Time>,
    #[serde(default)]
    pub deadline: Option<Time>,
    pub priority: f64,
    #[serde(default)]
    pub predecessors: Vec<String>,
}

pub fn expand(input: ProductionInput) -> Result<Problem, Vec<Diagnostic>> {
    let fail = |id: &str, msg: &str| vec![Diagnostic::new("PRODUCTION_INPUT", id, msg)];
    let mut p = input.problem;
    if !p.tasks.is_empty()
        || !p.jobs.is_empty()
        || !p.orders.is_empty()
        || !p.routes.is_empty()
        || !p.dependencies.is_empty()
    {
        return Err(fail(
            &p.id,
            "Expansion header must have no existing tasks, jobs, orders, routes or dependencies",
        ));
    }
    let plans: HashMap<_, _> = input.workplans.iter().map(|w| (w.id.as_str(), w)).collect();
    let demands: HashMap<_, _> = input.demands.iter().map(|d| (d.id.as_str(), d)).collect();
    if plans.len() != input.workplans.len() || demands.len() != input.demands.len() {
        return Err(fail(&p.id, "Duplicate workplan or demand ID"));
    }
    let mut task_orders = HashMap::<String, String>::new();
    for demand in &input.demands {
        if demand.id.is_empty()
            || !demand.quantity.is_finite()
            || demand.quantity <= 0.0
            || demand.workplans.is_empty()
            || demand.max_lot.is_some_and(|v| !v.is_finite() || v <= 0.0)
            || demand
                .selected_workplan
                .as_ref()
                .is_some_and(|id| !demand.workplans.contains(id))
        {
            return Err(fail(
                &demand.id,
                "Invalid quantity, lot size or workplan selection",
            ));
        }
        let lot = demand.max_lot.unwrap_or(demand.quantity);
        let count = (demand.quantity / lot).ceil() as usize;
        if count > 100_000 {
            return Err(fail(
                &demand.id,
                "Too many lots; reduce expansion or supply canonical tasks",
            ));
        }
        let mut jobs = vec![];
        for i in 0..count {
            let quantity = (demand.quantity - i as f64 * lot).min(lot);
            let job = format!("{}/lot-{}", demand.id, i + 1);
            jobs.push(job.clone());
            p.jobs.push(Job {
                id: job.clone(),
                item: demand.item.clone(),
                quantity,
                due: demand.due,
                deadline: demand.deadline,
                priority: demand.priority,
            });
            let mut alternatives = vec![];
            for wp in &demand.workplans {
                let Some(plan) = plans.get(wp.as_str()) else {
                    return Err(fail(wp, "Unknown workplan"));
                };
                if plan.item != demand.item || plan.tasks.is_empty() {
                    return Err(fail(
                        wp,
                        "Workplan item differs from demand or contains no tasks",
                    ));
                }
                let prefix = format!("{job}/{wp}/");
                let mut tasks = vec![];
                for template in &plan.tasks {
                    let mut t = template.clone();
                    t.id = format!("{prefix}{}", template.id);
                    t.job = Some(job.clone());
                    t.quantity = quantity * template.quantity;
                    t.priority = demand.priority;
                    t.source = Some(format!(
                        "demand:{};lot:{};workplan:{};operation:{}",
                        demand.id,
                        i + 1,
                        wp,
                        template.id
                    ));
                    if t.execution.is_some() {
                        return Err(fail(
                            &t.id,
                            "Supply running work as canonical tasks, not repeated templates",
                        ));
                    }
                    let scale = |values: &mut std::collections::BTreeMap<String, f64>| {
                        *values = values
                            .iter()
                            .map(|(key, value)| (key.replace("$job", &job), value * t.quantity))
                            .collect();
                    };
                    scale(&mut t.consume);
                    scale(&mut t.produce);
                    for mode in &mut t.modes {
                        if let Some(v) = &mut mode.consume {
                            scale(v);
                        }
                        if let Some(v) = &mut mode.produce {
                            scale(v);
                        }
                    }
                    tasks.push(t.id.clone());
                    task_orders.insert(t.id.clone(), demand.id.clone());
                    p.tasks.push(t);
                    if p.tasks.len() > 1_000_000 {
                        return Err(fail(&p.id, "Expanded task limit exceeded"));
                    }
                }
                alternatives.push(RouteAlternative {
                    id: wp.clone(),
                    tasks,
                    dependencies: plan
                        .dependencies
                        .iter()
                        .map(|d| Dependency {
                            before: format!("{prefix}{}", d.before),
                            after: format!("{prefix}{}", d.after),
                            min_lag: d.min_lag,
                            max_lag: d.max_lag,
                        })
                        .collect(),
                });
            }
            p.routes.push(RouteChoice {
                id: job,
                alternatives,
                selected: demand.selected_workplan.clone(),
            });
        }
        p.orders.push(Order {
            id: demand.id.clone(),
            jobs,
            due: demand.due,
            deadline: demand.deadline,
            priority: demand.priority,
        });
    }
    // Supplied inter-order dependencies include every route; inactive endpoints vanish on selection.
    for demand in &input.demands {
        for predecessor in &demand.predecessors {
            if !demands.contains_key(predecessor.as_str()) || predecessor == &demand.id {
                return Err(fail(
                    &demand.id,
                    "Unknown or self-referential predecessor order",
                ));
            }
            for before in p
                .tasks
                .iter()
                .filter(|t| &task_orders[&t.id] == predecessor)
            {
                for after in p.tasks.iter().filter(|t| task_orders[&t.id] == demand.id) {
                    p.dependencies.push(Dependency {
                        before: before.id.clone(),
                        after: after.id.clone(),
                        min_lag: 0,
                        max_lag: None,
                    });
                    if p.dependencies.len() > 2_000_000 {
                        return Err(fail(
                            &p.id,
                            "Dependency expansion too large; supply explicit job/task edges",
                        ));
                    }
                }
            }
        }
    }
    if p.objectives.is_empty() {
        p.objectives = vec![
            Objective {
                metric: "order_weighted_tardiness".into(),
                weight: 1.0,
                priority: 0,
                ..Default::default()
            },
            Objective {
                metric: "makespan".into(),
                weight: 1.0,
                priority: 1,
                ..Default::default()
            },
        ];
    }
    crate::compile::compile(&p)?;
    Ok(p)
}
