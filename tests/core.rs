use apex::{compile::compile, demo, engine, model::*, validate::validate};
use std::collections::BTreeMap;

fn one() -> Problem {
    let mut p = demo::problem(1);
    p.tasks[0].modes = vec![demo::mode("M0", 90.0)];
    p.tasks[0].due = None;
    p
}
fn window(start: i64, end: i64, rate: f64) -> Window {
    Window {
        start,
        end,
        capacity: 1.0,
        rate,
    }
}
fn run(p: &Problem) -> Schedule {
    engine::create(p, &Options::default()).unwrap_or_else(|e| panic!("{e:#?}"))
}
fn conditional(id: &str, resource: &str, work: f64, release: bool) -> Conditional {
    Conditional {
        id: id.into(),
        modes: vec![demo::mode(resource, work)],
        releases_product: release,
        ..Default::default()
    }
}

#[test]
fn c01_resumable_break_and_retention() {
    let mut p = one();
    p.tasks[0].release = 30;
    p.resources[0].calendar = vec![window(0, 60, 1.0), window(90, 1000, 1.0)];
    let s = run(&p);
    let a = &s.assignments[0];
    assert_eq!((a.start, a.end), (30, 150));
    assert_eq!(
        a.activities[0].segments.iter().map(|x| x.work).sum::<f64>(),
        90.0
    );
    assert_eq!(a.activities[0].reservations[0].end, 150);
}
#[test]
fn c02_non_interruptible_waits() {
    let mut p = one();
    p.tasks[0].release = 30;
    p.tasks[0].modes[0].phases[0].interruption = Interrupt::NonInterruptible;
    p.resources[0].calendar = vec![window(0, 60, 1.0), window(90, 1000, 1.0)];
    assert_eq!(
        (run(&p).assignments[0].start, run(&p).assignments[0].end),
        (90, 180)
    );
}
#[test]
fn c03_shift_rate_change() {
    let mut p = one();
    p.tasks[0].modes[0].phases[0].work = Some(60.0);
    p.tasks[0].release = 30;
    p.resources[0].calendar = vec![window(0, 60, 1.0), window(60, 1000, 0.5)];
    assert_eq!(run(&p).assignments[0].end, 120);
}
#[test]
fn c04_worker_released_during_break() {
    let mut p = one();
    p.resources[0].calendar = vec![window(0, 60, 1.0), window(90, 1000, 1.0)];
    p.resources[1].calendar = p.resources[0].calendar.clone();
    p.tasks[0].modes[0].phases[0]
        .requirements
        .push(Requirement {
            resource: "M1".into(),
            amount: 1.0,
            retain: false,
        });
    let s = run(&p);
    let rs: Vec<_> = s.assignments[0].activities[0]
        .reservations
        .iter()
        .filter(|r| r.resource == "M1")
        .collect();
    assert_eq!(rs.len(), 2);
    assert_eq!((rs[0].end, rs[1].start), (60, 90));
}
#[test]
fn c05_phase_staffing_and_handover() {
    let mut p = one();
    let mut load = p.tasks[0].modes[0].phases[0].clone();
    load.id = "handover".into();
    load.work = Some(10.0);
    load.requirements.push(Requirement {
        resource: "M1".into(),
        amount: 1.0,
        retain: false,
    });
    p.tasks[0].modes[0].phases.insert(0, load);
    let s = run(&p);
    assert_eq!(s.assignments[0].end, 100);
    assert_eq!(
        s.assignments[0].activities[0]
            .reservations
            .iter()
            .find(|r| r.resource == "M1")
            .unwrap()
            .end,
        10
    );
}
#[test]
fn c06_retention_forbidden_during_break() {
    let mut p = one();
    p.resources[0].calendar = vec![window(0, 60, 1.0), window(90, 1000, 1.0)];
    p.resources[0].retention_calendar = p.resources[0].calendar.clone();
    let s = run(&p);
    assert!(s.assignments[0].start >= 90);
    assert!(validate(&p, &s).valid);
}
#[test]
fn c07_conditional_own_calendar() {
    let mut p = one();
    p.tasks[0].pre.push(conditional("setup", "M1", 10.0, false));
    p.resources[1].calendar = vec![window(100, 1000, 1.0)];
    let s = run(&p);
    assert_eq!(s.assignments[0].start, 110);
    assert_eq!(s.assignments[0].activities.len(), 2);
}
#[test]
fn c08_post_cleaning_vs_inspection() {
    let mut p = one();
    p.tasks[0]
        .post
        .push(conditional("clean", "M0", 10.0, false));
    let s = run(&p);
    assert_eq!(s.assignments[0].ready, 90);
    assert_eq!(s.assignments[0].activities[1].end, 100);
    p.tasks[0].post[0].releases_product = true;
    assert_eq!(run(&p).assignments[0].ready, 100);
}
fn transition_problem() -> Problem {
    let mut p = one();
    let mut b = p.tasks[0].clone();
    b.id = "B".into();
    b.family = "b".into();
    b.due = Some(10);
    p.tasks[0].family = "a".into();
    p.tasks[0].due = Some(0);
    p.tasks.push(b);
    for from in ["", "a", "b"] {
        for to in ["a", "b", "__end__"] {
            p.transitions.push(Transition {
                id: format!("{from}-{to}"),
                resource: "M0".into(),
                from: from.into(),
                to: to.into(),
                previous_post: if !from.is_empty() && from != to {
                    vec![conditional(
                        "wash",
                        "M0",
                        if from == "a" { 11.0 } else { 23.0 },
                        false,
                    )]
                } else {
                    vec![]
                },
                next_pre: vec![],
                ..Default::default()
            });
        }
    }
    p
}
#[test]
fn c09_neighbor_dependent_previous_post() {
    let mut p = transition_problem();
    let s = run(&p);
    assert_eq!(
        s.assignments[0].activities.iter().last().unwrap().segments[0].work,
        11.0
    );
    p.tasks[1].due = Some(0);
    p.tasks[0].due = Some(10);
    let b = run(&p);
    assert_eq!(b.assignments[1].start, 0);
    assert_eq!(b.assignments[0].start, 113);
}
#[test]
fn c10_terminal_post_and_missing_transition() {
    let mut p = transition_problem();
    assert!(
        run(&p).assignments[1]
            .activities
            .iter()
            .any(|a| a.id.contains("b-__end__"))
    );
    p.transitions.retain(|tr| tr.to != "__end__");
    let e = engine::create(&p, &Options::default()).unwrap_err();
    assert_eq!(e[0].code, "MISSING_TRANSITION");
}
#[test]
fn c11_resource_lock_leaves_time_free() {
    let mut p = demo::problem(1);
    p.locks.push(Lock::Resource {
        task: p.tasks[0].id.clone(),
        resource: "M1".into(),
    });
    p.tasks[0].release = 200;
    let s = run(&p);
    assert_eq!(s.assignments[0].primary, "M1");
    assert_eq!(s.assignments[0].start, 200);
}
#[test]
fn c12_sequence_block_prevents_interleaving() {
    let mut p = one();
    let a = p.tasks[0].clone();
    p.tasks = (0..3)
        .map(|i| {
            let mut t = a.clone();
            t.id = format!("T{i}");
            t.due = Some([0, 100, 10][i]);
            t
        })
        .collect();
    p.locks.push(Lock::Order {
        resource: "M0".into(),
        tasks: vec!["T0".into(), "T1".into()],
        consecutive: true,
    });
    let s = run(&p);
    assert_eq!(s.assignments[1].start, 90);
    assert_eq!(s.assignments[2].start, 180);
}
#[test]
fn c14_frozen_time_conflict() {
    let mut p = one();
    p.locks.push(Lock::Start {
        task: p.tasks[0].id.clone(),
        at: 0,
    });
    p.resources[0].calendar = vec![window(50, 1000, 1.0)];
    assert!(
        engine::create(&p, &Options::default())
            .unwrap_err()
            .iter()
            .any(|e| e.code == "LOCK_CONFLICT")
    );
}
#[test]
fn c15_running_work_and_post() {
    let mut p = one();
    let t = &mut p.tasks[0];
    t.pre.push(conditional("already-done", "M0", 50.0, false));
    t.post.push(conditional("inspect", "M1", 10.0, true));
    t.execution = Some(Execution {
        mode: "on-M0".into(),
        as_of: 30,
        actual: Activity {
            id: "actual-1".into(),
            task: t.id.clone(),
            role: "actual".into(),
            mode: "on-M0".into(),
            start: 0,
            end: 30,
            segments: vec![Segment {
                phase: "run".into(),
                start: 0,
                end: 30,
                work: 30.0,
            }],
            reservations: vec![Reservation {
                resource: "M0".into(),
                start: 0,
                end: 30,
                amount: 1.0,
            }],
        },
        remaining_work: BTreeMap::from([("run".into(), 60.0)]),
        restart: vec![],
    });
    let s = run(&p);
    assert_eq!(
        (
            s.assignments[0].start,
            s.assignments[0].end,
            s.assignments[0].ready
        ),
        (0, 90, 100)
    );
    assert!(
        !s.assignments[0]
            .activities
            .iter()
            .any(|a| a.id.contains("already-done"))
    );
}
#[test]
fn c16_restart_work() {
    let mut p = one();
    let t = &mut p.tasks[0];
    t.execution = Some(Execution {
        mode: "on-M0".into(),
        as_of: 30,
        actual: Activity {
            id: "actual".into(),
            task: t.id.clone(),
            role: "actual".into(),
            mode: "on-M0".into(),
            start: 0,
            end: 0,
            segments: vec![],
            reservations: vec![],
        },
        remaining_work: BTreeMap::from([("run".into(), 90.0)]),
        restart: vec![conditional("restart", "M0", 15.0, false)],
    });
    assert_eq!(run(&p).assignments[0].end, 135);
}
#[test]
fn c17_material_and_precedence() {
    let mut p = one();
    let mut b = p.tasks[0].clone();
    b.id = "B".into();
    b.modes = vec![demo::mode("M1", 10.0)];
    b.consume.insert("WIP".into(), 2.0);
    p.tasks[0].produce.insert("WIP".into(), 2.0);
    p.dependencies.push(Dependency {
        before: p.tasks[0].id.clone(),
        after: "B".into(),
        min_lag: 5,
        max_lag: Some(5),
    });
    p.tasks.push(b);
    let s = run(&p);
    assert_eq!(s.assignments[1].start, 95);
}
#[test]
fn c18_missing_work_and_unknown_fields() {
    let mut p = one();
    p.tasks[0].modes[0].phases[0].work = None;
    assert!(
        compile(&p)
            .err()
            .unwrap()
            .iter()
            .any(|e| e.code == "MISSING_PROCESSING_TIME")
    );
    let mut v = serde_json::to_value(one()).unwrap();
    v["unsupported_constraint"] = serde_json::json!(true);
    assert!(serde_json::from_value::<Problem>(v).is_err());
}
#[test]
fn zero_rate_is_pause_not_division() {
    let mut p = one();
    p.resources[0].calendar = vec![
        window(0, 30, 1.0),
        window(30, 60, 0.0),
        window(60, 1000, 1.0),
    ];
    assert_eq!(run(&p).assignments[0].end, 120);
}

