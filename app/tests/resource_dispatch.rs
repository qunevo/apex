//! Generic multi-resource construction regressions, independent of any adapter.
use apex::{model::*, validate, xg};
use serde_json::json;

fn problem() -> Problem {
    serde_json::from_value(json!({
        "id":"shared-staff", "horizon":10,
        "resources":(["M1","M2","P1","P2"].map(|id| json!({
            "id":id,"calendar":[{"start":0,"end":10}]
        }))),
        "tasks":([("A","M1"),("B","M2")].map(|(id,machine)| json!({
            "id":id,"modes":(["P1","P2"].map(|person| json!({
                "id":person,"primary":machine,"phases":[{
                    "id":"work","work":10,"requirements":[
                        {"resource":machine,"retain":true},{"resource":person}
                    ]
                }]
            })))
        })))
    }))
    .unwrap()
}

#[test]
fn construction_accounts_for_shared_staff_before_selecting_modes() {
    let p = problem();
    for strategy in [
        "queues",
        "due",
        "shortest",
        "priority",
        "release",
        "objective",
    ] {
        let options = Options {
            strategy: strategy.into(),
            ..Default::default()
        };
        let schedule =
            xg::create(&p, &options).unwrap_or_else(|error| panic!("{strategy}: {error:?}"));
        assert!(validate::validate(&p, &schedule).valid);
        assert_ne!(schedule.assignments[0].mode, schedule.assignments[1].mode);
        assert!(
            schedule
                .assignments
                .iter()
                .all(|a| a.start == 0 && a.end == 10)
        );
        let mut corrupt = schedule.clone();
        for activity in &mut corrupt.assignments[1].activities {
            for reservation in &mut activity.reservations {
                if reservation.resource == "P2" {
                    reservation.resource = "P1".into();
                }
            }
        }
        assert!(!validate::validate(&p, &corrupt).valid);
    }
}

#[test]
fn explicit_staff_commitments_remain_hard_constraints() {
    let mut p = problem();
    p.locks = ["A", "B"]
        .map(|task| Lock::Mode {
            task: task.into(),
            mode: "P1".into(),
        })
        .to_vec();
    assert_eq!(
        xg::create(
            &p,
            &Options {
                strategy: "queues".into(),
                ..Default::default()
            }
        )
        .unwrap_err()[0]
            .code,
        "NO_SLOT"
    );
}

#[test]
fn a_faster_mode_without_a_real_calendar_slot_does_not_hide_a_feasible_mode() {
    let mut p = apex::demo::problem(1);
    p.horizon = 20;
    p.resources.truncate(2);
    p.resources[0].calendar[0].end = 5;
    p.resources[1].calendar[0].end = 20;
    p.tasks[0].modes = vec![apex::demo::mode("M0", 10.0), apex::demo::mode("M1", 20.0)];
    for mode in &mut p.tasks[0].modes {
        mode.phases[0].interruption = Interrupt::NonInterruptible;
    }
    for strategy in ["queues", "due"] {
        let options = Options {
            strategy: strategy.into(),
            ..Default::default()
        };
        let schedule = xg::create(&p, &options).unwrap();
        assert_eq!(schedule.assignments[0].primary, "M1");
        assert!(validate::validate(&p, &schedule).valid);
        let pinned = Options {
            mode_choices: [(p.tasks[0].id.clone(), "on-M0".into())].into(),
            ..options
        };
        assert!(xg::create(&p, &pinned).is_err());
    }
}

#[test]
fn setup_only_staff_are_released_before_the_machine_finishes() {
    let mut p = problem();
    p.locks.push(Lock::Start {
        task: "A".into(),
        at: 0,
    });
    p.resources[3].calendar[0].start = 3;
    p.tasks[0].due = Some(0);
    p.tasks[1].due = Some(1);
    p.tasks[0].modes.truncate(1);
    let mode = &mut p.tasks[0].modes[0];
    mode.phases[0].work = Some(2.0);
    let mut run = mode.phases[0].clone();
    run.id = "run".into();
    run.work = Some(8.0);
    run.requirements.truncate(1);
    mode.phases.push(run);
    for mode in &mut p.tasks[1].modes {
        mode.phases[0].work = Some(1.0);
    }
    let s = xg::create(
        &p,
        &Options {
            strategy: "queues".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert_eq!(s.assignments[1].mode, "P1");
    assert_eq!(s.assignments[1].start, 2);
}

#[test]
fn later_phase_demand_does_not_delay_the_whole_operation() {
    let mut p = problem();
    p.horizon = 11;
    for resource in &mut p.resources {
        resource.calendar[0].end = 11;
    }
    p.tasks[0].due = Some(0);
    p.tasks[1].due = Some(1);
    p.tasks[0].modes.truncate(1);
    for mode in &mut p.tasks[1].modes {
        mode.phases[0].work = Some(1.0);
        let mut first = mode.phases[0].clone();
        first.id = "automatic".into();
        first.work = Some(10.0);
        first.requirements.truncate(1);
        mode.phases.insert(0, first);
    }
    let s = xg::create(
        &p,
        &Options {
            strategy: "queues".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert_eq!(s.assignments[1].mode, "P1");
    assert_eq!(s.assignments[1].start, 0);
}
