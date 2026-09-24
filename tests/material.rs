use apex::{
    engine,
    material::{self, DispatchOptions},
    model::*,
    validate,
};
use serde_json::json;

fn problem(mut tasks: serde_json::Value) -> Problem {
    for t in tasks.as_array_mut().unwrap() {
        t["modes"] = json!([]);
    }
    let mut p: Problem = serde_json::from_value(json!({
        "id":"synthetic-material-network", "horizon":10000,
        "resources":[{"id":"M","calendar":[{"start":0,"end":10000}]}],
        "tasks":tasks
    }))
    .unwrap();
    for t in &mut p.tasks {
        t.modes = serde_json::from_value(json!([{"id":"standard","primary":"M","phases":[{"id":"work","work":10,"requirements":[{"resource":"M","retain":true}]}]}])).unwrap();
    }
    p
}

fn plan(p: &Problem) -> Schedule {
    let s = engine::create(p, &Options::default()).unwrap();
    assert!(validate::validate(p, &s).diagnostics.is_empty());
    s
}

#[test]
fn multilevel_bom_pegs_partial_stock_receipts_and_existing_production() {
    let mut p = problem(json!([
        {"id":"assemble","due":50,"consume":{"part":6},"produce":{"finished":2}},
        {"id":"make-part","consume":{"blank":4},"produce":{"part":4}},
        {"id":"cut-blank","consume":{"raw":4},"produce":{"blank":4}}
    ]));
    p.inventory.insert("raw".into(), 4.0);
    p.inventory.insert("part".into(), 1.0);
    p.receipts.push(Receipt {
        item: "part".into(),
        at: 15,
        amount: 1.0,
    });
    let (q, r) = material::prepare(&p, &DispatchOptions::default()).unwrap();
    assert_eq!(r.added_dependencies, 2);
    let a: Vec<_> = r
        .allocations
        .iter()
        .filter(|a| a.consumer == "assemble")
        .collect();
    assert_eq!(a.iter().map(|a| a.quantity).sum::<f64>(), 6.0);
    assert_eq!(
        a.iter().map(|a| a.source.as_str()).collect::<Vec<_>>(),
        vec!["stock", "receipt", "production"]
    );
    let s = plan(&q);
    let start = |id: &str| s.assignments.iter().find(|a| a.task == id).unwrap().start;
    assert!(start("cut-blank") < start("make-part"));
    assert!(start("make-part") < start("assemble"));
    // A forged result must still satisfy material events and the added supply edge.
    let mut corrupt = s.clone();
    corrupt.assignments[0].start = 0;
    corrupt.assignments[0].activities[0].start = 0;
    assert!(!validate::validate(&q, &corrupt).diagnostics.is_empty());
    assert_eq!(p.dependencies.len(), 0);
    assert_eq!(p.tasks.len(), q.tasks.len());
}

#[test]
fn production_precedes_late_receipts_and_receipts_still_delay_processing() {
    let mut p = problem(
        json!([{"id":"consume","due":20,"consume":{"part":2}}, {"id":"produce","produce":{"part":1}}]),
    );
    p.receipts.push(Receipt {
        item: "part".into(),
        at: 100,
        amount: 2.0,
    });
    let (q, r) = material::prepare(&p, &DispatchOptions::default()).unwrap();
    assert_eq!(
        r.allocations
            .iter()
            .map(|a| (a.source.as_str(), a.quantity))
            .collect::<Vec<_>>(),
        vec![("production", 1.0), ("receipt", 1.0)]
    );
    let s = plan(&q);
    assert!(
        s.assignments
            .iter()
            .find(|a| a.task == "consume")
            .unwrap()
            .start
            >= 100
    );
}

#[test]
fn pegged_stock_cannot_be_stolen_by_a_different_dispatch_policy() {
    let mut p = problem(
        json!([{"id":"early","due":20,"consume":{"part":1}},{"id":"later","due":200,"consume":{"part":1}}]),
    );
    p.inventory.insert("part".into(), 1.0);
    p.receipts.push(Receipt {
        item: "part".into(),
        at: 100,
        amount: 1.0,
    });
    let (q, r) = material::prepare(&p, &DispatchOptions::default()).unwrap();
    assert_eq!(r.allocations[0].consumer, "early");
    assert_eq!(r.allocations[0].source, "stock");
    let options = Options {
        decision_prefix: vec![Decision {
            task: "later".into(),
            mode: "standard".into(),
        }],
        ..Default::default()
    };
    let s = engine::create(&q, &options).unwrap();
    assert!(
        s.assignments
            .iter()
            .find(|a| a.task == "later")
            .unwrap()
            .start
            >= 100
    );
    assert!(validate::validate(&q, &s).diagnostics.is_empty());
    let unpegged = engine::create(&p, &options).unwrap();
    let checked = validate::validate(&q, &unpegged);
    assert!(
        checked
            .diagnostics
            .iter()
            .any(|d| d.code == "MATERIAL_BALANCE")
    );
}

