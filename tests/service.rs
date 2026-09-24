use apex::{demo, service::Service, transport};
use serde_json::{Value, json};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};
fn service() -> Service {
    let root = std::env::temp_dir().join(format!(
        "apex-test-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    Service::new(&root, std::env::current_dir().unwrap()).unwrap()
}
fn call(s: &Service, n: &str, a: Value) -> Value {
    s.call(n, &a).unwrap_or_else(|e| panic!("{n}: {e}"))
}
#[test]
fn material_routes_urgency_conditionals_and_kpis_are_available_to_agents() {
    let s = service();
    let p: Value = serde_json::from_str(include_str!("../examples/chain-routing.json")).unwrap();
    let source = call(&s, "problem.import", json!({"problem":p}));
    let prepared = call(
        &s,
        "material.prepare",
        json!({"scenario_id":source["scenario_id"],"expected_revision":1}),
    );
    assert_eq!(prepared["allocation_preview"], true);
    let id = &prepared["scenario_id"];
    let plan = call(
        &s,
        "schedule.improve",
        json!({"scenario_id":id,"options":{"iterations":24,"budget_ms":0,"trainer":{"population_size":4}}}),
    );
    let args =
        json!({"scenario_id":id,"schedule_id":plan["schedule_id"],"section":"materials","limit":1});
    let rows = call(&s, "model.page", args);
    assert_eq!(rows["items"].as_array().unwrap().len(), 1);
    assert!(rows["total"].as_u64().unwrap() >= 3);
    let inspected = call(
        &s,
        "task.inspect",
        json!({"schedule_id":plan["schedule_id"],"task":"PRODUCE-FAST"}),
    );
    assert_eq!(inspected["dispatch_urgency"]["due"], 75);
    assert_eq!(inspected["dispatch_urgency"]["priority"], 9.0);
    assert_eq!(inspected["task"]["due"], 800);
    assert_eq!(
        inspected["conditional_alternatives"]["pre:setup"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let metrics = call(
        &s,
        "model.page",
        json!({"schedule_id":plan["schedule_id"],"section":"metrics","limit":200}),
    );
    assert!(
        metrics["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == "order_on_time_delivery"
                && v["value"].is_number()
                && v["unit"] == "fraction")
    );
    call(
        &s,
        "scenario.patch",
        json!({"scenario_id":id,"expected_revision":1,"patches":[{"kind":"conditional_modes","task":"PRODUCE-FAST","choices":{"pre:setup":"independent-technician"}},{"kind":"due","task":"PRODUCE-FAST","value":700}]}),
    );
    let pinned = call(&s, "schedule.create", json!({"scenario_id":id}));
    assert_eq!(pinned["route_choices"]["component-workplan"], "fast");
    let inspected = call(
        &s,
        "task.inspect",
        json!({"schedule_id":pinned["schedule_id"],"task":"PRODUCE-FAST"}),
    );
    assert_eq!(
        inspected["assignment"]["activities"][0]["mode"],
        "independent-technician"
    );
    let old = call(
        &s,
        "model.page",
        json!({"scenario_id":id,"schedule_id":plan["schedule_id"],"section":"routes"}),
    );
    assert_eq!(old["schedule_revision"], 1);
    assert_eq!(
        call(
            &s,
            "schedule.validate",
            json!({"schedule_id":plan["schedule_id"]})
        )["valid"],
        true
    );
    let task_schema = call(&s, "schema.get", json!({"definition":"Task"}));
    assert!(task_schema["properties"]["conditional_modes"].is_object());
    std::fs::remove_dir_all(s.root).unwrap();
}
#[test]
fn improve_tool_retains_current_incumbent_and_rejects_stale_or_foreign_results() {
    let s = service();
    let source = call(&s, "demo.create", json!({"tasks":8}));
    let id = &source["scenario_id"];
    let base = call(&s, "schedule.create", json!({"scenario_id":id}));
    let args = json!({"scenario_id":id,"schedule_id":base["schedule_id"],"options":{"iterations":16,"budget_ms":0,"improve":{"evolution_share":0.3},"trainer":{"population_size":4}}});
    let improved = call(&s, "schedule.improve", args.clone());
    assert_eq!(improved["search"]["algorithm"], "trainer_plus_ga");
    assert_eq!(improved["evaluations"], 16);
    let evolved = call(&s, "schedule.evolve", args.clone());
    assert_eq!(evolved["search"]["algorithm"], "direct_schedule_ga");
    assert_eq!(evolved["evaluations"], 16);
    assert_eq!(
        call(
            &s,
            "schedule.validate",
            json!({"schedule_id":evolved["schedule_id"]})
        )["valid"],
        true
    );
    assert_eq!(
        call(
            &s,
            "schedule.validate",
            json!({"schedule_id":improved["schedule_id"]})
        )["valid"],
        true
    );
    let fork = call(&s, "scenario.fork", json!({"scenario_id":id}));
    let mut foreign = args.clone();
    foreign["scenario_id"] = fork["scenario_id"].clone();
    assert!(s.call("schedule.improve", &foreign).is_err());
    assert!(s.call("schedule.evolve", &foreign).is_err());
    call(
        &s,
        "scenario.patch",
        json!({"scenario_id":id,"expected_revision":1,"patches":[{"kind":"priority","task":"T000000","value":2}]}),
    );
    assert!(s.call("schedule.improve", &args).is_err());
    assert!(s.call("schedule.evolve", &args).is_err());
    std::fs::remove_dir_all(s.root).unwrap();
}
#[test]
fn material_preparation_is_revision_bound_and_report_is_paged() {
    let s = service();
    let mut p = demo::problem(2);
    p.tasks[0].consume.insert("part".into(), 2.0);
    p.tasks[1].produce.insert("part".into(), 2.0);
    p.tasks[1].consume.clear();
    p.dependencies.clear();
    let source = call(&s, "problem.import", json!({"problem":p}));
    assert!(
        s.call(
            "material.prepare",
            &json!({"scenario_id":source["scenario_id"],"expected_revision":2})
        )
        .is_err()
    );
    let prepared = call(
        &s,
        "material.prepare",
        json!({"scenario_id":source["scenario_id"],"expected_revision":1}),
    );
    assert_ne!(prepared["scenario_id"], source["scenario_id"]);
    assert_eq!(prepared["added_dependencies"], 1);
    let allocations = call(
        &s,
        "artifact.read",
        json!({"artifact_id":prepared["report_id"],"pointer":"/allocations","limit":1}),
    );
    assert_eq!(allocations["items"][0]["source_task"], "T000001");
    assert_eq!(allocations["items"][0]["consumer"], "T000000");
    let plan = call(
        &s,
        "schedule.create",
        json!({"scenario_id":prepared["scenario_id"]}),
    );
    assert_eq!(
        call(
            &s,
            "schedule.validate",
            json!({"schedule_id":plan["schedule_id"]})
        )["valid"],
        true
    );
    assert_eq!(
        call(
            &s,
            "model.page",
            json!({"scenario_id":source["scenario_id"],"section":"dependencies"})
        )["total"],
        0
    );
}

#[test]
fn production_baseline_freeze_keeps_independent_post_processing_feasible() {
    let s = service();
    let input = call(&s, "demo.create", json!({"profile":"production"}));
    let base = call(
        &s,
        "schedule.create",
        json!({"scenario_id":input["scenario_id"]}),
    );
    let fork = call(
        &s,
        "scenario.fork",
        json!({"scenario_id":input["scenario_id"]}),
    );
    call(
        &s,
        "scenario.freeze",
        json!({"scenario_id":fork["scenario_id"],"expected_revision":1,"schedule_id":base["schedule_id"],"until":1000,"dimensions":["start","resource","order"]}),
    );
    let plan = call(
        &s,
        "schedule.create",
        json!({"scenario_id":fork["scenario_id"]}),
    );
    assert_eq!(
        call(
            &s,
            "schedule.validate",
            json!({"schedule_id":plan["schedule_id"]})
        )["valid"],
        true
    );
}
#[test]
fn durable_chunks_retry_conflict_and_cross_chunk_validation() {
    let s = service();
    let mut p = demo::problem(2);
    let rows = std::mem::take(&mut p.tasks);
    p.dependencies.push(apex::model::Dependency {
        before: rows[0].id.clone(),
        after: rows[1].id.clone(),
        min_lag: 0,
        max_lag: None,
    });
    let begin = call(&s, "import.begin", json!({"problem":p,"expected_tasks":2}));
    let key = &begin["import_id"];
    let args = json!({"import_id":key,"chunk_id":"first","tasks":[rows[0]]});
    assert_eq!(call(&s, "import.append", args.clone())["replayed"], false);
    assert_eq!(call(&s, "import.append", args)["replayed"], true);
    assert!(
        s.call(
            "import.append",
            &json!({"import_id":key,"chunk_id":"first","tasks":[rows[1]]})
        )
        .is_err()
    );
    assert!(
        s.call("import.finalize", &json!({"import_id":key}))
            .is_err()
    );
    let restarted = Service::new(&s.root, &s.workspace).unwrap();
    call(
        &restarted,
        "import.append",
        json!({"import_id":key,"chunk_id":"second","tasks":[rows[1]]}),
    );
    let result = call(&restarted, "import.finalize", json!({"import_id":key}));
    assert!(result["scenario_id"].is_string());
    assert_eq!(
        call(&restarted, "import.finalize", json!({"import_id":key}))["replayed"],
        true
    );
}
#[test]
fn errors_retain_source_and_are_pageable() {
    let s = service();
    let mut p = demo::problem(1);
    p.tasks[0].modes[0].phases[0].work = None;
    let rows = std::mem::take(&mut p.tasks);
    let begin = call(&s, "import.begin", json!({"problem":p,"expected_tasks":1}));
    call(
        &s,
        "import.append",
        json!({"import_id":begin["import_id"],"chunk_id":"one","tasks":rows}),
    );
    let result = s
        .call("import.finalize", &json!({"import_id":begin["import_id"]}))
        .unwrap_err();
    let page = call(
        &s,
        "diagnostics.page",
        json!({"diagnostic_id":result["diagnostic_id"]}),
    );
    assert_eq!(page["items"][0]["code"], "MISSING_PROCESSING_TIME");
    assert!(page["items"][0]["source"].is_string());
}
#[test]
fn revision_conflict_atomic_patch_and_independent_fork() {
    let s = service();
    let a = call(&s, "demo.create", json!({"tasks":4}));
    let fork = call(&s, "scenario.fork", json!({"scenario_id":a["scenario_id"]}));
    let patch = json!({"scenario_id":fork["scenario_id"],"expected_revision":1,"patches":[{"kind":"priority","task":"T000000","value":100}]});
    call(&s, "scenario.patch", patch.clone());
    assert!(s.call("scenario.patch", &patch).is_err());
    assert_eq!(
        call(&s, "scenario.get", json!({"scenario_id":a["scenario_id"]}))["revision"],
        1
    );
    let invalid = json!({"scenario_id":fork["scenario_id"],"expected_revision":2,"patches":[{"kind":"priority","task":"T000000","value":200},{"kind":"priority","task":"missing","value":1}]});
    assert!(s.call("scenario.patch", &invalid).is_err());
    let rows = call(&s, "tasks.page", json!({"scenario_id":fork["scenario_id"]}));
    assert_eq!(rows["items"][0]["priority"].as_f64(), Some(100.0));
}
#[test]
fn c13_freeze_from_pinned_baseline() {
    let s = service();
    let scenario = call(&s, "demo.create", json!({"tasks":8}));
    let planned = call(
        &s,
        "schedule.create",
        json!({"scenario_id":scenario["scenario_id"]}),
    );
    let frozen = call(
        &s,
        "scenario.freeze",
        json!({"scenario_id":scenario["scenario_id"],"expected_revision":1,"schedule_id":planned["schedule_id"],"until":1000,"dimensions":["resource","order"]}),
    );
    assert_eq!(frozen["revision"], 2);
    let repaired = call(
        &s,
        "schedule.repair",
        json!({"scenario_id":scenario["scenario_id"]}),
    );
    assert_eq!(repaired["valid"], true);
    assert_eq!(
        call(
            &s,
            "schedule.validate",
            json!({"schedule_id":planned["schedule_id"]})
        )["valid"],
        true
    );
}
#[test]
fn pagination_and_path_traversal() {
    let s = service();
    let scenario = call(&s, "demo.create", json!({"tasks":205}));
    let page = call(
        &s,
        "tasks.page",
        json!({"scenario_id":scenario["scenario_id"],"limit":10000}),
    );
    assert_eq!(page["items"].as_array().unwrap().len(), 200);
    assert_eq!(page["next_offset"], 200);
    assert!(
        s.call("scenario.get", &json!({"scenario_id":"../secret"}))
            .is_err()
    );
    assert!(
        s.call("problem.import", &json!({"path":"../../secret.json"}))
            .is_err()
    );
}
#[test]
fn mcp_handshake_and_tools() {
    let s = service();
    let init=transport::rpc(&s,&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}})).unwrap();
    assert_eq!(init["result"]["serverInfo"]["name"], "apex-scheduler");
    assert!(
        transport::rpc(
            &s,
            &json!({"jsonrpc":"2.0","method":"notifications/initialized"})
        )
        .is_none()
    );
    let tools = transport::rpc(&s, &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), 32);
    let result=transport::rpc(&s,&json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"solver.solve","arguments":{}}})).unwrap();
    assert_eq!(result["result"]["isError"], true);
}
#[test]
fn scenario_artifacts_survive_restart() {
    let s = service();
    let result = call(&s, "demo.create", json!({"tasks":2}));
    assert!(fs::read_dir(&s.root).unwrap().count() >= 2);
    let restarted = Service::new(&s.root, &s.workspace).unwrap();
    assert_eq!(
        call(
            &restarted,
            "scenario.get",
            json!({"scenario_id":result["scenario_id"]})
        )["tasks"],
        2
    );
}

