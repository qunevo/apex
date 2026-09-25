//! Diagnostic of the group-delivery Q mapping; results are not timing assertions.
use apex::{compile, demo, model::*, queues, search, xg};
use serde_json::json;
use std::collections::HashMap;
fn rank(values: &[f64]) -> Vec<f64> {
    values
        .iter()
        .map(|v| {
            let less = values.iter().filter(|x| *x < v).count();
            let equal = values.iter().filter(|x| *x == v).count();
            less as f64 + (equal.saturating_sub(1)) as f64 / 2.0
        })
        .collect()
}
fn correlation(a: &[f64], b: &[f64]) -> Option<f64> {
    let a = rank(a);
    let b = rank(b);
    let ma = a.iter().sum::<f64>() / a.len() as f64;
    let mb = b.iter().sum::<f64>() / b.len() as f64;
    let cov = a
        .iter()
        .zip(&b)
        .map(|(a, b)| (a - ma) * (b - mb))
        .sum::<f64>();
    let den = (a.iter().map(|a| (a - ma).powi(2)).sum::<f64>()
        * b.iter().map(|b| (b - mb).powi(2)).sum::<f64>())
    .sqrt();
    (den > 0.0).then_some(cov / den)
}

#[test]
#[ignore = "Writes release-only proxy diagnostics"]
fn measured_group_delivery_proxy() {
    if cfg!(debug_assertions) {
        panic!("Measure the release build");
    }
    let mut quality = vec![];
    for seed in 0..24 {
        let mut p = demo::problem(8);
        let mut rng = search::Random(seed + 200);
        p.objectives = vec![Objective {
            metric: "job_on_time_delivery".into(),
            maximize: true,
            ..Default::default()
        }];
        p.jobs = (0..4)
            .map(|i| Job {
                id: format!("J{i}"),
                item: "synthetic".into(),
                quantity: 1.0,
                due: Some(25 + rng.index(90) as i64),
                deadline: None,
                priority: 1.0,
            })
            .collect();
        for (i, t) in p.tasks.iter_mut().enumerate() {
            t.due = None;
            t.job = Some(format!("J{}", i / 2));
            t.release = rng.index(15) as i64;
            t.modes = vec![demo::mode("M0", (1 + rng.index(20)) as f64)];
        }
        let (active, _) = apex::domain::resolve(&p, &Options::default()).unwrap();
        let p = active.into_owned();
        let c = compile::compile(&p).unwrap();
        let remaining = queues::remaining(&c);
        let tails = vec![0.0; p.resources.len()];
        let ready = vec![0.0; p.tasks.len()];
        let history = HashMap::new();
        let defs = queues::definitions(&p, None);
        let policy = queues::default_policy(&p, &defs);
        let state = queues::DispatchState {
            tails: &tails,
            ready: &ready,
            remaining: &remaining,
            history: &history,
            definitions: &defs,
            extension: None,
        };
        let mut candidates = vec![];
        for (i, t) in p.tasks.iter().enumerate() {
            for mi in 0..t.modes.len() {
                candidates.push(queues::candidate(&c, i, mi, &state).unwrap());
            }
        }
        let predicted = queues::scores(&p, &mut candidates, &policy);
        let mut observed = vec![];
        for candidate in &candidates {
            let t = &p.tasks[candidate.task];
            let s = xg::create(
                &p,
                &Options {
                    strategy: "queues".into(),
                    decision_prefix: vec![Decision {
                        task: t.id.clone(),
                        mode: t.modes[candidate.mode].id.clone(),
                    }],
                    ..Default::default()
                },
            )
            .unwrap();
            observed.push(1.0 - s.metrics["job_on_time_delivery"]);
        }
        let rho = correlation(&predicted, &observed);
        let greedy = (0..predicted.len())
            .min_by(|a, b| predicted[*a].total_cmp(&predicted[*b]))
            .unwrap();
        let best = observed.iter().copied().fold(f64::INFINITY, f64::min);
        quality.push(json!({"seed":seed,"partition":if seed<12{"initial"}else{"holdout"},"candidates":candidates.len(),"spearman":rho,"selected_delivery_loss":observed[greedy],"best_tested_delivery_loss":best,"regret":observed[greedy]-best,"warning":rho.is_some_and(|r|r<0.0)}));
    }
    let report = json!({"contract":"24 synthetic eight-operation/four-job models; cases 12..23 held out. First-choice Q scores versus final lost job-delivery fraction for all eight forced-first completions. Lower is better for both; negative Spearman exposes proxy failure. Regret is relative to these tested completions, not an optimum. Standard due/slack/short-work mapping is experimental for grouped delivery.","rows":quality});
    std::fs::write(
        "docs/reports/proxy-measurements.json",
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
}
