//! Fictional example: penalize a return to a family after intervening work.
use crate::{
    extensions::{Metric, SequenceContext, TaskDecoration},
    model::*,
    rules::Customization,
};
pub struct Policy;
impl Customization for Policy {
    fn id(&self) -> &str {
        "dummy_customer"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, input: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        let mut p = input.clone();
        p.customization = None;
        Ok(p)
    }
    fn supports_prefix_decoration(&self) -> bool {
        true
    }
    fn sequence(
        &self,
        context: &SequenceContext<'_>,
    ) -> Result<Vec<TaskDecoration>, Vec<Diagnostic>> {
        let mut seen = std::collections::HashSet::new();
        let mut result = vec![];
        for (i, task) in context.tasks.iter().enumerate() {
            if i > 0 && context.tasks[i - 1].family != task.family && seen.contains(&task.family) {
                result.push(TaskDecoration {
                    task: task.id.clone(),
                    penalty: 100.0,
                    ..Default::default()
                });
            }
            seen.insert(&task.family);
        }
        Ok(result)
    }
    fn dispatch_rank(&self, _: &Problem, t: &Task) -> f64 {
        -t.attributes.get("urgency").copied().unwrap_or(0.0)
    }
    fn queue_definitions(&self, _: &Problem) -> Vec<QueueDefinition> {
        vec![QueueDefinition {
            id: "dummy:priority_completion".into(),
            metric: "dummy_priority_completion".into(),
            attribute: "urgency".into(),
            weight: 1.0,
            prefer_high: true,
        }]
    }
    fn objectives(&self, _: &Problem) -> Vec<Objective> {
        vec![Objective {
            metric: "dummy_priority_completion".into(),
            priority: 200,
            ..Default::default()
        }]
    }
    fn queue_value(&self, id: &str, context: &crate::queues::Context<'_>) -> Option<f64> {
        (id == "dummy:priority_completion").then(|| {
            context.task.priority
                / context
                    .mode
                    .phases
                    .iter()
                    .map(|p| p.work.unwrap_or(0.0))
                    .sum::<f64>()
                    .max(1e-9)
        })
    }
    fn metrics(&self, p: &Problem, s: &Schedule) -> Result<Vec<Metric>, Vec<Diagnostic>> {
        let tasks: std::collections::HashMap<_, _> = p.tasks.iter().map(|t| (&t.id, t)).collect();
        Ok(vec![Metric {
            id: "dummy_priority_completion".into(),
            value: s
                .assignments
                .iter()
                .map(|a| a.ready as f64 * tasks[&a.task].priority)
                .sum(),
            weight: 1.0,
            priority: 200,
        }])
    }
}
