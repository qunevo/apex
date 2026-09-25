use apex::{compile, demo, improve, material, model::*, validate::validate, xg};
use serde_json::json;

fn options() -> Options {
    Options {
        strategy: "queues".into(),
        iterations: 48,
        budget_ms: 0,
        xh: XhConfig {
            population_size: 4,
            mutation_rate: 1.0,
            crossover_rate: 1.0,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn fixture() -> Problem {
    let mut p = demo::problem(2);
    p.id = "synthetic-conditional-resources".into();
    p.dependencies.clear();
    p.horizon = 1000;
    for r in &mut p.resources {
        r.calendar = vec![Window {
            start: 0,
            end: 1000,
            capacity: 1.0,
            rate: 1.0,
        }];
    }
    p.tasks[0].id = "A".into();
    p.tasks[1].id = "B".into();
    p.tasks[0].modes = vec![demo::mode("M0", 1.0)];
    p.tasks[1].modes = vec![demo::mode("M1", 100.0)];
    for t in &mut p.tasks {
        t.due = None;
        t.release = 0;
    }
    let mut slow = demo::mode("M2", 12.0);
    slow.id = "slow-free-resource".into();
    slow.cost = 3.0;
    let mut fast = demo::mode("M1", 10.0);
    fast.id = "fast-shared-resource".into();
    fast.cost = 1.0;
    p.tasks[0].pre = vec![Conditional {
        id: "setup".into(),
        modes: vec![fast, slow],
        ..Default::default()
    }];
    p.objectives = vec![Objective {
        metric: "makespan".into(),
        ..Default::default()
    }];
    p
}
fn prefix(p: &Problem) -> Options {
    let mut o = options();
    o.decision_prefix = vec![Decision {
        task: "A".into(),
        mode: p.tasks[0].modes[0].id.clone(),
    }];
    o
}

#[test]
fn conditional_alternatives_improve_complete_schedules_in_both_searches_and_replay() {
    let p = fixture();
    let o = prefix(&p);
    let initial = xg::create(&p, &o).unwrap();
    assert_eq!(initial.metrics["makespan"], 110.0);
    for s in [
        apex::xh::search_with(&p, &o, None).unwrap(),
        apex::xt::search(&p, &o).unwrap(),
        improve::improve(&p, &o).unwrap(),
    ] {
        assert_eq!(s.metrics["makespan"], 100.0, "{}", s.strategy);
        assert_eq!(
            s.assignments
                .iter()
                .find(|a| a.task == "A")
                .unwrap()
                .activities[0]
                .mode,
            "slow-free-resource"
        );
        assert!(validate(&p, &s).valid);
        let replay = xg::create(&p, s.replay.as_ref().unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&s.assignments).unwrap(),
            serde_json::to_value(&replay.assignments).unwrap()
        );
    }
}

#[test]
fn conditional_commitments_conflicts_corruption_and_parallel_search() {
    let mut p = fixture();
    let mut o = prefix(&p);
    o.conditional_choices.insert(
        "A".into(),
        [("pre:setup".into(), "slow-free-resource".into())].into(),
    );
    let s = xg::create(&p, &o).unwrap();
    let mut parallel = o.clone();
    parallel.xh.workers = 2;
    let a = apex::xh::search_with(&p, &o, None).unwrap();
    let b = apex::xh::search_with(&p, &parallel, None).unwrap();
    assert_eq!(a.score, b.score);
    assert_eq!(
        a.replay.unwrap().conditional_choices,
        b.replay.unwrap().conditional_choices
    );
    p.tasks[0]
        .conditional_modes
        .insert("pre:setup".into(), "fast-shared-resource".into());
    assert!(
        xg::create(&p, &o)
            .unwrap_err()
            .iter()
            .any(|e| e.code == "CONDITIONAL_LOCK")
    );
    assert!(!validate(&p, &s).valid);
    let mut bad = fixture();
    bad.tasks[0]
        .conditional_modes
        .insert("pre:absent".into(), "unknown".into());
    assert!(compile::compile(&bad).is_err());
    let mut corrupt = s.clone();
    corrupt.assignments[0].activities[0].mode = "fast-shared-resource".into();
    assert!(!validate(&fixture(), &corrupt).valid);
    assert!(
        improve::improve_from(
            &fixture(),
            &Options {
                conditional_choices: [(
                    "A".into(),
                    [("pre:setup".into(), "fast-shared-resource".into())].into()
                )]
                .into(),
                ..o
            },
            None,
            Some(&s)
        )
        .is_err()
    );
}

#[test]
fn post_choices_and_governed_prefixes_use_the_same_conditional_genes() {
    let mut p = fixture();
    p.tasks[0].post = std::mem::take(&mut p.tasks[0].pre);
    p.planning=serde_json::from_value(json!({"version":"apex.planning.v1","policies":[{"kind":"idle_gap","id":"gap","resource":"M0","maximum":1000,"fallback_to_smallest":false}]})).unwrap();
    let o = prefix(&p);
    let s = apex::xt::search(&p, &o).unwrap();
    assert_eq!(s.metrics["makespan"], 100.0);
    assert!(validate(&p, &s).valid);
    assert!(s.dispatch.is_some());
    assert!(s.replay.as_ref().unwrap().conditional_choices["A"].contains_key("post:setup"));
}

#[test]
fn urgency_propagates_across_jobs_and_merges_without_changing_business_objectives() {
    let mut p = demo::problem(4);
    p.dependencies = serde_json::from_value(json!([
        {"before":"T000000","after":"T000002"},{"before":"T000002","after":"T000003"}]))
    .unwrap();
    for t in &mut p.tasks {
        t.due = Some(900);
        t.priority = 1.0;
        t.modes = vec![demo::mode("M0", 10.0)];
    }
    p.tasks[3].due = Some(20);
    p.tasks[3].priority = 9.0;
    p.jobs = vec![Job {
        id: "upstream".into(),
        item: "part".into(),
        quantity: 1.0,
        due: Some(900),
        deadline: None,
        priority: 1.0,
    }];
    p.tasks[0].job = Some("upstream".into());
    p.tasks[1].job = Some("upstream".into());
    let original = serde_json::to_value(&p).unwrap();
    let (active, _) = apex::domain::resolve(&p, &options()).unwrap();
    let c = compile::compile(&active).unwrap();
    for u in &c.urgency {
        assert_eq!(u.due, Some(20));
        assert_eq!(u.priority, 9.0);
    }
    assert_eq!(active.jobs[0].due, Some(900));
    assert_eq!(active.tasks[0].due, Some(900));
    let s = xg::create(&p, &options()).unwrap();
    assert_eq!(s.metrics["job_weighted_tardiness"], 0.0);
    assert_eq!(serde_json::to_value(&p).unwrap(), original);
    assert!(validate(&p, &s).valid);
    // Fixed resource order is not a production dependency and must not propagate urgency.
    p.dependencies.clear();
    p.locks = vec![Lock::Order {
        resource: "M0".into(),
        tasks: vec![p.tasks[0].id.clone(), p.tasks[3].id.clone()],
        consecutive: false,
    }];
    let c = compile::compile(&p).unwrap();
    assert_eq!(c.urgency[0].due, Some(900));
}

#[test]
fn urgency_changes_dispatch_of_a_critical_upstream_operation() {
    let mut p = demo::problem(3);
    p.dependencies =
        serde_json::from_value(json!([{"before":"T000000","after":"T000002"}])).unwrap();
    for t in &mut p.tasks {
        t.modes = vec![demo::mode("M0", 10.0)];
        t.priority = 1.0;
        t.release = 0;
    }
    p.tasks[0].due = Some(900);
    p.tasks[1].due = Some(100);
    p.tasks[2].due = Some(20);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(
        s.assignments.iter().min_by_key(|a| a.start).unwrap().task,
        "T000000"
    );
    assert!(
        s.assignments
            .iter()
            .find(|a| a.task == "T000002")
            .unwrap()
            .ready
            <= 20
    );
}

fn material_routes() -> Problem {
    let mut p = demo::problem(4);
    p.dependencies.clear();
    for (i, t) in p.tasks.iter_mut().enumerate() {
        t.modes = vec![demo::mode("M0", if i == 0 { 50.0 } else { 10.0 })];
        t.due = Some(500);
        t.release = 0;
        t.consume.clear();
        t.produce.clear();
    }
    p.tasks[0].consume.insert("raw".into(), 2.0);
    p.tasks[1].consume.insert("raw".into(), 2.0);
    p.tasks[0].produce.insert("part".into(), 2.0);
    p.tasks[1].produce.insert("part".into(), 2.0);
    p.tasks[2].consume.insert("part".into(), 1.0);
    p.tasks[3].consume.insert("part".into(), 1.0);
    p.inventory.insert("raw".into(), 2.0);
    p.routes=serde_json::from_value(json!([{"id":"producer","alternatives":[{"id":"slow","tasks":["T000000"]},{"id":"fast","tasks":["T000001"]}]}])).unwrap();
    p.objectives = vec![Objective {
        metric: "makespan".into(),
        ..Default::default()
    }];
    p
}

#[test]
fn pegging_preserves_routes_and_reallocates_actual_producers_without_double_supply() {
    let p = material_routes();
    let original = serde_json::to_value(&p).unwrap();
    let (q, preview) = material::prepare(&p, &Default::default()).unwrap();
    assert!(preview.preview);
    assert_eq!(q.routes.len(), 1);
    assert_eq!(q.tasks.len(), 4);
    for (route, producer, end) in [("slow", "T000000", 70.0), ("fast", "T000001", 30.0)] {
        let o = Options {
            route_choices: [("producer".into(), route.into())].into(),
            ..options()
        };
        for s in [
            xg::create(&q, &o).unwrap(),
            apex::xh::search_with(&q, &o, None).unwrap(),
            apex::xt::search(&q, &o).unwrap(),
        ] {
            assert_eq!(s.metrics["makespan"], end);
            assert!(validate(&q, &s).valid);
            let report = s.material_report.as_ref().unwrap();
            let produced: f64 = report
                .allocations
                .iter()
                .filter(|a| a.item == "part")
                .map(|a| {
                    assert_eq!(a.source_task.as_deref(), Some(producer));
                    a.quantity
                })
                .sum();
            assert_eq!(produced, 2.0);
            let mut bad = s.clone();
            bad.material_report.as_mut().unwrap().allocations[0].quantity += 1.0;
            assert!(!validate(&q, &bad).valid);
            let replay = xg::create(&q, s.replay.as_ref().unwrap()).unwrap();
            assert_eq!(replay.material_report, s.material_report);
        }
    }
    assert_eq!(serde_json::to_value(&p).unwrap(), original);
    assert!(material::prepare(&q, &Default::default()).is_err());
}

#[test]
fn material_shortage_in_one_route_does_not_discard_a_feasible_alternative() {
    let mut p = material_routes();
    p.tasks[1].consume.insert("missing".into(), 1.0);
    let (q, preview) = material::prepare(&p, &Default::default()).unwrap();
    assert!(!preview.preview_diagnostics.is_empty());
    let bad = Options {
        route_choices: [("producer".into(), "fast".into())].into(),
        ..options()
    };
    assert!(
        xg::create(&q, &bad)
            .unwrap_err()
            .iter()
            .any(|e| e.code == "MATERIAL_UNRESOLVED")
    );
    let good = Options {
        route_choices: [("producer".into(), "slow".into())].into(),
        ..options()
    };
    assert!(validate(&q, &xg::create(&q, &good).unwrap()).valid);
    let s = apex::xt::search(&q, &options()).unwrap();
    assert_eq!(s.route_choices["producer"], "slow");
}

#[test]
fn new_kpis_have_explicit_group_units_and_are_independently_checked() {
    let mut p = demo::problem(2);
    p.dependencies.clear();
    for t in &mut p.tasks {
        t.modes = vec![demo::mode("M0", 10.0)];
        t.release = 0;
        t.due = Some(15);
        t.job = Some("J".into());
    }
    p.jobs = serde_json::from_value(
        json!([{"id":"J","item":"part","quantity":1,"due":15,"priority":1}]),
    )
    .unwrap();
    p.orders =
        serde_json::from_value(json!([{"id":"O","jobs":["J"],"due":25,"priority":1}])).unwrap();
    let s = xg::create(&p, &options()).unwrap();
    assert_eq!(s.metrics["on_time_delivery"], 1.0);
    assert_eq!(s.metrics["task_on_time_delivery"], 0.5);
    assert_eq!(s.metrics["job_on_time_delivery"], 0.0);
    assert_eq!(s.metrics["order_on_time_delivery"], 1.0);
    assert_eq!(s.metrics["job_tardiness"], 5.0);
    assert_eq!(s.metrics["order_tardiness"], 0.0);
    assert_eq!(s.metrics["resource:M0:utilization"], 1.0);
    let mut corrupt = s.clone();
    corrupt.metrics.insert("job_on_time_delivery".into(), 1.0);
    assert!(!validate(&p, &corrupt).valid);
    corrupt = s.clone();
    corrupt.metrics.remove("job_on_time_delivery");
    assert!(!validate(&p, &corrupt).valid);
    corrupt.metrics.clear();
    assert!(!validate(&p, &corrupt).valid);
    let mut old = s.clone();
    old.metric_version = 0;
    old.metrics
        .retain(|k, _| apex::metrics::ORIGINAL.contains(&k.as_str()));
    assert!(validate(&p, &old).valid);
    p.objectives = vec![Objective {
        metric: "job_on_time_delivery".into(),
        maximize: true,
        ..Default::default()
    }];
    assert!(!apex::queues::objective_queues("job_on_time_delivery").is_empty());
    assert!(validate(&p, &apex::xh::search_with(&p, &options(), None).unwrap()).valid);
}

#[test]
fn productive_utilization_excludes_pauses_and_counts_conditional_cost_separately() {
    let p = fixture();
    let s = xg::create(&p, &prefix(&p)).unwrap();
    assert_eq!(s.metrics["conditional_time"], 10.0);
    assert_eq!(s.metrics["conditional_mode_cost"], 1.0);
    assert!((s.metrics["resource:M1:utilization"] - 100.0 / 110.0).abs() < 1e-9);
    let mut p = demo::problem(1);
    p.tasks[0].modes = vec![demo::mode("M0", 15.0)];
    p.tasks[0].modes[0].phases[0].interruption = Interrupt::CalendarResumable;
    p.tasks[0].release = 0;
    p.resources[0].calendar = vec![
        Window {
            start: 0,
            end: 10,
            capacity: 1.0,
            rate: 1.0,
        },
        Window {
            start: 20,
            end: 1000,
            capacity: 1.0,
            rate: 1.0,
        },
    ];
    let s = xg::create(&p, &options()).unwrap();
    assert_eq!(s.metrics["resource:M0:productive_time"], 15.0);
    assert_eq!(s.metrics["resource:M0:available_time"], 15.0);
    assert_eq!(s.metrics["resource:M0:utilization"], 1.0);
}

#[test]
fn mode_specific_conditionals_and_depth_boundary_remain_searchable() {
    let mut p = fixture();
    let mut other = p.tasks[0].modes[0].clone();
    other.id = "with-preparation".into();
    other.pre = std::mem::take(&mut p.tasks[0].pre);
    p.tasks[0].modes.push(other);
    p.tasks[0]
        .conditional_modes
        .insert("pre:setup".into(), "slow-free-resource".into());
    let s = xg::create(&p, &options()).unwrap();
    assert_eq!(
        s.assignments.iter().find(|a| a.task == "A").unwrap().mode,
        "with-preparation"
    );
    let p = fixture();
    let mut o = prefix(&p);
    o.xt.depth = 1;
    assert_eq!(apex::xt::search(&p, &o).unwrap().metrics["makespan"], 100.0);
}

#[test]
fn restart_and_transition_alternatives_are_pinned_and_revalidated() {
    let mut p = fixture();
    let restart = std::mem::take(&mut p.tasks[0].pre);
    let t = &mut p.tasks[0];
    t.execution = Some(Execution {
        mode: t.modes[0].id.clone(),
        as_of: 0,
        actual: Activity {
            id: "actual".into(),
            task: t.id.clone(),
            role: "actual".into(),
            mode: t.modes[0].id.clone(),
            start: 0,
            end: 0,
            segments: vec![],
            reservations: vec![],
        },
        remaining_work: [("run".into(), 1.0)].into(),
        restart,
    });
    t.conditional_modes
        .insert("restart:setup".into(), "slow-free-resource".into());
    let s = apex::xt::search(&p, &prefix(&p)).unwrap();
    assert!(validate(&p, &s).valid);
    assert!(
        s.assignments[0]
            .activities
            .iter()
            .any(|a| a.id == "A:restart:setup" && a.mode == "slow-free-resource")
    );
    let mut p = fixture();
    let pre = std::mem::take(&mut p.tasks[0].pre);
    p.transitions = vec![
        Transition {
            id: "initial".into(),
            resource: "M0".into(),
            from: p.resources[0].initial_state.clone(),
            to: p.tasks[0].family.clone(),
            next_pre: pre,
            ..Default::default()
        },
        Transition {
            id: "terminal".into(),
            resource: "M0".into(),
            from: p.tasks[0].family.clone(),
            to: "__end__".into(),
            ..Default::default()
        },
    ];
    let s = apex::xt::search(&p, &prefix(&p)).unwrap();
    assert_eq!(s.metrics["makespan"], 100.0);
    assert_eq!(
        s.replay.as_ref().unwrap().conditional_choices["A"]["transition:initial:pre:setup"],
        "slow-free-resource"
    );
    assert!(validate(&p, &s).valid);
    let mut wrong = s.replay.unwrap();
    wrong
        .conditional_choices
        .get_mut("A")
        .unwrap()
        .insert("transition:terminal:post:absent".into(), "unknown".into());
    assert!(xg::create(&p, &wrong).is_err());
}

#[test]
fn native_compile_can_supply_searchable_conditional_alternatives() {
    struct AddPreparation;
    impl apex::rules::Customization for AddPreparation {
        fn id(&self) -> &str {
            "synthetic-preparation"
        }
        fn version(&self) -> &str {
            "1"
        }
        fn compile(&self, p: &Problem) -> Result<Problem, Vec<Diagnostic>> {
            let mut p = p.clone();
            p.tasks[0].pre = fixture().tasks[0].pre.clone();
            Ok(p)
        }
    }
    let mut p = fixture();
    p.tasks[0].pre.clear();
    let o = prefix(&p);
    for s in [
        apex::xh::search_with(&p, &o, Some(&AddPreparation)).unwrap(),
        apex::xt::search_customized(&p, &o, Some(&AddPreparation)).unwrap(),
    ] {
        assert_eq!(s.metrics["makespan"], 100.0);
        let (lowered, replay) =
            xg::create_customized(&p, s.replay.as_ref().unwrap(), &AddPreparation).unwrap();
        assert!(apex::extensions::validate_lowered(&lowered, &s, &AddPreparation).valid);
        assert_eq!(s.metrics, replay.metrics);
    }
}

#[test]
fn metric_catalog_units_and_reserved_names_are_unambiguous() {
    let mut p = fixture();
    for (id, unit) in [
        ("transition_work", "work_units"),
        ("weighted_tardiness", "priority_seconds"),
        ("resource:M0:productive_time", "capacity_seconds"),
        ("task_on_time_delivery", "fraction"),
    ] {
        assert_eq!(
            apex::metrics::catalog(&p)
                .iter()
                .find(|r| r["id"] == id)
                .unwrap()["unit"],
            unit
        );
    }
    p.rules.push(Rule::AttributeObjective {
        id: "task_on_time_delivery".into(),
        attribute: "urgency".into(),
        weight: 1.0,
        priority: 0,
    });
    assert!(
        compile::compile(&p)
            .err()
            .unwrap()
            .iter()
            .any(|e| e.code == "RESERVED_METRIC")
    );
}

#[test]
fn fractional_objectives_and_native_metrics_survive_json_persistence() {
    let mut p: Problem = apex::production::expand(
        serde_json::from_str(include_str!("../examples/production-orders.json")).unwrap(),
    )
    .unwrap();
    p.customization = Some(CustomizationRef {
        id: "dummy_customer".into(),
        version: "1".into(),
    });
    for metric in ["dummy_priority_completion", "task_on_time_delivery"] {
        p.objectives = vec![Objective {
            metric: metric.into(),
            maximize: metric.ends_with("delivery"),
            ..Default::default()
        }];
        let s = apex::xh::search_with(&p, &options(), None).unwrap();
        let restored: Schedule = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(s.metrics, restored.metrics);
        assert_eq!(s.score, restored.score);
        assert!(validate(&p, &restored).valid);
        let mut bad = restored.clone();
        bad.metrics.insert("task_on_time_delivery".into(), 42.0);
        assert!(!validate(&p, &bad).valid);
        let mut old = restored;
        old.metric_version = 0;
        old.metrics.retain(|id, _| {
            apex::metrics::ORIGINAL.contains(&id.as_str())
                || id == metric
                || id == "dummy_priority_completion"
        });
        assert!(validate(&p, &old).valid);
    }
}
