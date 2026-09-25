use crate::service::{Service, tool_names};
use serde_json::{Value, json};
use std::io::{self, BufRead, Read, Write};

fn options_schema() -> Value {
    let schema = serde_json::to_value(schemars::schema_for!(crate::model::Options)).unwrap();
    fn inline(value: &Value, root: &Value) -> Value {
        if let Some(reference) = value
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|s| s.strip_prefix('#'))
        {
            return inline(root.pointer(reference).unwrap(), root);
        }
        match value {
            Value::Object(map) => Value::Object(
                map.iter()
                    .filter(|(k, _)| !matches!(k.as_str(), "$schema" | "$defs"))
                    .map(|(k, v)| (k.clone(), inline(v, root)))
                    .collect(),
            ),
            Value::Array(v) => Value::Array(v.iter().map(|x| inline(x, root)).collect()),
            _ => value.clone(),
        }
    }
    inline(&schema, &schema)
}

pub fn tools() -> Value {
    let definitions=tool_names().into_iter().map(|name|{
        let (description,required,properties)=match name{
            "material.prepare"=>("Peg existing stock, receipts and production to operation material requirements. Preserves free workplans and reallocates after each route selection; the returned allocation report is a preview for flexible routes. Use model.page/materials for actual saved allocations. Never creates orders. Select differing material modes first.",vec!["scenario_id","expected_revision"],json!({"scenario_id":{"type":"string"},"expected_revision":{"type":"integer","minimum":1},"options":{"type":"object","additionalProperties":false,"properties":{"route_choices":{"type":"object","additionalProperties":{"type":"string"}},"mode_choices":{"type":"object","additionalProperties":{"type":"string"}}}}})),
            "artifact.read"=>("Read a retained response by artifact ID and JSON pointer. Objects list fields; arrays are paginated; strings are bounded. Works for remote agents without filesystem access.",vec!["artifact_id"],json!({"artifact_id":{"type":"string"},"pointer":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}})),
            "production.import"=>("Expand production orders into quantity-conserving lots, jobs, workplan choices and material flows. Supply a ProductionInput by file path or inline production object.",vec![],json!({"path":{"type":"string"},"production":{"type":"object"}})),
            "model.page"=>("Page model relationships, metric definitions/values or actual material allocations. Use schedule_id for the immutable model of a result, or scenario_id for current input.",vec!["section"],json!({"scenario_id":{"type":"string"},"schedule_id":{"type":"string"},"section":{"type":"string","enum":["routes","jobs","orders","dependencies","rules","locks","freeze_zones","policies","constraints","planning_objectives","metrics","materials"]},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}})),
            "task.inspect"=>("Inspect a task's chosen mode, conditional activities, source, materials and active dependencies from a pinned schedule.",vec!["schedule_id","task"],json!({"schedule_id":{"type":"string"},"task":{"type":"string"}})),
            "policy.inspect"=>("Inspect the bounded planning language, ordered dispatch policies, units, exceptions and extension workflow.",vec!["scenario_id"],json!({"scenario_id":{"type":"string"}})),
            "schedule.explain_decision"=>("Explain a selected task or zero-based construction position using the immutable policy witness. Detailed reasons are retained for the first 256 decisions; this is not an optimality certificate.",vec!["schedule_id"],json!({"schedule_id":{"type":"string"},"task":{"type":"string"},"position":{"type":"integer","minimum":0}})),
            "queues.inspect"=>("Inspect standard/custom Qs, stage policies, objective-to-proxy mapping and missing mappings. Qs are heuristics; full validated objectives drive selection.",vec!["scenario_id"],json!({"scenario_id":{"type":"string"}})),
            "schema.get"=>("Read executable input schema root or a named definition. Resolve nested references with another schema.get call.",vec![],json!({"definition":{"type":"string"},"model":{"type":"string","enum":["problem","production","options"]}})),
            "capabilities"=>("Read implemented capabilities and explicit limitations.",vec![],json!({})),
            "demo.create"=>("Create a deliberately synthetic planning scenario.",vec![],json!({"tasks":{"type":"integer","minimum":1,"maximum":100000},"profile":{"type":"string","enum":["simple","production"]}})),
            "problem.import"=>("Import executable apex.v3.4 (or compatible v3.1/v3.2/v3.3) JSON by workspace-relative path or inline problem. Prefer paths for large files.",vec![],json!({"path":{"type":"string"},"problem":{"type":"object"}})),
            "import.begin"=>("Start durable chunk import. Header has resources/rules/dependencies but an empty tasks array. Count is checked on finalize.",vec!["problem","expected_tasks"],json!({"problem":{"type":"object"},"expected_tasks":{"type":"integer","minimum":0,"maximum":1000000}})),
            "import.append"=>("Append at most 5000 task records/4 MiB with an idempotent chunk ID. Path points to a JSON task array inside workspace.",vec!["import_id","chunk_id"],json!({"import_id":{"type":"string"},"chunk_id":{"type":"string"},"tasks":{"type":"array","items":{"type":"object"},"maxItems":5000},"path":{"type":"string"}})),
            "import.status"|"import.finalize"=>("Read import progress or finalize all chunks with global reference and readiness checks.",vec!["import_id"],json!({"import_id":{"type":"string"}})),
            "diagnostics.page"=>("Page through retained import diagnostics, including source references.",vec!["diagnostic_id"],json!({"diagnostic_id":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}})),
            "scenario.get"|"scenario.fork"=>("Read scenario metadata or fork an independent scenario before a what-if change.",vec!["scenario_id"],json!({"scenario_id":{"type":"string"}})),
            "tasks.page"=>("Read a bounded page of canonical task inputs. Keep full datasets out of chat context.",vec!["scenario_id"],json!({"scenario_id":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}})),
            "scenario.patch"=>("Apply typed patches with optimistic revision check: due/priority(task,value), downtime(resource,start,end,forbid_retention), lock(lock object), objectives, rules, planning (constraints/policies/objectives), route_choice(route,alternative), conditional_modes(task,choices keyed by activity suffix), job(job,due/priority/deadline), customization. Fork first for scenarios. Unknown fields in planning objects are rejected.",vec!["scenario_id","expected_revision","patches"],json!({"scenario_id":{"type":"string"},"expected_revision":{"type":"integer"},"patches":{"type":"array","items":{"type":"object"},"maxItems":1000}})),
            "scenario.freeze"=>("Pin baseline-start membership before until (integer seconds), optionally for one resource. until_by_resource overrides the global boundary before selecting baseline members. Dimensions: mode, resource, start, order. Existing locks remain.",vec!["scenario_id","expected_revision","schedule_id","until","dimensions"],json!({"scenario_id":{"type":"string"},"expected_revision":{"type":"integer"},"schedule_id":{"type":"string"},"until":{"type":"integer"},"until_by_resource":{"type":"object","additionalProperties":{"type":"integer","minimum":0}},"resource":{"type":"string"},"dimensions":{"type":"array","items":{"type":"string","enum":["mode","resource","start","order"]}}})),
            "schedule.improve"=>("Improve a plan using XH, XT with distinct Q policies, and optionally XE direct schedule evolution with adaptive operators (options.improve.xe_share > 0; default 0 after quality ablations) and one shared time/evaluation budget. Optionally retain a validated schedule_id from the same scenario revision. Preserves the best valid incumbent; no optimality guarantee. Prefer this for user-facing improvement; individual search tools are for diagnostics.",vec!["scenario_id"],json!({"scenario_id":{"type":"string"},"schedule_id":{"type":"string"},"options":options_schema()})),
            // XE: direct schedule evolution with adaptive genetic operators.
            "schedule.evolve"=>("XE: refine operation priorities, modes, routes and conditional choices with a direct GA and adaptive operators. Optional schedule_id retains a validated incumbent from the same scenario revision. Extra calls spend an additional budget. No optimality guarantee. Inspect operator evidence and compare with the baseline.",vec!["scenario_id"],json!({"scenario_id":{"type":"string"},"schedule_id":{"type":"string"},"options":options_schema()})),
            // XG: one greedy construction with independent validation.
            "schedule.create"=>("XG: construct and independently validate a schedule using the requested dispatch policy. Use for quick planning; schedule.improve combines the search components under one budget. A failed construction does not prove infeasibility.",vec!["scenario_id"],json!({"scenario_id":{"type":"string"},"options":options_schema()})),
            // XH: hypersearch over dispatch policies, weights and supported choices.
            "schedule.hypersearch"|"schedule.repair"=>("XH: search dispatch policies, weights and supported model choices; independently validate each accepted schedule. Repair reconstructs the current scenario while preserving its commitments. This is instance-specific hypersearch, not model training or a correctness certificate. Time is checked between worker batches; inspect search and replay metadata.",vec!["scenario_id"],json!({"scenario_id":{"type":"string"},"options":options_schema()})),
            // XT: UCT-guided search over admissible construction prefixes.
            "schedule.treesearch"=>("XT: run UCT-guided tree search over admissible construction prefixes and complete rollouts through XG. Preserve hard rules and commitments and independently validate accepted schedules. No completeness or optimality guarantee. Time is checked between worker batches; inspect search and replay metadata.",vec!["scenario_id"],json!({"scenario_id":{"type":"string"},"options":options_schema()})),
            "schedule.validate"=>("Independently recheck stored assignments against their pinned problem revision.",vec!["schedule_id"],json!({"schedule_id":{"type":"string"}})),
            "schedule.page"=>("Read a bounded schedule page with segments, reservations, metrics and optional occupied-resource filter (including secondary resources).",vec!["schedule_id"],json!({"schedule_id":{"type":"string"},"resource":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}})),
            "scenario.compare"=>("Compare two immutable schedules: KPIs and changed task assignments.",vec!["baseline_id","candidate_id"],json!({"baseline_id":{"type":"string"},"candidate_id":{"type":"string"}})),
            _=>("Reserved solver boundary. Returns UNSUPPORTED_BACKEND until a backend is implemented.",vec![],json!({})),
        };
        let read_only=matches!(name,"queues.inspect"|"policy.inspect"|"schedule.explain_decision"|"artifact.read"|"model.page"|"task.inspect"|"capabilities"|"schema.get"|"import.status"|"diagnostics.page"|"scenario.get"|"tasks.page"|"schedule.validate"|"schedule.page"|"scenario.compare"|"solver.solve");
        json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"annotations":{"readOnlyHint":read_only,"destructiveHint":false,"openWorldHint":false}})
    }).collect::<Vec<_>>();
    json!({"tools":definitions})
}
pub fn rpc(service: &Service, request: &Value) -> Option<Value> {
    let id = request.get("id")?.clone();
    let result = match request["method"].as_str().unwrap_or("") {
        "initialize" => Ok(
            json!({"protocolVersion":"2025-11-25","capabilities":{"tools":{}},"serverInfo":{"name":"apex-scheduler","version":env!("CARGO_PKG_VERSION")},"instructions":"APEX plans discrete production. Call capabilities first. Import by artifact path or durable chunks; never send huge datasets into chat. Fork before what-if patches; use expected_revision. Hard rules and locks are always checked. A failed heuristic is not proof of infeasibility. Use schedule.create for quick plans and schedule.improve for combined XH/XT/XE search. XH tunes dispatch policies; independent validation checks correctness. Use returned viewer_url to inspect schedules. New code rules require tests and rebuild; runtime tools never execute generated code."}),
        ),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(tools()),
        "tools/call" => {
            let name = request["params"]["name"].as_str().unwrap_or("");
            let arguments = request["params"]
                .get("arguments")
                .cloned()
                .unwrap_or(json!({}));
            let (value, is_error) = service.agent_response(service.call(name, &arguments));
            Ok(
                json!({"content":[{"type":"text","text":value.to_string()}],"structuredContent":value,"isError":is_error}),
            )
        }
        _ => Err(json!({"code":-32601,"message":"Method not found"})),
    };
    Some(match result {
        Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
        Err(error) => json!({"jsonrpc":"2.0","id":id,"error":error}),
    })
}
pub fn stdio(service: &Service) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    loop {
        let mut bytes = Vec::new();
        let n = (&mut input)
            .take(8 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut bytes)?;
        if n == 0 {
            break;
        }
        if n > 8 * 1024 * 1024 {
            eprintln!("MCP input exceeds 8 MiB; use import paths/chunks");
            return Err("message too large".into());
        }
        let response = match serde_json::from_slice::<Value>(&bytes) {
            Ok(v) => rpc(service, &v),
            Err(_) => Some(
                json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON"}}),
            ),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut out, &response)?;
            writeln!(&mut out)?;
            out.flush()?;
        }
    }
    Ok(())
}
pub fn openapi(base: &str) -> Value {
    let mut paths = serde_json::Map::new();
    for tool in tools()["tools"].as_array().unwrap() {
        let name = tool["name"].as_str().unwrap();
        paths.insert(format!("/api/tools/{name}"),json!({"post":{"operationId":name.replace('.',"_"),"description":tool["description"],"requestBody":{"required":true,"content":{"application/json":{"schema":tool["inputSchema"]}}},"responses":{"200":{"description":"Tool result"},"422":{"description":"Structured planning or input diagnostic"}},"security":[{"bearerAuth":[]}]}}));
    }
    json!({"openapi":"3.1.0","info":{"title":"APEX agent scheduling tools","version":env!("CARGO_PKG_VERSION")},"servers":[{"url":base}],"paths":paths,"components":{"securitySchemes":{"bearerAuth":{"type":"http","scheme":"bearer"}}}})
}

