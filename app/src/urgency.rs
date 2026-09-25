//! Derived dispatch signals. Contractual dates, priorities and objective values stay unchanged.
use crate::model::*;
use serde::Serialize;
use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashMap},
};

#[derive(Clone, Copy, Debug, Serialize)]
pub struct Urgency {
    pub due: Option<Time>,
    pub priority: f64,
}

/// Propagate the earliest due date and highest priority through physical dependencies.
/// Job members share urgency. Resource-order locks do not imply customer demand.
/// Multi-source heap traversal also handles cycles created by grouping interleaved jobs.
pub fn derive(p: &Problem) -> Vec<Urgency> {
    let mut groups = HashMap::new();
    let mut nodes = Vec::with_capacity(p.tasks.len());
    for t in &p.tasks {
        let key = (t.job.is_some(), t.job.as_deref().unwrap_or(&t.id));
        let next = groups.len();
        nodes.push(*groups.entry(key).or_insert(next));
    }
    let mut values = vec![
        Urgency {
            due: None,
            priority: 0.0
        };
        groups.len()
    ];
    for (t, &i) in p.tasks.iter().zip(&nodes) {
        if let Some(due) = t.due {
            values[i].due = Some(values[i].due.map_or(due, |d| d.min(due)));
        }
        values[i].priority = values[i].priority.max(t.priority);
    }
    // A more relaxed operation/job date must not hide a tighter sales-order promise.
    let mut include = |job: &str, due: Option<Time>, priority: f64| {
        if let Some(&i) = groups.get(&(true, job)) {
            if let Some(due) = due {
                values[i].due = Some(values[i].due.map_or(due, |d| d.min(due)));
            }
            values[i].priority = values[i].priority.max(priority);
        }
    };
    for job in &p.jobs {
        include(&job.id, job.due, job.priority);
    }
    for order in &p.orders {
        for job in &order.jobs {
            include(job, order.due, order.priority);
        }
    }
    let ids: HashMap<_, _> = p
        .tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), nodes[i]))
        .collect();
    let mut predecessors = vec![vec![]; groups.len()];
    for d in &p.dependencies {
        if let (Some(&a), Some(&b)) = (ids.get(d.before.as_str()), ids.get(d.after.as_str()))
            && a != b
        {
            predecessors[b].push(a);
        }
    }
    if p.dependencies.is_empty() {
        return nodes.into_iter().map(|i| values[i]).collect();
    }
    let mut dates = BinaryHeap::new();
    let mut priorities = BinaryHeap::new();
    for (i, u) in values.iter().enumerate() {
        if let Some(due) = u.due {
            dates.push(Reverse((due, i)));
        }
        priorities.push((u.priority.to_bits(), i));
    }
    let mut seen = vec![false; groups.len()];
    while let Some(Reverse((due, i))) = dates.pop() {
        if std::mem::replace(&mut seen[i], true) {
            continue;
        }
        values[i].due = Some(due);
        for &j in &predecessors[i] {
            if !seen[j] {
                dates.push(Reverse((due, j)));
            }
        }
    }
    seen.fill(false);
    while let Some((priority, i)) = priorities.pop() {
        if std::mem::replace(&mut seen[i], true) {
            continue;
        }
        values[i].priority = f64::from_bits(priority);
        for &j in &predecessors[i] {
            if !seen[j] {
                priorities.push((priority, j));
            }
        }
    }
    nodes.into_iter().map(|i| values[i]).collect()
}
