use apex::{compile, demo, engine, model::*, queues, rules, search, validate};
use serde_json::json;
use std::collections::BTreeMap;

fn problem(n: usize) -> Problem {
    let mut p = demo::problem(n);
    for (i, t) in p.tasks.iter_mut().enumerate() {
        t.id = format!("T{i}");
        t.stage = "0".into();
        t.priority = 1.0;
        t.due = None;
        t.modes = vec![demo::mode("M0", 10.0)];
    }
    p
}
fn sequence(s: &Schedule) -> Vec<&str> {
    let mut rows = s.assignments.iter().collect::<Vec<_>>();
    rows.sort_by_key(|a| a.start);
    rows.iter().map(|a| a.task.as_str()).collect()
}
fn options() -> Options {
    Options {
        iterations: 48,
        budget_ms: 0,
        trainer: TrainerConfig {
            population_size: 8,
            mutation_rate: 1.0,
            crossover_rate: 1.0,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn policy(weights: &[(&str, f64)]) -> QueuePolicy {
    QueuePolicy {
        stages: BTreeMap::from([(
            "*".into(),
            weights.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        )]),
        candidate_limit: 0,
        ..Default::default()
    }
}

#[test]
fn queue_normalization_matches_legacy_minmax_and_robust_formula() {
    assert_eq!(queues::normalize(&[4.0, 4.0], "robust"), vec![0.0, 0.0]);
    assert_eq!(
        queues::normalize(&[10.0, 20.0, 30.0], "minmax"),
        vec![0.0, 0.5, 1.0]
    );
    let actual = queues::normalize(&[0.0, 1.0, 2.0, 3.0, 100.0], "robust");
    for (v, x) in actual.iter().zip([0.0, 1.0, 2.0, 3.0, 100.0]) {
        assert!((v - 1.0 / (1.0 + (-((x - 2.0) / 2.0_f64)).exp())).abs() < 1e-12);
    }
}

#[test]
fn queues_combine_normalized_costs_and_respect_numeric_stages() {
    let mut p = problem(3);
    p.tasks[0].stage = "10".into();
    p.tasks[0].modes[0].phases[0].work = Some(1.0);
    p.tasks[1].stage = "2".into();
    p.tasks[1].modes[0].phases[0].work = Some(30.0);
    p.tasks[2].stage = "2".into();
    p.tasks[2].modes[0].phases[0].work = Some(5.0);
    let s = engine::create(
        &p,
        &Options {
            queue_policy: Some(policy(&[("shortest_work", 4.0)])),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(sequence(&s), vec!["T2", "T1", "T0"]);
    assert!(validate::validate(&p, &s).valid);
}

#[test]
fn custom_objective_generates_proxy_and_explicit_override_controls_direction_and_weight() {
    let mut p = problem(2);
    p.rules=serde_json::from_value(json!([{"kind":"attribute_objective","id":"service","attribute":"urgency","weight":3,"priority":0}])).unwrap();
    p.objectives = vec![Objective {
        metric: "service".into(),
        weight: 0.0,
        ..Default::default()
    }];
    let defs = queues::definitions(&p, None);
    assert_eq!(
        queues::default_policy(&p, &defs).stages["*"]["attribute:service"],
        0.0
    );
    p.objectives[0].weight = 7.0;
    p.objectives[0].maximize = true;
    assert_eq!(
        queues::default_policy(&p, &defs).stages["*"]["reverse:attribute:service"],
        7.0
    );
    p.queue_definitions = vec![QueueDefinition {
        id: "missing".into(),
        metric: "service".into(),
        attribute: "absent".into(),
        weight: 1.0,
        prefer_high: false,
    }];
    assert!(
        compile::compile(&p)
            .err()
            .unwrap()
            .iter()
            .any(|e| e.code == "QUEUE_ATTRIBUTE")
    );
}

#[test]
fn metric_direction_scale_priority_and_corrupted_vectors_are_checked() {
    let mut p = problem(2);
    p.tasks[0].due = Some(10);
    p.objectives = vec![
        Objective {
            metric: "on_time_delivery".into(),
            maximize: true,
            priority: 0,
            ..Default::default()
        },
        Objective {
            metric: "makespan".into(),
            weight: 3.0,
            scale: 2.0,
            priority: 1,
            ..Default::default()
        },
    ];
    let s = engine::create(&p, &Options::default()).unwrap();
    assert_eq!(s.score, vec![-1.0, 30.0]);
    assert_eq!(s.objective_values, vec![-1.0, 10.0]);
    let mut bad = s.clone();
    bad.objective_values[0] = 2.0;
    assert!(!validate::validate(&p, &bad).valid);
    let mut bad = s;
    bad.score[1] = 0.0;
    assert!(!validate::validate(&p, &bad).valid);
    p.objectives.push(p.objectives[0].clone());
    assert!(compile::compile(&p).is_err());
}

#[test]
fn trainer_honors_generations_evaluations_and_produces_replayable_elite() {
    let p = problem(10);
    let mut o = options();
    o.iterations = 0;
    o.trainer.generations = Some(2);
    let s = engine::train(&p, &o).unwrap();
    let r = s.search.as_ref().unwrap();
    assert_eq!(r.generations, 2);
    assert_eq!(r.evaluations, 24);
    assert_eq!(r.stop_reason, "generation_limit");
    assert_eq!(r.mutations, 16);
    assert_eq!(r.crossovers, 16);
    assert!(s.score <= engine::create(&p, &o).unwrap().score);
    let replay = engine::create(&p, s.replay.as_ref().unwrap()).unwrap();
    assert_eq!(replay.score, s.score);
    assert_eq!(
        serde_json::to_value(replay.assignments).unwrap(),
        serde_json::to_value(&s.assignments).unwrap()
    );
    o.iterations = 11;
    o.trainer.generations = None;
    let s = engine::train(&p, &o).unwrap();
    assert_eq!(s.evaluations, 11);
    assert_eq!(s.search.unwrap().stop_reason, "evaluation_limit");
}

#[test]
fn time_limit_and_unbounded_search_rejection() {
    let p = problem(80);
    let mut o = options();
    o.iterations = 0;
    o.budget_ms = 1;
    let s = engine::train(&p, &o).unwrap();
    assert_eq!(s.search.unwrap().stop_reason, "time_limit");
    o.budget_ms = 0;
    assert!(engine::train(&p, &o).is_err());
    o.trainer.generations = Some(1);
    assert!(search::plus(&p, &o).is_err());
}

#[test]
fn overflowing_objective_is_a_diagnostic_not_a_search_panic() {
    let mut p = problem(2);
    p.objectives = vec![Objective {
        metric: "makespan".into(),
        weight: 1.0,
        scale: 1e-310,
        ..Default::default()
    }];
    assert!(
        engine::create(&p, &Options::default())
            .unwrap_err()
            .iter()
            .any(|e| e.code == "NUMERIC_OVERFLOW")
    );
    assert!(engine::train(&p, &options()).is_err());
}

#[test]
fn parallel_trainer_matches_serial_with_fixed_evaluation_budget() {
    let p = demo::problem(35);
    let a = engine::train(&p, &options()).unwrap();
    let mut o = options();
    o.trainer.workers = 4;
    let b = engine::train(&p, &o).unwrap();
    assert_eq!(a.score, b.score);
    assert_eq!(a.objective_values, b.objective_values);
    assert_eq!(
        serde_json::to_value(a.assignments).unwrap(),
        serde_json::to_value(b.assignments).unwrap()
    );
    assert_eq!(a.search.unwrap().mutations, b.search.unwrap().mutations);
}

#[test]
fn crossover_exchanges_stage_blocks_and_mutation_preserves_protected_gene() {
    let mut a = options();
    let mut b = options();
    a.queue_policy = Some(policy(&[
        ("due", 1.0),
        ("slack", 2.0),
        ("deadline_interval_fit", 4.0),
    ]));
    a.queue_policy
        .as_mut()
        .unwrap()
        .stages
        .insert("second".into(), BTreeMap::from([("due".into(), 8.0)]));
    b.queue_policy = Some(policy(&[
        ("due", 8.0),
        ("slack", 16.0),
        ("deadline_interval_fit", 4.0),
    ]));
    b.queue_policy
        .as_mut()
        .unwrap()
        .stages
        .insert("second".into(), BTreeMap::from([("due".into(), 2.0)]));
    let mut rng = search::Random(7);
    let mut mixed = false;
    for _ in 0..50 {
        let c = search::crossover(&a, &b, &mut rng);
        let cp = c.queue_policy.unwrap();
        for (stage, weights) in &cp.stages {
            assert!(
                *weights == a.queue_policy.as_ref().unwrap().stages[stage]
                    || *weights == b.queue_policy.as_ref().unwrap().stages[stage]
            );
        }
        mixed |= cp.stages["*"] == a.queue_policy.as_ref().unwrap().stages["*"]
            && cp.stages["second"] == b.queue_policy.as_ref().unwrap().stages["second"];
    }
    assert!(mixed);
    for _ in 0..100 {
        search::mutate(&problem(2), &mut a, &Options::default(), &mut rng);
        assert_eq!(
            a.queue_policy.as_ref().unwrap().stages["*"]["deadline_interval_fit"],
            4.0
        );
    }
}

#[test]
fn pareto_archive_keeps_conflicting_goals_and_every_entry_replays() {
    let mut p = problem(2);
    p.tasks[0].attributes.insert("urgency".into(), 100.0);
    p.tasks[1].attributes.insert("urgency".into(), 1.0);
    p.tasks[1].modes[0].phases[0].work = Some(1.0);
    p.rules=serde_json::from_value(json!([{"kind":"attribute_objective","id":"service","attribute":"urgency","weight":1,"priority":0}])).unwrap();
    p.objectives = vec![
        Objective {
            metric: "flow_time".into(),
            ..Default::default()
        },
        Objective {
            metric: "service".into(),
            ..Default::default()
        },
    ];
    let mut o = options();
    o.trainer.selection = "pareto".into();
    let s = engine::train(&p, &o).unwrap();
    let archive = &s.search.as_ref().unwrap().archive;
    assert_eq!(archive.len(), 2);
    for entry in archive {
        let candidate = engine::create(&p, &entry.options).unwrap();
        assert_eq!(candidate.objective_values, entry.objectives);
        assert!(validate::validate(&p, &candidate).valid);
    }
    assert!(!search::dominates(
        &archive[0].objectives,
        &archive[1].objectives
    ));
    assert!(!search::dominates(
        &archive[1].objectives,
        &archive[0].objectives
    ));
}

#[test]
fn plus_backtracks_after_infeasible_baseline_and_revisits_branches() {
    let mut p = problem(2);
    p.tasks[0].modes[0].phases[0].work = Some(100.0);
    p.tasks[0].due = Some(1);
    p.tasks[1].due = Some(10);
    p.tasks[1].deadline = Some(10);
    let mut o = options();
    o.plus.depth = 2;
    o.plus.workers = 2;
    assert!(engine::create(&p, &o).is_err());
    let s = search::plus(&p, &o).unwrap();
    let report = s.search.as_ref().unwrap();
    assert_eq!(sequence(&s)[0], "T1");
    assert!(report.failed_evaluations > 0);
    assert!(report.nodes > 2);
    assert!(report.revisits > 0);
    assert_eq!(report.evaluations, 48);
    assert!(validate::validate(&p, &s).valid);
    let again = search::plus(&p, &o).unwrap();
    assert_eq!(again.score, s.score);
    assert_eq!(again.search.unwrap().nodes, report.nodes);
}

#[test]
fn every_search_preserves_start_resource_mode_and_consecutive_sequence_locks() {
    let mut p = problem(4);
    p.locks = serde_json::from_value(json!([
        {"kind":"start","task":"T0","at":0},
        {"kind":"resource","task":"T0","resource":"M0"},
        {"kind":"mode","task":"T0","mode":p.tasks[0].modes[0].id},
        {"kind":"order","resource":"M0","tasks":["T0","T2"],"consecutive":true}
    ]))
    .unwrap();
    for s in [
        engine::train(&p, &options()).unwrap(),
        search::plus(&p, &options()).unwrap(),
    ] {
        assert!(validate::validate(&p, &s).valid);
        assert_eq!(s.assignments[0].task, "T0");
        assert_eq!(s.assignments[0].start, 0);
        assert_eq!(sequence(&s)[1], "T2");
    }
}

#[test]
fn native_metric_contract_parallel_queues_and_independent_validation() {
    let mut p = demo::problem(15);
    p.customization = Some(CustomizationRef {
        id: "dummy_customer".into(),
        version: "1".into(),
    });
    p.objectives = vec![Objective {
        metric: "dummy_priority_completion".into(),
        ..Default::default()
    }];
    let mut o = options();
    o.trainer.workers = 3;
    o.strategy = "queues".into();
    let s = engine::train(&p, &o).unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert_eq!(s.score[0], s.metrics["dummy_priority_completion"]);
    let mut corrupt = s;
    corrupt
        .metrics
        .insert("dummy_priority_completion".into(), -1.0);
    assert!(!validate::validate(&p, &corrupt).valid);
    p.customization = None;
    assert!(compile::compile(&p).is_err());
}

#[test]
fn objective_change_reverses_the_actual_preferred_schedule() {
    let mut p = problem(2);
    p.tasks[1].modes[0].phases[0].work = Some(1.0);
    p.objectives = vec![Objective {
        metric: "flow_time".into(),
        ..Default::default()
    }];
    let minimum = engine::train(&p, &options()).unwrap();
    p.objectives[0].maximize = true;
    let maximum = engine::train(&p, &options()).unwrap();
    assert!(minimum.metrics["flow_time"] < maximum.metrics["flow_time"]);
    assert_eq!(rules::score(&p, &maximum.metrics), maximum.score);
}

#[test]
fn automatic_attribute_proxy_matches_weighted_completion_pairwise_optimum() {
    for work in [1.0, 7.0, 100.0, 1000.0] {
        for weight in [1.0, 3.0, 50.0] {
            let mut p = problem(2);
            p.tasks[0].modes[0].phases[0].work = Some(work);
            p.tasks[0].attributes.insert("urgency".into(), weight);
            p.tasks[1].attributes.insert("urgency".into(), 2.0);
            p.rules=serde_json::from_value(json!([{"kind":"attribute_objective","id":"service","attribute":"urgency","weight":1,"priority":0}])).unwrap();
            p.objectives = vec![Objective {
                metric: "service".into(),
                ..Default::default()
            }];
            let o = Options {
                queue_policy: Some(policy(&[("attribute:service", 1.0)])),
                ..Default::default()
            };
            let best = engine::create(&p, &o).unwrap();
            for t in &p.tasks {
                let other = engine::create(
                    &p,
                    &Options {
                        decision_prefix: vec![Decision {
                            task: t.id.clone(),
                            mode: t.modes[0].id.clone(),
                        }],
                        ..o.clone()
                    },
                )
                .unwrap();
                assert!(best.score <= other.score);
            }
        }
    }
}