#[test]
fn started_inputs_are_not_consumed_twice_and_output_waits_for_completion() {
    let mut p = problem(json!([
        {"id":"started","consume":{"already-used":1},"produce":{"part":1}},
        {"id":"next","consume":{"part":1}}
    ]));
    p.tasks[0].execution = Some(serde_json::from_value(json!({
        "mode":"standard","as_of":4,
        "actual":{"id":"actual","task":"started","role":"actual","mode":"standard","start":0,"end":4,"segments":[{"phase":"work","start":0,"end":4,"work":4}],"reservations":[{"resource":"M","start":0,"end":4,"amount":1}]},
        "remaining_work":{"work":6}
    })).unwrap());
    let (q, r) = material::prepare(&p, &DispatchOptions::default()).unwrap();
    assert_eq!(r.started_inputs_already_consumed, vec!["started"]);
    assert_eq!(r.allocations.len(), 1);
    let s = plan(&q);
    assert_eq!(s.assignments[0].ready, 10);
    assert!(s.assignments[1].start >= 10);
}

#[test]
fn shortages_cycles_and_double_preparation_are_explicit() {
    let p = problem(
        json!([{"id":"A","consume":{"B":1},"produce":{"A":1}},{"id":"B","consume":{"A":1},"produce":{"B":1}}]),
    );
    let errors = material::prepare(&p, &DispatchOptions::default())
        .err()
        .unwrap();
    assert_eq!(errors.len(), 2);
    assert!(errors.iter().all(|d| d.code == "MATERIAL_UNRESOLVED"));
    let mut p = problem(json!([{"id":"A","consume":{"raw":2}}]));
    p.inventory.insert("raw".into(), 1.0);
    assert!(material::prepare(&p, &DispatchOptions::default()).is_err());
    p.inventory.insert("raw".into(), 2.0);
    let (q, _) = material::prepare(&p, &DispatchOptions::default()).unwrap();
    assert_eq!(
        material::prepare(&q, &DispatchOptions::default())
            .err()
            .unwrap()[0]
            .code,
        "MATERIAL_ALREADY_PREPARED"
    );
}

#[test]
fn routes_remain_flexible_and_material_modes_remain_consistent() {
    let mut p = problem(json!([{"id":"A","consume":{"raw":1}},{"id":"B","consume":{"raw":2}}]));
    p.inventory.insert("raw".into(), 2.0);
    p.routes = serde_json::from_value(
        json!([{"id":"route","alternatives":[{"id":"a","tasks":["A"]},{"id":"b","tasks":["B"]}]}]),
    )
    .unwrap();
    let (flexible, preview) = material::prepare(&p, &DispatchOptions::default()).unwrap();
    assert!(preview.preview);
    assert_eq!(flexible.routes.len(), 1);
    assert!(flexible.material_policy.is_some());
    for choice in ["a", "b"] {
        let o = Options {
            route_choices: [("route".into(), choice.into())].into(),
            ..Default::default()
        };
        let s = engine::create(&flexible, &o).unwrap();
        assert!(apex::validate::validate(&flexible, &s).valid);
        assert_eq!(
            s.material_report.as_ref().unwrap().route_choices["route"],
            choice
        );
    }
    let options = DispatchOptions {
        route_choices: [("route".into(), "b".into())].into(),
        ..Default::default()
    };
    let (q, r) = material::prepare(&p, &options).unwrap();
    assert_eq!(q.tasks.len(), 1);
    assert_eq!(r.route_choices["route"], "b");
    assert_eq!(r.allocations[0].quantity, 2.0);
    plan(&q);
    p.routes.clear();
    p.tasks.truncate(1);
    let mut alt = p.tasks[0].modes[0].clone();
    alt.id = "other".into();
    p.tasks[0].modes.push(alt);
    assert_eq!(
        material::prepare(&p, &DispatchOptions::default())
            .unwrap()
            .0
            .tasks[0]
            .modes
            .len(),
        2
    );
    p.tasks[0].modes[1].consume = Some([("raw".into(), 2.0)].into());
    let (flexible, report) = material::prepare(&p, &DispatchOptions::default()).unwrap();
    assert!(report.preview);
    assert!(flexible.material_policy.is_some());
    assert_eq!(flexible.tasks[0].modes.len(), 2);
    plan(&flexible);
    let options = DispatchOptions {
        mode_choices: [("A".into(), "other".into())].into(),
        ..Default::default()
    };
    let (q, _) = material::prepare(&p, &options).unwrap();
    assert_eq!(q.tasks[0].modes.len(), 1);
    assert_eq!(q.tasks[0].consume["raw"], 2.0);
    plan(&q);
}

#[test]
fn explicit_predecessors_do_not_override_material_requirements() {
    let mut p = problem(json!([{"id":"A"},{"id":"B","consume":{"missing":1}}]));
    p.dependencies.push(Dependency {
        before: "A".into(),
        after: "B".into(),
        min_lag: 0,
        max_lag: None,
    });
    assert_eq!(
        material::prepare(&p, &DispatchOptions::default())
            .err()
            .unwrap()[0]
            .code,
        "MATERIAL_UNRESOLVED"
    );
}