#[test]
fn oversized_tool_output_is_retained_instead_of_flooding_context() {
    let s = service();
    let (response, failed) = s.agent_response(Ok(json!({"items":["x".repeat(100000)]})));
    assert!(!failed);
    assert!(response.to_string().len() < 2000);
    assert!(response["artifact_path"].is_string());
    assert!(
        fs::read_to_string(response["artifact_path"].as_str().unwrap())
            .unwrap()
            .len()
            > 100000
    );
}

#[test]
fn resource_freeze_override_and_remote_artifact_reading() {
    let s = service();
    let imported = call(&s, "demo.create", json!({"tasks":10}));
    let planned = call(
        &s,
        "schedule.create",
        json!({"scenario_id":imported["scenario_id"]}),
    );
    let rows = call(
        &s,
        "schedule.page",
        json!({"schedule_id":planned["schedule_id"],"limit":100}),
    );
    let expected = rows["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|a| a["primary"] != "M0")
        .count();
    let frozen = call(
        &s,
        "scenario.freeze",
        json!({"scenario_id":imported["scenario_id"],"expected_revision":1,"schedule_id":planned["schedule_id"],"until":86400,"until_by_resource":{"M0":0},"dimensions":["resource"]}),
    );
    assert_eq!(frozen["frozen_tasks"].as_u64().unwrap() as usize, expected);
    let (retained, failed) =
        s.agent_response(Ok(json!({"large":"x".repeat(70000),"rows":[1,2,3]})));
    assert!(!failed);
    let read = call(
        &s,
        "artifact.read",
        json!({"artifact_id":retained["artifact_id"],"pointer":"/rows","limit":2}),
    );
    assert_eq!(read["items"], json!([1, 2]));
    assert_eq!(read["next_offset"], 2);
}

#[test]
fn declarative_policy_tools_patch_inspect_and_explain_pinned_results() {
    let s = service();
    let mut p = demo::problem(3);
    for (i, t) in p.tasks.iter_mut().enumerate() {
        t.family = if i == 0 { "B" } else { "A" }.into();
        t.modes = vec![demo::mode("M0", 10.0)];
    }
    let source = call(&s, "problem.import", json!({"problem":p}));
    let id = &source["scenario_id"];
    call(
        &s,
        "scenario.patch",
        json!({"scenario_id":id,"expected_revision":1,"patches":[{"kind":"planning","planning":{"policies":[{"kind":"campaign","id":"campaign","resource":"M0","basis":"work","minimum":20,"initial":{"family":"A","credit":0},"on_no_match":"allow_switch"}],"constraints":[{"kind":"sequence","id":"fixed-order","resource":"M0","tasks":["T000001","T000002"],"consecutive":true}],"objectives":[{"id":"service","attribute":"urgency","weight":1}]}}]}),
    );
    let inspected = call(&s, "policy.inspect", json!({"scenario_id":id}));
    assert_eq!(inspected["planning"]["policies"][0]["id"], "campaign");
    let queues = call(&s, "queues.inspect", json!({"scenario_id":id}));
    assert!(
        queues["definitions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|q| q["id"] == "attribute:service")
    );
    let result = call(&s, "schedule.create", json!({"scenario_id":id}));
    let result_id = &result["schedule_id"];
    let explanation = call(
        &s,
        "schedule.explain_decision",
        json!({"schedule_id":result_id,"position":0}),
    );
    assert_eq!(explanation["selected"]["task"], "T000001");
    assert_eq!(explanation["step"]["reasons"][0]["rule"], "campaign");
    let row = call(
        &s,
        "task.inspect",
        json!({"schedule_id":result_id,"task":"T000001"}),
    );
    assert_eq!(row["locks"][0]["consecutive"], true);
    call(
        &s,
        "scenario.patch",
        json!({"scenario_id":id,"expected_revision":2,"patches":[{"kind":"planning","planning":{}}]}),
    );
    assert_eq!(
        call(
            &s,
            "schedule.explain_decision",
            json!({"schedule_id":result_id,"position":0})
        ),
        explanation
    );
    assert_eq!(
        call(&s, "schedule.validate", json!({"schedule_id":result_id}))["valid"],
        true
    );
}
