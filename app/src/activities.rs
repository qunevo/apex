//! Dependency graphs within preparation and post-processing stages.
use crate::model::*;
use std::collections::{HashMap, VecDeque};
pub type StageEdges = Vec<Vec<(usize, Time, Option<Time>)>>;
pub type StagePlan = (Vec<usize>, StageEdges);

pub fn graph(specs: &[(String, &Conditional)], horizon: Time) -> Result<StagePlan, Diagnostic> {
    let mut ids = HashMap::new();
    for (i, (_, c)) in specs.iter().enumerate() {
        if c.id.is_empty() || ids.insert(c.id.as_str(), i).is_some() {
            return Err(Diagnostic::new(
                "CONDITIONAL_ID",
                &c.id,
                "Conditional IDs must be unique within the active stage",
            ));
        }
    }
    let mut edges: StageEdges = vec![vec![]; specs.len()];
    let mut successors = vec![vec![]; specs.len()];
    for (i, (_, c)) in specs.iter().enumerate() {
        if let Some(after) = &c.after {
            for edge in after {
                let Some(&before) = ids.get(edge.before.as_str()) else {
                    return Err(Diagnostic::new(
                        "CONDITIONAL_REFERENCE",
                        &c.id,
                        "Dependency must reference an active conditional in the same stage",
                    ));
                };
                if edge.min_lag < 0
                    || edge.min_lag > horizon
                    || edge
                        .max_lag
                        .is_some_and(|v| v < edge.min_lag || v > horizon)
                {
                    return Err(Diagnostic::new(
                        "CONDITIONAL_LAG",
                        &c.id,
                        "Invalid conditional lag",
                    ));
                }
                edges[i].push((before, edge.min_lag, edge.max_lag));
            }
        } else if i > 0 {
            edges[i].push((i - 1, 0, None));
        }
        for &(before, _, _) in &edges[i] {
            successors[before].push(i);
        }
    }
    let mut counts: Vec<_> = edges.iter().map(Vec::len).collect();
    let mut ready: VecDeque<_> = counts
        .iter()
        .enumerate()
        .filter_map(|(i, n)| (*n == 0).then_some(i))
        .collect();
    let mut order = vec![];
    while let Some(i) = ready.pop_front() {
        order.push(i);
        for &next in &successors[i] {
            counts[next] -= 1;
            if counts[next] == 0 {
                ready.push_back(next);
            }
        }
    }
    if order.len() != specs.len() {
        return Err(Diagnostic::new(
            "CONDITIONAL_CYCLE",
            "activities",
            "Conditional dependencies contain a cycle",
        ));
    }
    Ok((order, edges))
}