#[test]
fn contiguous_phases_shift_together() {
    let mut p = one();
    let mut second = p.tasks[0].modes[0].phases[0].clone();
    p.tasks[0].modes[0].phases[0].work = Some(10.0);
    second.id = "finish".into();
    second.work = Some(20.0);
    second.requirements.push(Requirement {
        resource: "M1".into(),
        amount: 1.0,
        retain: false,
    });
    p.resources[1].calendar = vec![window(100, 1000, 1.0)];
    p.tasks[0].modes[0].phases.push(second);
    p.tasks[0].modes[0].contiguous = true;
    let s = run(&p);
    assert_eq!((s.assignments[0].start, s.assignments[0].end), (90, 120));
    p.tasks[0].modes[0].contiguous = false;
    assert_eq!(run(&p).assignments[0].start, 0);
}

#[test]
fn running_machine_stays_reserved_until_resume() {
    let mut p = one();
    let t = &mut p.tasks[0];
    t.execution = Some(Execution {
        mode: "on-M0".into(),
        as_of: 30,
        actual: Activity {
            id: "actual".into(),
            task: t.id.clone(),
            role: "actual".into(),
            mode: "on-M0".into(),
            start: 0,
            end: 30,
            segments: vec![Segment {
                phase: "run".into(),
                start: 0,
                end: 30,
                work: 30.0,
            }],
            reservations: vec![Reservation {
                resource: "M0".into(),
                start: 0,
                end: 30,
                amount: 1.0,
            }],
        },
        remaining_work: BTreeMap::from([("run".into(), 60.0)]),
        restart: vec![],
    });
    p.resources[0].calendar = vec![window(0, 30, 1.0), window(90, 1000, 1.0)];
    let s = run(&p);
    assert_eq!(
        (
            s.assignments[0].retained[0].start,
            s.assignments[0].retained[0].end
        ),
        (30, 90)
    );
    let mut bad = s;
    bad.assignments[0].retained.clear();
    assert!(!validate(&p, &bad).valid);
    p.resources[0].retention_calendar = p.resources[0].calendar.clone();
    assert!(engine::create(&p, &Options::default()).is_err());
}

