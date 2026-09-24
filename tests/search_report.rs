//! Opt-in measurements, not timing assertions: cargo test --release --test search_report -- --ignored
use apex::{compile, demo, engine, model::*, queues, search, validate};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};

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
#[ignore = "Produces release timing and proxy-quality report; run explicitly"]
fn measured_search_and_proxy_quality() {
    if cfg!(debug_assertions) {
        panic!("Measure the release build");
    }
    let mut timings = vec![];
    let production = apex::production::expand(
        serde_json::from_str(include_str!("../examples/production-orders.json")).unwrap(),
    )
    .unwrap();
    for (name, p) in [
        ("synthetic_100", demo::problem(100)),
        ("synthetic_1000", demo::problem(1000)),
        ("production_routes_conditionals", production),
    ] {
        for run in 0..3 {
            let base = Options {
                strategy: "queues".into(),
                iterations: 24,
                budget_ms: 0,
                trainer: TrainerConfig {
                    population_size: 8,
                    ..Default::default()
                },
                plus: PlusConfig {
                    depth: 4,
                    ..Default::default()
                },
                ..Default::default()
            };
            let fast = engine::create(&p, &base).unwrap();
            timings.push(json!({"dataset":name,"method":"fast","workers":1,"run":run,"milliseconds":fast.elapsed_ms,"score":fast.score,"evaluations":1}));
            for workers in [1, 4] {
                let mut o = base.clone();
                o.trainer.workers = workers;
                o.plus.workers = workers;
                for (method, s) in [
                    ("trainer", engine::train(&p, &o).unwrap()),
                    ("plus", search::plus(&p, &o).unwrap()),
                ] {
                    assert!(validate::validate(&p, &s).valid);
                    assert!(s.score <= fast.score);
                    timings.push(json!({"dataset":name,"method":method,"workers":workers,"run":run,"milliseconds":s.elapsed_ms,"score":s.score,"evaluations":s.evaluations,"search":s.search.as_ref().map(|r|json!({"generations":r.generations,"nodes":r.nodes,"revisits":r.revisits,"failed":r.failed_evaluations}))}));
                }
            }
        }
    }
    let mut quality = vec![];
    for seed in 0..40 {
        let mut p = demo::problem(12);
        let mut rng = search::Random(seed);
        p.objectives = vec![Objective {
            metric: "weighted_tardiness".into(),
            ..Default::default()
        }];
        for t in &mut p.tasks {
            t.due = Some(100 + rng.index(900) as i64);
            t.priority = (1 + rng.index(8)) as f64;
            for m in &mut t.modes {
                m.phases[0].work = Some((30 + rng.index(300)) as f64);
            }
        }
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
            let s = engine::create(
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
            observed.push(s.metrics["weighted_tardiness"]);
        }
        let rho = correlation(&predicted, &observed);
        let greedy = (0..predicted.len())
            .min_by(|a, b| predicted[*a].total_cmp(&predicted[*b]))
            .unwrap();
        let best = observed.iter().copied().fold(f64::INFINITY, f64::min);
        quality.push(json!({"seed":seed,"partition":if seed<20{"initial"}else{"holdout"},"candidates":candidates.len(),"spearman":rho,"selected_completion_tardiness":observed[greedy],"best_tested_completion_tardiness":best,"regret":observed[greedy]-best,"warning":rho.is_some_and(|r|r<0.0)}));
    }
    let mut medians = BTreeMap::<String, Vec<f64>>::new();
    for row in &timings {
        medians
            .entry(format!(
                "{} / {} / {} workers",
                row["dataset"].as_str().unwrap(),
                row["method"].as_str().unwrap(),
                row["workers"]
            ))
            .or_default()
            .push(row["milliseconds"].as_f64().unwrap());
    }
    let medians: BTreeMap<_, _> = medians
        .into_iter()
        .map(|(k, mut v)| {
            v.sort_by(f64::total_cmp);
            (k, v[v.len() / 2])
        })
        .collect();
    let report = json!({"engine":"0.3.0","profile":"release","os":std::env::consts::OS,"hardware_threads":std::thread::available_parallelism().map(|n|n.get()).unwrap_or(1),"timing_contract":"24 complete evaluations, no wall-clock limit, three repeats; includes compilation, construction, decoding and validation; API serialization excluded. Plus also reconstructs prefixes.","median_milliseconds":medians,"timings":timings,"proxy_quality_contract":"40 synthetic 12-task instances (20 initial and 20 holdout), first-decision normalized Q cost vs full greedy completion tardiness for each forced first choice. Spearman ties averaged; null means constant signal. This is a diagnostic, not a universal correlation guarantee.","proxy_quality":quality});
    std::fs::write(
        "docs/reports/v0.3-search-measurements.json",
        serde_json::to_string_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&medians).unwrap());
}
