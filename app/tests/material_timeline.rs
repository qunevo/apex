//! Material availability must hold in time, independently of construction order.
use apex::{model::*, validate, xg};
use serde_json::json;

fn problem() -> Problem {
    serde_json::from_value(json!({
        "id":"material-timeline", "horizon":20,
        "inventory":{"raw":2},
        "resources":[
            {"id":"late","calendar":[{"start":10,"end":20}]},
            {"id":"early","calendar":[{"start":0,"end":10}]}
        ],
        "tasks":([("A","late"),("B","early")].map(|(id,machine)| json!({
            "id":id,"consume":{"raw":1},"modes":[{
                "id":"work","primary":machine,"phases":[{
                    "id":"work","work":10,"requirements":[{"resource":machine,"retain":true}]
                }]
            }]
        })))
    }))
    .unwrap()
}

fn options() -> Options {
    Options {
        decision_order: vec!["A".into(), "B".into()],
        ..Default::default()
    }
}

#[test]
fn later_consumption_does_not_prevent_using_earlier_available_stock() {
    let p = problem();
    let s = xg::create(&p, &options()).unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert_eq!(s.assignments[0].start, 10);
    assert_eq!(s.assignments[1].start, 0);
}

#[test]
fn future_receipt_can_supply_later_work_while_opening_stock_supplies_earlier_work() {
    let mut p = problem();
    p.inventory.insert("raw".into(), 1.0);
    p.receipts = vec![Receipt {
        item: "raw".into(),
        at: 10,
        amount: 1.0,
    }];
    let s = xg::create(&p, &options()).unwrap();
    assert!(validate::validate(&p, &s).valid);
    let mut unavailable = p.clone();
    unavailable.inventory.insert("raw".into(), 0.0);
    assert!(!validate::validate(&unavailable, &s).valid);
    assert!(xg::create(&unavailable, &options()).is_err());
}

#[test]
fn a_late_nonconsumer_cannot_make_a_future_receipt_available_in_the_past() {
    let mut p = problem();
    p.tasks[0].consume.clear();
    p.inventory.clear();
    p.receipts = vec![Receipt {
        item: "raw".into(),
        at: 10,
        amount: 1.0,
    }];
    assert!(xg::create(&p, &options()).is_err());
}
