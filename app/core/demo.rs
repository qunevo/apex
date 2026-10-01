use crate::model::*;
use std::collections::BTreeMap;

pub fn problem(count: usize) -> Problem {
    let horizon = (count as i64 * 900).max(86400);
    let resources = (0..4)
        .map(|i| Resource {
            id: format!("M{i}"),
            capacity: 1.0,
            calendar: vec![Window {
                start: 0,
                end: horizon,
                capacity: 1.0,
                rate: 1.0,
            }],
            retention_calendar: vec![],
            initial_state: String::new(),
        })
        .collect();
    let tasks = (0..count)
        .map(|i| Task {
            id: format!("T{i:06}"),
            release: 0,
            due: Some(600 + (i as i64 % 23) * 240),
            deadline: None,
            priority: 1.0 + (i % 5) as f64,
            family: String::new(),
            modes: vec![
                mode(&format!("M{}", i % 4), 60.0 + (i % 17) as f64 * 30.0),
                mode(&format!("M{}", (i + 1) % 4), 90.0 + (i % 17) as f64 * 40.0),
            ],
            pre: vec![],
            post: vec![],
            consume: BTreeMap::new(),
            produce: BTreeMap::new(),
            attributes: BTreeMap::from([("urgency".into(), 1.0 + (i % 7) as f64)]),
            execution: None,
            source: Some(format!("synthetic:row:{}", i + 1)),
            quantity: 1.0,
            ..Default::default()
        })
        .collect();
    Problem {
        schema_version: "apex.v3.1".into(),
        id: "synthetic-factory".into(),
        horizon,
        epoch: Some("2026-09-24T06:00:00Z".into()),
        resources,
        tasks,
        dependencies: vec![],
        locks: vec![],
        transitions: vec![],
        inventory: BTreeMap::new(),
        receipts: vec![],
        objectives: vec![
            Objective {
                metric: "weighted_tardiness".into(),
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
        ],
        rules: vec![],
        assumptions: vec![],
        ..Default::default()
    }
}
pub fn mode(resource: &str, work: f64) -> Mode {
    Mode {
        id: format!("on-{resource}"),
        primary: resource.into(),
        phases: vec![Phase {
            id: "run".into(),
            work: Some(work),
            interruption: Interrupt::CalendarResumable,
            rate_resource: Some(resource.into()),
            requirements: vec![Requirement {
                resource: resource.into(),
                amount: 1.0,
                retain: true,
            }],
            ..Default::default()
        }],
        cost: 0.0,
        contiguous: false,
        ..Default::default()
    }
}
