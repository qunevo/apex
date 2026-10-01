use apex::{
    demo,
    model::{Options, Problem},
    production::{ProductionInput, expand},
    validate, xg,
};
use serde_json::{Value, json};

#[test]
fn current_production_input_roundtrips_and_produces_a_valid_schedule() {
    let input: ProductionInput =
        serde_json::from_str(include_str!("../examples/production-orders.json")).unwrap();
    let expanded = expand(input).unwrap();
    let encoded = serde_json::to_value(expanded).unwrap();
    assert!(encoded.get("schema_version").is_none());
    assert!(encoded["planning"].get("version").is_none());
    let problem: Problem = serde_json::from_value(encoded).unwrap();
    let mut schedule = xg::create(&problem, &Options::default()).unwrap();
    assert!(validate::validate(&problem, &schedule).valid);
    schedule.assignments[0].end = schedule.assignments[0].start - 1;
    assert!(!validate::validate(&problem, &schedule).valid);
}

#[test]
fn retired_version_fields_and_unknown_fields_are_rejected() {
    let problem = serde_json::to_value(demo::problem(1)).unwrap();
    for field in ["schema_version", "unexpected_setting"] {
        let mut input = problem.clone();
        input[field] = json!("retired");
        let error = serde_json::from_value::<Problem>(input).unwrap_err();
        assert!(error.to_string().contains("unknown field"), "{error}");
    }
    let mut input = problem;
    input["planning"]["version"] = json!("retired");
    assert!(serde_json::from_value::<Problem>(input.clone()).is_err());
    let production = json!({"problem": input, "workplans": [], "demands": []});
    assert!(serde_json::from_value::<ProductionInput>(production).is_err());
    assert!(serde_json::from_value::<Options>(json!({"unexpected_setting": true})).is_err());
}

#[test]
fn shipped_schemas_match_the_current_rust_input_contracts() {
    let contracts = [
        (
            include_str!("../schemas/scheduling-problem.schema.json"),
            schemars::schema_for!(Problem),
        ),
        (
            include_str!("../schemas/production-orders.schema.json"),
            schemars::schema_for!(ProductionInput),
        ),
        (
            include_str!("../schemas/planning-options.schema.json"),
            schemars::schema_for!(Options),
        ),
    ];
    for (snapshot, current) in contracts {
        let saved: Value = serde_json::from_str(snapshot).unwrap();
        assert_eq!(saved, serde_json::to_value(current).unwrap());
    }
}