pub fn serve(service: &Service, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let bind = std::env::var("APEX_BIND").unwrap_or_else(|_| "127.0.0.1".into());
    let token = std::env::var("APEX_API_TOKEN")
        .ok()
        .filter(|v| !v.is_empty());
    if !["127.0.0.1", "localhost", "::1"].contains(&bind.as_str())
        && token.as_ref().is_none_or(|t| t.len() < 24)
    {
        return Err("Remote binding requires APEX_API_TOKEN with at least 24 characters".into());
    }
    let server = tiny_http::Server::http((bind.as_str(), port)).map_err(|e| e.to_string())?;
    eprintln!("APEX viewer and MCP: {}", service.viewer_url);
    for mut request in server.incoming_requests() {
        let header = |name: &str| {
            request
                .headers()
                .iter()
                .find(|h| h.field.as_str().as_str().eq_ignore_ascii_case(name))
                .map(|h| h.value.as_str().to_owned())
        };
        let host = header("Host").unwrap_or_default();
        let origin = header("Origin");
        let authorization = header("Authorization");
        let protocol = header("MCP-Protocol-Version");
        let content_type = header("Content-Type").unwrap_or_default();
        let path = request.url().split('?').next().unwrap_or("").to_owned();
        let public_authority = service
            .viewer_url
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("");
        let allowed_host = host == format!("127.0.0.1:{port}")
            || host == format!("localhost:{port}")
            || host == public_authority;
        let allowed_origin = origin.as_ref().is_none_or(|o| {
            o == &format!("http://127.0.0.1:{port}")
                || o == &format!("http://localhost:{port}")
                || o == &service.viewer_url
        });
        let authorized = token.as_ref().is_none_or(|secret| {
            authorization
                .as_ref()
                .is_some_and(|a| a == &format!("Bearer {secret}"))
        });
        let protected = path.starts_with("/api/") || path == "/mcp";
        let (status, mime, body) = if !allowed_host || !allowed_origin {
            (
                403,
                "application/json",
                json!({"code":"ORIGIN","message":"Unrecognized host or origin"}).to_string(),
            )
        } else if protected && !authorized {
            (
                401,
                "application/json",
                json!({"code":"AUTHENTICATION","message":"Supply the configured bearer token"})
                    .to_string(),
            )
        } else if request.method() == &tiny_http::Method::Get && path == "/" {
            (
                200,
                "text/html; charset=utf-8",
                include_str!("../web/index.html").to_string(),
            )
        } else if request.method() == &tiny_http::Method::Get && path == "/openapi.json" {
            (
                200,
                "application/json",
                openapi(&service.viewer_url).to_string(),
            )
        } else if request.method() == &tiny_http::Method::Get && path == "/api/tools" {
            (200, "application/json", tools().to_string())
        } else if path == "/mcp" && request.method() != &tiny_http::Method::Post {
            (405,"application/json",json!({"message":"Use POST; this stateless endpoint returns JSON, not an SSE stream"}).to_string())
        } else if request.method() == &tiny_http::Method::Post
            && (path == "/mcp" || path == "/api/tool" || path.starts_with("/api/tools/"))
        {
            if !content_type
                .to_ascii_lowercase()
                .starts_with("application/json")
            {
                (
                    415,
                    "application/json",
                    json!({"message":"application/json required"}).to_string(),
                )
            } else if path == "/mcp"
                && protocol.as_ref().is_some_and(|p| {
                    !["2025-11-25", "2025-06-18", "2025-03-26"].contains(&p.as_str())
                })
            {
                (
                    400,
                    "application/json",
                    json!({"message":"Unsupported MCP protocol version"}).to_string(),
                )
            } else {
                let mut bytes = vec![];
                let read = request
                    .as_reader()
                    .take(8 * 1024 * 1024 + 1)
                    .read_to_end(&mut bytes);
                if read.is_err() {
                    (
                        400,
                        "application/json",
                        json!({"message":"Unable to read request"}).to_string(),
                    )
                } else if bytes.len() > 8 * 1024 * 1024 {
                    (
                        413,
                        "application/json",
                        json!({"message":"Use chunked imports"}).to_string(),
                    )
                } else {
                    match serde_json::from_slice::<Value>(&bytes) {
                        Ok(v) if path == "/mcp" => {
                            if !v.is_object()
                                || v["jsonrpc"] != "2.0"
                                || !(v["method"].is_string()
                                    || v.get("result").is_some()
                                    || v.get("error").is_some())
                            {
                                (400,"application/json",json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":"Invalid request"}}).to_string())
                            } else if let Some(response) = rpc(service, &v) {
                                (200, "application/json", response.to_string())
                            } else {
                                (202, "application/json", String::new())
                            }
                        }
                        Ok(v) => {
                            let (name, args) = if let Some(name) = path.strip_prefix("/api/tools/")
                            {
                                (name, &v)
                            } else {
                                (
                                    v["name"].as_str().unwrap_or(""),
                                    v.get("arguments").unwrap_or(&Value::Null),
                                )
                            };
                            let (value, failed) = service.agent_response(service.call(name, args));
                            (
                                if failed { 422 } else { 200 },
                                "application/json",
                                value.to_string(),
                            )
                        }
                        Err(e) => (
                            400,
                            "application/json",
                            json!({"message":e.to_string()}).to_string(),
                        ),
                    }
                }
            }
        } else {
            (
                404,
                "application/json",
                json!({"message":"Not found"}).to_string(),
            )
        };
        let mut response = tiny_http::Response::from_string(body)
            .with_status_code(status)
            .with_header(tiny_http::Header::from_bytes("Content-Type", mime).unwrap())
            .with_header(
                tiny_http::Header::from_bytes("X-Content-Type-Options", "nosniff").unwrap(),
            )
            .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap());
        if status == 401 {
            response = response
                .with_header(tiny_http::Header::from_bytes("WWW-Authenticate", "Bearer").unwrap());
        }
        if status == 405 {
            response =
                response.with_header(tiny_http::Header::from_bytes("Allow", "POST").unwrap());
        }
        let _ = request.respond(response);
    }
    Ok(())
}
