use apex::{demo, model::Options, service::Service, transport, xe, xg, xh, xt};
use serde_json::{Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn canonical_component_options_preserve_legacy_saved_options() {
    let legacy = json!({
        "iterations": 12, "budget_ms": 0,
        "trainer": {"population_size": 4, "workers": 2},
        "plus": {"workers": 2, "depth": 3},
        "evolution": {"population_size": 8, "parent_selection": "uniform"},
        "improve": {"trainer_share": 0.3, "evolution_share": 0.2}
    });
    let canonical = json!({
        "iterations": 12, "budget_ms": 0,
        "xh": {"population_size": 4, "workers": 2},
        "xt": {"workers": 2, "depth": 3},
        "xe": {"population_size": 8, "parent_selection": "uniform"},
        "improve": {"xh_share": 0.3, "xe_share": 0.2}
    });
    let old: Options = serde_json::from_value(legacy).unwrap();
    let current: Options = serde_json::from_value(canonical).unwrap();
    let serialized = serde_json::to_value(&current).unwrap();
    assert_eq!(serde_json::to_value(old).unwrap(), serialized);
    for obsolete in ["trainer", "plus", "evolution"] {
        assert!(serialized.get(obsolete).is_none());
    }
    assert!(serialized["improve"].get("trainer_share").is_none());
    assert!(serialized["improve"].get("evolution_share").is_none());
    let schema = serde_json::to_value(schemars::schema_for!(Options)).unwrap();
    for name in ["xh", "xt", "xe"] {
        assert!(schema["properties"].get(name).is_some());
    }
    for obsolete in ["trainer", "plus", "evolution"] {
        assert!(schema["properties"].get(obsolete).is_none());
    }
}

#[test]
fn ambiguous_or_misspelled_component_options_are_rejected() {
    for malformed in [
        json!({"xh": {}, "trainer": {}}),
        json!({"xt": {}, "plus": {}}),
        json!({"xe": {}, "evolution": {}}),
        json!({"improve": {"xh_share": 0.4, "trainer_share": 0.5}}),
        json!({"improve": {"xe_share": 0.2, "evolution_share": 0.3}}),
        json!({"hypersearch": {}}),
        json!({"xt": {"unknown_setting": true}}),
    ] {
        assert!(serde_json::from_value::<Options>(malformed).is_err());
    }
}

#[test]
fn advertised_component_tools_run_and_independently_validate() {
    let directory = std::env::temp_dir().join(format!(
        "apex-components-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let service = Service::new(&directory, &directory).unwrap();
    let tools = transport::tools();
    let definitions = tools["tools"].as_array().unwrap();
    assert_eq!(definitions.len(), 32);
    let components = [
        (xg::ID, "schedule.create"),
        (xh::ID, "schedule.hypersearch"),
        (xt::ID, "schedule.treesearch"),
        (xe::ID, "schedule.evolve"),
    ];
    let capabilities = service.call("capabilities", &json!({})).unwrap();
    let expected: Vec<Value> = components
        .iter()
        .map(|(id, tool)| json!({"id": id, "tool": tool}))
        .collect();
    assert_eq!(capabilities["algorithms"], json!(expected));
    let imported = service
        .call("problem.import", &json!({"problem": demo::problem(8)}))
        .unwrap();
    for (id, name) in components {
        let definition = definitions.iter().find(|t| t["name"] == name).unwrap();
        assert!(definition["description"].as_str().unwrap().starts_with(id));
        let plan = service
            .call(
                name,
                &json!({
                    "scenario_id": imported["scenario_id"],
                    "options": {"iterations": 8, "budget_ms": 0, "xh": {"population_size": 4}}
                }),
            )
            .unwrap();
        if id != xg::ID {
            assert_eq!(plan["search"]["algorithm"], id);
        }
        let validation = service
            .call(
                "schedule.validate",
                &json!({"schedule_id": plan["schedule_id"]}),
            )
            .unwrap();
        assert_eq!(validation["valid"], true);
    }
    for obsolete in ["schedule.train", "schedule.plus"] {
        assert!(!definitions.iter().any(|t| t["name"] == obsolete));
        assert!(service.call(obsolete, &json!({})).is_err());
    }
}
