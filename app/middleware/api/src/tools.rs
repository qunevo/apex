//! The operation catalog. HTTP routes and MCP tools both dispatch through
//! [`call`], so every interface has the same semantics, checks and events.
use crate::events::Events;
use apex_control::{
    Actor, Control, Error, ResultId, RunId, ScenarioId,
    ops::{DecideResult, ReviseScenario, StartRun},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};

pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub input: Value,
    /// Changes state; read-only clients never need these.
    pub mutates: bool,
}

fn id_arg(name: &str, description: &str) -> Value {
    json!({"type":"object","properties":{name:{"type":"string","format":"uuid","description":description}},"required":[name],"additionalProperties":false})
}

const CONTENT: &str = "Scenario content: {facts: <engine facts model, e.g. an APEX scheduling problem>, intent: {declarations: [{id, kind, target, parameters, note}]}, source_summary?: <bounded adapter evidence>}. Declarations the engine cannot represent are rejected.";

pub fn catalog() -> Vec<Tool> {
    let scenario = || id_arg("scenario_id", "Scenario ID");
    let result = |verb: &str| json!({"type":"object","properties":{"result_id":{"type":"string","format":"uuid"},"note":{"type":"string","description":format!("Optional reason recorded with the {verb}")}},"required":["result_id"],"additionalProperties":false});
    vec![
        Tool {
            name: "views.get",
            description: "Open the standard Planning overview in an MCP App: delivery KPIs, critical orders, workload and resource charts. Optional result_id selects saved outcomes and pins its revision; otherwise revision defaults to current and outcomes are unknown. Customization views are listed in available_views. Read-only; no planning or source refresh.",
            input: json!({"type":"object","properties":{"scenario_id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":1},"result_id":{"type":"string","format":"uuid"},"view_id":{"type":"string","description":"Registered view ID from available_views; omit for the standard overview"}},"required":["scenario_id"],"additionalProperties":false}),
            mutates: false,
        },
        Tool {
            name: "engines.list",
            description: "List available engines with their facts model, run methods, supported planning declarations and extensions.",
            input: json!({"type":"object","properties":{},"additionalProperties":false}),
            mutates: false,
        },
        Tool {
            name: "scenarios.list",
            description: "List the tenant's planning scenarios with current revision and published result.",
            input: json!({"type":"object","properties":{},"additionalProperties":false}),
            mutates: false,
        },
        Tool {
            name: "scenarios.get",
            description: "Get one scenario.",
            input: scenario(),
            mutates: false,
        },
        Tool {
            name: "scenarios.create",
            description: "Create a scenario at revision 1. The engine checks the facts before anything is saved.",
            input: json!({"type":"object","properties":{"customization":{"type":"string","description":"Enabled customization package ID; defaults to the server configuration"},"name":{"type":"string"},"engine":{"type":"string","description":"Engine ID from engines.list, e.g. apex"},"content":{"type":"object","description":CONTENT},"note":{"type":"string"}},"required":["name","engine","content"],"additionalProperties":false}),
            mutates: true,
        },
        Tool {
            name: "scenarios.revise",
            description: "Append an immutable revision. Fails with CONFLICT unless expected_revision is the current revision. Existing runs and results keep their revision.",
            input: json!({"type":"object","properties":{"scenario_id":{"type":"string","format":"uuid"},"expected_revision":{"type":"integer","minimum":1},"content":{"type":"object","description":CONTENT},"note":{"type":"string"}},"required":["scenario_id","expected_revision","content"],"additionalProperties":false}),
            mutates: true,
        },
        Tool {
            name: "revisions.get",
            description: "Get a revision's full content: facts and planning intent.",
            input: json!({"type":"object","properties":{"scenario_id":{"type":"string","format":"uuid"},"number":{"type":"integer","minimum":1}},"required":["scenario_id","number"],"additionalProperties":false}),
            mutates: false,
        },
        Tool {
            name: "runs.start",
            description: "Queue a background optimization of a revision (default: current). For engine apex, options are {method: create|hypersearch|treesearch|evolve|improve, options: <apex Options>}. Poll runs.get or follow events.",
            input: json!({"type":"object","properties":{"scenario_id":{"type":"string","format":"uuid"},"revision":{"type":"integer","minimum":1},"options":{"type":"object"}},"required":["scenario_id"],"additionalProperties":false}),
            mutates: true,
        },
        Tool {
            name: "runs.list",
            description: "List a scenario's runs with state, phase, attempts, diagnostics and result ID.",
            input: scenario(),
            mutates: false,
        },
        Tool {
            name: "runs.get",
            description: "Get one run.",
            input: id_arg("run_id", "Run ID"),
            mutates: false,
        },
        Tool {
            name: "runs.cancel",
            description: "Cancel a queued run, or request cancellation of a running one.",
            input: id_arg("run_id", "Run ID"),
            mutates: true,
        },
        Tool {
            name: "results.list",
            description: "List a scenario's results: revision, status, validity and metrics, without schedules.",
            input: scenario(),
            mutates: false,
        },
        Tool {
            name: "results.get",
            description: "Get a result with provenance, validation, metrics and the engine-neutral schedule view. Set include_schedule for the engine's native schedule.",
            input: json!({"type":"object","properties":{"result_id":{"type":"string","format":"uuid"},"include_schedule":{"type":"boolean","default":false}},"required":["result_id"],"additionalProperties":false}),
            mutates: false,
        },
        Tool {
            name: "results.approve",
            description: "Approve a proposed, independently validated result. Requires the approver role.",
            input: result("approval"),
            mutates: true,
        },
        Tool {
            name: "results.reject",
            description: "Reject a proposed result.",
            input: result("rejection"),
            mutates: true,
        },
        Tool {
            name: "results.publish",
            description: "Publish an approved result as the scenario's plan. Fails with CONFLICT if the scenario has a newer revision. A previously published result becomes superseded.",
            input: result("publication"),
            mutates: true,
        },
    ]
}