#[test]
fn pooled_capacity_allows_parallel_work_and_rejects_overbooking() {
    let mut p = demo::problem(2);
    p.resources[2].capacity = 2.0;
    p.resources[2].calendar[0].capacity = 2.0;
    for (i, t) in p.tasks.iter_mut().enumerate() {
        t.modes = vec![demo::mode(&format!("M{i}"), 90.0)];
        t.modes[0].phases[0].requirements.push(Requirement {
            resource: "M2".into(),
            amount: 1.0,
            retain: false,
        });
    }
    let s = run(&p);
    assert_eq!(s.assignments[0].start, 0);
    assert_eq!(s.assignments[1].start, 0);
    p.resources[2].capacity = 1.0;
    p.resources[2].calendar[0].capacity = 1.0;
    assert!(!validate(&p, &s).valid);
    let s = run(&p);
    let mut starts: Vec<_> = s.assignments.iter().map(|a| a.start).collect();
    starts.sort_unstable();
    assert_eq!(starts, vec![0, 90]);
}
#[test]
fn cycle_and_unknown_fixation_rejected() {
    let mut p = one();
    p.dependencies.push(Dependency {
        before: p.tasks[0].id.clone(),
        after: p.tasks[0].id.clone(),
        min_lag: 0,
        max_lag: None,
    });
    assert!(compile(&p).err().unwrap().iter().any(|e| e.code == "CYCLE"));
    p.dependencies.clear();
    p.locks.push(Lock::Start {
        task: "absent".into(),
        at: 0,
    });
    assert!(compile(&p).is_err());
}
#[test]
fn trainer_preserves_valid_incumbent() {
    let p = demo::problem(100);
    let baseline = run(&p);
    let trained = engine::train(
        &p,
        &Options {
            iterations: 12,
            budget_ms: 60000,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(trained.score <= baseline.score);
    assert_eq!(trained.evaluations, 12);
    assert!(validate(&p, &trained).valid);
}

#[test]
fn weighted_training_is_reproducible_and_replayable() {
    let p = demo::problem(50);
    let options = Options {
        iterations: 15,
        budget_ms: 60000,
        seed: 88,
        ..Default::default()
    };
    let a = engine::train(&p, &options).unwrap();
    let b = engine::train(&p, &options).unwrap();
    assert_eq!(a.score, b.score);
    assert_eq!(a.strategy, b.strategy);
    assert_eq!(a.seed, b.seed);
    let replay = engine::create(&p, a.replay.as_ref().unwrap()).unwrap();
    assert_eq!(a.score, replay.score);
    assert_eq!(a.evaluations, 15);
}
#[test]
fn objective_generates_matching_dispatch_signal() {
    let mut p = one();
    let mut b = p.tasks[0].clone();
    b.id = "urgent".into();
    b.attributes.insert("urgency".into(), 100.0);
    p.tasks.push(b);
    p.rules.push(Rule::AttributeObjective {
        id: "urgent_completion".into(),
        attribute: "urgency".into(),
        weight: 1.0,
        priority: 0,
    });
    let s = engine::create(
        &p,
        &Options {
            strategy: "objective".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(s.assignments[1].start, 0);
    assert!(s.metrics.contains_key("urgent_completion"));
}
#[test]
fn task_window_is_hard() {
    let mut p = one();
    p.rules.push(Rule::TaskWindow {
        id: "late".into(),
        task: p.tasks[0].id.clone(),
        earliest: 200,
        latest: 400,
    });
    let s = run(&p);
    assert_eq!(s.assignments[0].start, 200);
    let mut bad = s.clone();
    bad.assignments[0].start = 0;
    assert!(!validate(&p, &bad).valid);
}
#[test]
fn validator_rejects_corrupted_work_reservations_and_kpis() {
    let p = one();
    let s = run(&p);
    let mut wrong = s.clone();
    wrong.assignments[0].activities[0].segments[0].work = 1.0;
    assert!(!validate(&p, &wrong).valid);
    let mut wrong = s.clone();
    wrong.assignments[0].activities[0].reservations.clear();
    assert!(!validate(&p, &wrong).valid);
    let mut wrong = s.clone();
    wrong.metrics.insert("makespan".into(), 0.0);
    assert!(!validate(&p, &wrong).valid);
    let mut wrong = s;
    wrong.assignments.clear();
    assert!(!validate(&p, &wrong).valid);
}
#[test]
fn seeded_synthetic_variants_validate() {
    for seed in 0..20 {
        let mut p = demo::problem(20 + seed as usize);
        for (i, t) in p.tasks.iter_mut().enumerate() {
            t.release = ((i as u64 * seed) % 20) as i64;
            t.modes[0].phases[0].work = Some(10.0 + (seed % 7) as f64 * 1.1);
        }
        let s = engine::create(
            &p,
            &Options {
                strategy: "random".into(),
                seed,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(validate(&p, &s).valid);
    }
}
