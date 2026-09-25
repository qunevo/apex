use apex::{compile::compile, demo, model::*, validate, xg};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn problem(count: usize) -> Problem {
    let mut p = demo::problem(count);
    p.schema_version = "apex.v3.2".into();
    for (i, t) in p.tasks.iter_mut().enumerate() {
        t.id = format!("T{i}");
        t.modes = vec![demo::mode("M0", 10.0)];
        t.due = None;
        t.priority = 1.0;
    }
    p
}
fn conditional(
    id: &str,
    resource: &str,
    work: f64,
    after: Option<Vec<ActivityDependency>>,
) -> Conditional {
    Conditional {
        id: id.into(),
        modes: vec![demo::mode(resource, work)],
        after,
        ..Default::default()
    }
}

#[test]
fn route_selection_changes_presence_edges_materials_and_replays() {
    let mut p = problem(3);
    p.tasks[0].modes[0].phases[0].work = Some(100.0);
    p.tasks[0].produce.insert("part".into(), 1.0);
    p.tasks[1].produce.insert("part".into(), 1.0);
    p.tasks[2].consume.insert("part".into(), 1.0);
    p.routes = serde_json::from_value(json!([{"id":"route","alternatives":[
        {"id":"long","tasks":["T0"],"dependencies":[{"before":"T0","after":"T2"}]},
        {"id":"short","tasks":["T1"],"dependencies":[{"before":"T1","after":"T2"}]}]}]))
    .unwrap();
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(s.route_choices["route"], "short");
    assert_eq!(s.assignments.len(), 2);
    assert!(validate::validate(&p, &s).valid);
    let long = xg::create(
        &p,
        &Options {
            route_choices: BTreeMap::from([("route".into(), "long".into())]),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(long.metrics["makespan"] > s.metrics["makespan"]);
    let mut bad = s.clone();
    bad.route_choices.insert("route".into(), "long".into());
    assert!(!validate::validate(&p, &bad).valid);
    p.locks.push(Lock::Start {
        task: "T0".into(),
        at: 0,
    });
    assert_eq!(
        xg::create(&p, &Options::default()).unwrap().route_choices["route"],
        "long"
    );
}

#[test]
fn mode_conditionals_parallel_preparation_and_material_overrides() {
    let mut p = problem(1);
    p.tasks[0].consume.insert("unavailable".into(), 1.0);
    let a = &mut p.tasks[0].modes[0];
    a.consume = Some(BTreeMap::new());
    a.pre = vec![
        conditional("tool", "M1", 10.0, Some(vec![])),
        conditional("worker", "M2", 20.0, Some(vec![])),
        conditional(
            "join",
            "M3",
            5.0,
            Some(vec![
                ActivityDependency {
                    before: "tool".into(),
                    min_lag: 0,
                    max_lag: None,
                },
                ActivityDependency {
                    before: "worker".into(),
                    min_lag: 0,
                    max_lag: None,
                },
            ]),
        ),
    ];
    a.post = vec![
        conditional("inspect", "M1", 10.0, Some(vec![])),
        conditional("clean", "M2", 30.0, Some(vec![])),
    ];
    a.post[0].releases_product = true;
    let mut b = demo::mode("M3", 10.0);
    b.pre = vec![conditional("different", "M2", 70.0, None)];
    b.consume = Some(BTreeMap::new());
    p.tasks[0].modes.push(b);
    let s = xg::create(
        &p,
        &Options {
            mode_choices: BTreeMap::from([("T0".into(), "on-M0".into())]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        (
            s.assignments[0].start,
            s.assignments[0].end,
            s.assignments[0].ready
        ),
        (25, 35, 45)
    );
    assert_eq!(s.metrics["makespan"], 65.0);
    let b = xg::create(
        &p,
        &Options {
            mode_choices: BTreeMap::from([("T0".into(), "on-M3".into())]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(b.assignments[0].start, 70);
    assert_eq!(b.assignments[0].activities.len(), 2);
    let mut corrupt = s;
    corrupt.assignments[0].mode = "on-M3".into();
    assert!(!validate::validate(&p, &corrupt).valid);
}

#[test]
fn conditional_cycles_references_and_nested_work_are_rejected() {
    let mut p = problem(1);
    p.tasks[0].modes[0].pre = vec![
        conditional(
            "a",
            "M1",
            1.0,
            Some(vec![ActivityDependency {
                before: "b".into(),
                min_lag: 0,
                max_lag: None,
            }]),
        ),
        conditional("b", "M2", 1.0, None),
    ];
    assert!(
        compile(&p)
            .err()
            .unwrap()
            .iter()
            .any(|d| d.code == "CONDITIONAL_CYCLE")
    );
    p.tasks[0].modes[0].pre[0].after.as_mut().unwrap()[0].before = "absent".into();
    assert!(compile(&p).is_err());
    p.tasks[0].modes[0].pre = vec![conditional("outer", "M1", 1.0, None)];
    p.tasks[0].modes[0].pre[0].modes[0].pre = vec![conditional("nested", "M2", 1.0, None)];
    assert!(compile(&p).is_err());
}

#[test]
fn quantity_formulas_jobs_orders_and_corrupt_completions() {
    let mut p = problem(2);
    p.tasks[0].quantity = 5.0;
    p.tasks[0].modes[0].phases[0].work = Some(2.0);
    p.tasks[0].modes[0].phases[0].work_per_unit = Some(3.0);
    for t in &mut p.tasks {
        t.job = Some("J".into());
    }
    p.jobs = vec![Job {
        id: "J".into(),
        item: "part".into(),
        quantity: 5.0,
        due: Some(10),
        deadline: None,
        priority: 2.0,
    }];
    p.orders = vec![Order {
        id: "O".into(),
        jobs: vec!["J".into()],
        due: Some(5),
        deadline: None,
        priority: 3.0,
    }];
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(s.job_completions["J"], 27);
    assert_eq!(s.metrics["job_weighted_tardiness"], 34.0);
    assert_eq!(s.metrics["order_weighted_tardiness"], 66.0);
    let mut bad = s;
    bad.order_completions.insert("O".into(), 0);
    assert!(!validate::validate(&p, &bad).valid);
    p.orders[0].deadline = Some(20);
    assert!(xg::create(&p, &Options::default()).is_err());
}

#[test]
fn separate_sequence_penalties_drive_xh_and_are_revalidated() {
    let mut p = problem(2);
    p.tasks[0].family = "a".into();
    p.tasks[1].family = "b".into();
    for from in ["", "a", "b"] {
        for to in ["a", "b", "__end__"] {
            p.transitions.push(Transition {
                id: format!("{from}-{to}"),
                resource: "M0".into(),
                from: from.into(),
                to: to.into(),
                penalty: if from.is_empty() && to == "a" || from == "a" && to == "b" {
                    100.0
                } else {
                    0.0
                },
                ..Default::default()
            });
        }
    }
    p.objectives = vec![Objective {
        metric: "setup_penalty".into(),
        weight: 1.0,
        priority: 0,
        ..Default::default()
    }];
    let xg_result = xg::create(&p, &Options::default()).unwrap();
    let xh_result = apex::xh::search(
        &p,
        &Options {
            iterations: 12,
            budget_ms: 10000,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(xg_result.metrics["setup_penalty"], 200.0);
    assert_eq!(xh_result.metrics["setup_penalty"], 0.0);
    assert_eq!(xg_result.metrics["makespan"], xh_result.metrics["makespan"]);
    let mut bad = xh_result;
    bad.metrics.insert("setup_penalty".into(), 999.0);
    assert!(!validate::validate(&p, &bad).valid);
}

#[test]
fn sequence_context_rules_and_registered_native_hooks() {
    let mut p = problem(3);
    for (i, t) in p.tasks.iter_mut().enumerate() {
        t.family = if i == 1 { "b" } else { "a" }.into();
        t.due = Some(i as i64);
    }
    p.rules.push(Rule::SequencePattern {
        id: "return".into(),
        resource: "M0".into(),
        pattern: vec!["a".into(), "b".into(), "a".into()],
        previous_post: vec![],
        next_pre: vec![conditional("return-setup", "M1", 5.0, None)],
        penalty: 70.0,
    });
    p.customization = Some(CustomizationRef {
        id: "dummy_customer".into(),
        version: "1".into(),
    });
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(s.metrics["setup_penalty"], 170.0);
    assert!(s.metrics.contains_key("dummy_priority_completion"));
    assert!(validate::validate(&p, &s).valid);
    let mut bad = s;
    bad.metrics.insert("dummy_priority_completion".into(), 0.0);
    assert!(!validate::validate(&p, &bad).valid);
    p.customization.as_mut().unwrap().version = "missing".into();
    assert!(xg::create(&p, &Options::default()).is_err());
}

#[test]
fn production_expands_lots_routes_material_units_and_order_dependencies() {
    let mut base = problem(0);
    base.objectives.clear();
    base.inventory.insert("raw".into(), 5.0);
    let mut template = serde_json::to_value(&problem(1).tasks[0]).unwrap();
    template["consume"] = json!({"raw":1});
    template["produce"] = json!({"part":1});
    template["modes"][0]["phases"][0]["work"] = Value::Null;
    template["modes"][0]["phases"][0]["work_per_unit"] = json!(2);
    let input=serde_json::from_value(json!({"problem":base,"workplans":[{"id":"WP","item":"part","tasks":[template]}],"demands":[{"id":"ORDER","item":"part","quantity":5,"max_lot":3,"workplans":["WP"],"priority":1}]})).unwrap();
    let p = apex::production::expand(input).unwrap();
    assert_eq!(
        p.jobs.iter().map(|j| j.quantity).collect::<Vec<_>>(),
        vec![3.0, 2.0]
    );
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(s.metrics["makespan"], 10.0);
    assert_eq!(s.order_completions["ORDER"], 10);
    assert!(validate::validate(&p, &s).valid);

    let mut template = problem(1).tasks[0].clone();
    template.consume.insert("part".into(), 1.0);
    let input = serde_json::from_value(json!({
        "problem": problem(0),
        "workplans": [
            {"id":"supply","item":"part","tasks":[{
                "id":"make","modes":[demo::mode("M0", 10.0)],"produce":{"part":1}
            }]},
            {"id":"use","item":"assembly","tasks":[template]}
        ],
        "demands":[
            {"id":"SUPPLY","item":"part","quantity":1,"workplans":["supply"],"priority":1},
            {"id":"USE","item":"assembly","quantity":1,"workplans":["use"],"priority":10,"predecessors":["SUPPLY"]}
        ]
    })).unwrap();
    let p = apex::production::expand(input).unwrap();
    assert_eq!(p.dependencies.len(), 1);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert!(s.order_completions["SUPPLY"] < s.order_completions["USE"]);
    assert!(validate::validate(&p, &s).valid);
}

#[test]
fn independent_post_processing_does_not_block_a_released_machine() {
    let mut p = problem(2);
    p.tasks[0].due = Some(0);
    p.tasks[1].due = Some(100);
    p.tasks[0].post = vec![conditional("inspection", "M1", 100.0, Some(vec![]))];
    p.tasks[0].post[0].releases_product = true;
    p.tasks[1].deadline = Some(20);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(s.assignments[0].ready, 110);
    assert_eq!(s.assignments[1].start, 10);
    assert!(validate::validate(&p, &s).valid);
}

#[test]
fn frozen_sequence_does_not_become_a_product_release_dependency() {
    let mut p = problem(2);
    p.tasks[0].post = vec![conditional("inspection", "M1", 100.0, Some(vec![]))];
    p.tasks[0].post[0].releases_product = true;
    p.locks = vec![
        Lock::Order {
            resource: "M0".into(),
            tasks: vec!["T0".into(), "T1".into()],
            consecutive: true,
        },
        Lock::Start {
            task: "T0".into(),
            at: 0,
        },
        Lock::Start {
            task: "T1".into(),
            at: 10,
        },
    ];
    let options = Options {
        iterations: 8,
        budget_ms: 0,
        ..Default::default()
    };
    for s in [
        xg::create(&p, &options).unwrap(),
        apex::xh::search(&p, &options).unwrap(),
        apex::xt::search(&p, &options).unwrap(),
    ] {
        assert_eq!(s.assignments[0].ready, 110);
        assert_eq!(s.assignments[1].start, 10);
        assert!(validate::validate(&p, &s).valid);
        let mut reversed = p.clone();
        reversed.locks[0] = Lock::Order {
            resource: "M0".into(),
            tasks: vec!["T1".into(), "T0".into()],
            consecutive: true,
        };
        assert!(
            validate::validate(&reversed, &s)
                .diagnostics
                .iter()
                .any(|d| d.code == "LOCK_VIOLATION")
        );
    }
    // An actual material/operation dependency still waits for product release.
    p.dependencies.push(Dependency {
        before: "T0".into(),
        after: "T1".into(),
        min_lag: 0,
        max_lag: None,
    });
    assert!(xg::create(&p, &options).is_err());
}

#[test]
fn mode_override_cannot_silently_disappear_with_a_route() {
    let mut p = problem(2);
    p.routes=serde_json::from_value(json!([{"id":"choice","alternatives":[{"id":"A","tasks":["T0"]},{"id":"B","tasks":["T1"]}]}])).unwrap();
    let s = xg::create(
        &p,
        &Options {
            mode_choices: BTreeMap::from([("T1".into(), "on-M0".into())]),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s.route_choices["choice"], "B");
    assert!(
        xg::create(
            &p,
            &Options {
                mode_choices: BTreeMap::from([("absent".into(), "on-M0".into())]),
                ..Default::default()
            }
        )
        .is_err()
    );
}