fn arg<T: DeserializeOwned>(args: &Value, name: &str) -> Result<T, Error> {
    serde_json::from_value(args.get(name).cloned().unwrap_or(Value::Null))
        .map_err(|e| Error::invalid(format!("Argument {name}: {e}")))
}
fn body<T: DeserializeOwned>(args: Value, strip: &[&str]) -> Result<T, Error> {
    let mut args = args;
    if let Some(map) = args.as_object_mut() {
        for key in strip {
            map.remove(*key);
        }
    }
    serde_json::from_value(args).map_err(|e| Error::invalid(e.to_string()))
}
fn out<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("response serializes")
}

/// Run one named operation for an authenticated actor and announce changes.
pub async fn call(
    control: &Control,
    events: &Events,
    actor: &Actor,
    name: &str,
    args: Value,
) -> Result<Value, Error> {
    let args = if args.is_null() { json!({}) } else { args };
    let tenant = actor.tenant;
    Ok(match name {
        "views.get" => out(control.get_view(actor, body(args, &[])?).await?),
        "engines.list" => out(control.engines(actor)?),
        "scenarios.list" => out(control.list_scenarios(actor).await?),
        "scenarios.get" => out(control
            .get_scenario(actor, arg(&args, "scenario_id")?)
            .await?),
        "scenarios.create" => {
            let (scenario, revision) = control.create_scenario(actor, body(args, &[])?).await?;
            let id = scenario.id.to_string();
            events.publish(tenant, "scenario", id.clone(), id);
            json!({"scenario": scenario, "revision": revision.number, "content_hash": revision.content_hash})
        }
        "scenarios.revise" => {
            let id: ScenarioId = arg(&args, "scenario_id")?;
            let request: ReviseScenario = body(args, &["scenario_id"])?;
            let revision = control.revise_scenario(actor, id, request).await?;
            events.publish(tenant, "scenario", id.to_string(), id.to_string());
            json!({"scenario_id": id, "revision": revision.number, "content_hash": revision.content_hash})
        }
        "revisions.get" => out(control
            .get_revision(actor, arg(&args, "scenario_id")?, arg(&args, "number")?)
            .await?),
        "runs.start" => {
            let id: ScenarioId = arg(&args, "scenario_id")?;
            let request: StartRun = body(args, &["scenario_id"])?;
            let run = control.start_run(actor, id, request).await?;
            events.publish(tenant, "run", run.id.to_string(), id.to_string());
            out(run)
        }
        "runs.list" => out(control.list_runs(actor, arg(&args, "scenario_id")?).await?),
        "runs.get" => out(control.get_run(actor, arg(&args, "run_id")?).await?),
        "runs.cancel" => {
            let id: RunId = arg(&args, "run_id")?;
            let run = control.cancel_run(actor, id).await?;
            events.publish(tenant, "run", id.to_string(), run.scenario.to_string());
            out(run)
        }
        "results.list" => out(control
            .list_results(actor, arg(&args, "scenario_id")?)
            .await?),
        "results.get" => {
            let mut result = out(control.get_result(actor, arg(&args, "result_id")?).await?);
            if args["include_schedule"] != json!(true) {
                result.as_object_mut().unwrap().remove("schedule");
            }
            result
        }
        "results.approve" | "results.reject" | "results.publish" => {
            let id: ResultId = arg(&args, "result_id")?;
            let request: DecideResult = body(args, &["result_id"])?;
            let mut result = match name {
                "results.approve" => control.approve_result(actor, id, request).await?,
                "results.reject" => control.reject_result(actor, id, request).await?,
                _ => control.publish_result(actor, id, request).await?,
            };
            let scenario = result.scenario.to_string();
            events.publish(tenant, "result", id.to_string(), scenario.clone());
            if name == "results.publish" {
                events.publish(tenant, "scenario", scenario.clone(), scenario);
            }
            result.schedule = Value::Null;
            let mut value = out(result);
            value.as_object_mut().unwrap().remove("schedule");
            value
        }
        _ => return Err(Error::NotFound(format!("operation {name}"))),
    })
}
