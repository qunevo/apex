//! End to end through the HTTP router: agents operate over MCP and HTTP, display
//! clients read and receive change events, tenants stay isolated.
use apex_control::{Role, Store, Tenant, TenantId, memory::MemoryStore};
use apex_control_server::{
    auth::{AuthFile, TokenEntry, Tokens, hash},
    engines, http, state, worker, workers,
};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use std::sync::Arc;
use tower::ServiceExt;

struct Harness {
    app: Router,
    state: http::AppState,
}

impl Harness {
    async fn new() -> Self {
        let (a, b) = (TenantId::new(), TenantId::new());
        let token = |sha: &str, tenant, actor: &str, roles: Vec<Role>| TokenEntry {
            sha256: hash(sha),
            tenant,
            actor: actor.into(),
            roles,
        };
        let file = AuthFile {
            tenants: vec![
                Tenant {
                    id: a,
                    name: "A".into(),
                },
                Tenant {
                    id: b,
                    name: "B".into(),
                },
            ],
            tokens: vec![
                token("agent", a, "agent", vec![Role::Admin]),
                token("display", a, "display", vec![Role::Viewer]),
                token("other", b, "other", vec![Role::Admin]),
            ],
        };
        let (tokens, tenants) = Tokens::from_file(file).unwrap();
        let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
        for t in &tenants {
            store.ensure_tenant(t).await.unwrap();
        }
        let state = state(store, engines(), tokens);
        Self {
            app: http::router(state.clone()),
            state,
        }
    }

    async fn send(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(t) = token {
            request = request.header("authorization", format!("Bearer {t}"));
        }
        let request = match body {
            Some(b) => request
                .header("content-type", "application/json")
                .body(Body::from(b.to_string())),
            None => request.body(Body::empty()),
        }
        .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    async fn mcp(&self, token: &str, method: &str, params: Value) -> Value {
        let (status, body) = self
            .send(
                "POST",
                "/mcp",
                Some(token),
                Some(json!({"jsonrpc":"2.0","id":1,"method":method,"params":params})),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        body
    }

    async fn tool(&self, token: &str, name: &str, arguments: Value) -> (bool, Value) {
        let body = self
            .mcp(
                token,
                "tools/call",
                json!({"name": name, "arguments": arguments}),
            )
            .await;
        let result = &body["result"];
        (
            result["isError"] == json!(true),
            result["structuredContent"].clone(),
        )
    }

    async fn work(&self) {
        let w = worker(&self.state, engines(), "test");
        assert!(workers::process_one(&w, &self.state.events).await.unwrap());
    }
}

#[tokio::test]
async fn configurable_import_limit_preserves_auth_validation_and_revisions() {
    let mut h = Harness::new().await;
    let mut facts = serde_json::to_value(apex_engine::demo::problem(2)).unwrap();
    facts["assumptions"] = json!(["x".repeat(http::DEFAULT_MAX_REQUEST_BYTES)]);
    let request =
        json!({"name":"Large synthetic import", "engine":"apex", "content":{"facts":facts}});
    assert_eq!(
        h.send(
            "POST",
            "/v1/scenarios",
            Some("agent"),
            Some(request.clone())
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(
        h.state
            .control
            .list_scenarios(h.state.tokens.actor("agent").unwrap())
            .await
            .unwrap()
            .is_empty()
    );
    let mcp = json!({"jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{"name":"scenarios.create", "arguments":request}});
    assert_eq!(
        h.send("POST", "/mcp", Some("agent"), Some(mcp.clone()))
            .await
            .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    h.app = http::router_with_body_limit(h.state.clone(), 3 * 1024 * 1024);
    assert_eq!(
        h.send("POST", "/v1/scenarios", None, Some(request.clone()))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        h.send(
            "POST",
            "/v1/scenarios",
            Some("display"),
            Some(request.clone())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, created) = h
        .send(
            "POST",
            "/v1/scenarios",
            Some("agent"),
            Some(request.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let id = created["scenario"]["id"].as_str().unwrap();
    let path = format!("/v1/scenarios/{id}/revisions");
    let revise = json!({"expected_revision":1, "content":{"facts":facts}});
    assert_eq!(
        h.send("POST", &path, Some("agent"), Some(revise.clone()))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        h.send("POST", &path, Some("agent"), Some(revise)).await.0,
        StatusCode::CONFLICT
    );
    let (status, response) = h.send("POST", "/mcp", Some("agent"), Some(mcp)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(response["result"]["isError"], false);
    facts["resources"] = json!([]);
    let invalid = json!({"name":"Invalid large input", "engine":"apex", "content":{"facts":facts}});
    assert_eq!(
        h.send("POST", "/v1/scenarios", Some("agent"), Some(invalid))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    h.app = http::router_with_body_limit(h.state.clone(), 1024);
    assert_eq!(
        h.send("POST", "/v1/scenarios", Some("agent"), Some(request))
            .await
            .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
}

#[tokio::test]
async fn agents_operate_and_display_clients_follow() {
    let h = Harness::new().await;
    let mut events = h.state.events.subscribe();

    assert_eq!(
        h.send("GET", "/v1/scenarios", None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        h.send("GET", "/v1/scenarios", Some("nope"), None).await.0,
        StatusCode::UNAUTHORIZED
    );

    // MCP discovery.
    let init = h
        .mcp(
            "agent",
            "initialize",
            json!({"protocolVersion": "2025-06-18"}),
        )
        .await;
    assert_eq!(init["result"]["serverInfo"]["name"], "apex-control");
    let tools = h.mcp("agent", "tools/list", json!({})).await;
    let list = tools["result"]["tools"].as_array().unwrap();
    let start = list.iter().find(|t| t["name"] == "runs.start").unwrap();
    assert_eq!(start["annotations"]["readOnlyHint"], false);
    assert_eq!(list.len(), apex_control_server::tools::catalog().len());
    let result_tool = list.iter().find(|t| t["name"] == "results.get").unwrap();
    let uri = result_tool["_meta"]["ui"]["resourceUri"].clone();
    let resources = h.mcp("display", "resources/list", json!({})).await;
    assert_eq!(resources["result"]["resources"][0]["uri"], uri);
    let resource = h
        .mcp("display", "resources/read", json!({"uri": uri}))
        .await;
    let content = &resource["result"]["contents"][0];
    assert_eq!(content["mimeType"], "text/html;profile=mcp-app");
    let html = content["text"].as_str().unwrap();
    assert!(html.contains("ui/initialize") && html.contains("results.get"));
    assert!(!html.contains("/* APEX_APP_"));
    let missing = h
        .mcp(
            "display",
            "resources/read",
            json!({"uri":"file:///private"}),
        )
        .await;
    assert_eq!(missing["error"]["code"], -32002);

    // The agent creates a scenario and plans it over MCP.
    let facts = serde_json::to_value(apex_engine::demo::problem(10)).unwrap();
    let (failed, created) = h
        .tool(
            "agent",
            "scenarios.create",
            json!({"name":"Week 40","engine":"apex","content":{"facts":facts}}),
        )
        .await;
    assert!(!failed, "{created}");
    let scenario = created["scenario"]["id"].as_str().unwrap().to_string();
    let (failed, run) = h
        .tool("agent", "runs.start", json!({"scenario_id": scenario, "options": {"method":"hypersearch","options":{"iterations":4,"seed":5}}}))
        .await;
    assert!(!failed, "{run}");
    assert_eq!(run["state"], "queued");
    h.work().await;

    // The display client reads state over HTTP.
    let path = format!("/v1/runs/{}", run["id"].as_str().unwrap());
    let (status, finished) = h.send("GET", &path, Some("display"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(finished["state"], "succeeded", "{finished}");
    let result_id = finished["result"].as_str().unwrap().to_string();
    let (_, result) = h
        .send(
            "GET",
            &format!("/v1/results/{result_id}"),
            Some("display"),
            None,
        )
        .await;
    assert_eq!(result["validation"]["valid"], true);
    assert_eq!(result["view"]["operations"].as_array().unwrap().len(), 10);
    assert!(result.get("schedule").is_none());
    let (_, full) = h
        .send(
            "GET",
            &format!("/v1/results/{result_id}?include_schedule=true"),
            Some("display"),
            None,
        )
        .await;
    assert!(full["schedule"]["assignments"].is_array());

    // Display clients cannot change anything, over either interface.
    let (status, body) = h
        .send(
            "POST",
            &format!("/v1/scenarios/{scenario}/runs"),
            Some("display"),
            Some(json!({})),
        )
        .await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("FORBIDDEN"))
    );
    let (failed, body) = h
        .tool(
            "display",
            "results.approve",
            json!({"result_id": result_id}),
        )
        .await;
    assert!(failed);
    assert_eq!(body["code"], "FORBIDDEN");

    // Other tenants see nothing.
    let (status, _) = h
        .send(
            "GET",
            &format!("/v1/scenarios/{scenario}"),
            Some("other"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, listed) = h.send("GET", "/v1/scenarios", Some("other"), None).await;
    assert_eq!(listed, json!([]));

    // Approve and publish over MCP; the scenario shows its published plan.
    for name in ["results.approve", "results.publish"] {
        let (failed, body) = h
            .tool("agent", name, json!({"result_id": result_id, "note": "ok"}))
            .await;
        assert!(!failed, "{body}");
    }
    let (_, current) = h
        .send(
            "GET",
            &format!("/v1/scenarios/{scenario}"),
            Some("display"),
            None,
        )
        .await;
    assert_eq!(current["published_result"], json!(result_id));

    // A stale revise is a conflict over HTTP as well.
    let (status, body) = h
        .send(
            "POST",
            &format!("/v1/scenarios/{scenario}/revisions"),
            Some("agent"),
            Some(json!({"expected_revision": 7, "content": {"facts": facts}})),
        )
        .await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::CONFLICT, Some("CONFLICT"))
    );

    // Every change was announced to tenant A's subscribers, in order.
    let mut kinds = Vec::new();
    while let Ok(e) = events.try_recv() {
        assert_eq!(e.scenario, scenario);
        kinds.push(e.kind);
    }
    assert_eq!(kinds.first(), Some(&"scenario"));
    assert!(kinds.contains(&"run") && kinds.contains(&"result"));
    assert_eq!(
        kinds.last(),
        Some(&"scenario"),
        "publication updates the scenario"
    );

    let request = Request::builder()
        .uri("/v1/events")
        .header("authorization", "Bearer display")
        .body(Body::empty())
        .unwrap();
    let response = h.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");
}

#[tokio::test]
async fn unsupported_input_is_explained() {
    let h = Harness::new().await;
    let (failed, body) = h
        .tool(
            "agent",
            "scenarios.create",
            json!({"name":"x","engine":"apex","content":{"facts":{"id":"broken"}}}),
        )
        .await;
    assert!(failed);
    assert_eq!(body["code"], "INVALID");
    assert_eq!(body["diagnostics"][0]["code"], "FACTS_SCHEMA");
    let (failed, body) = h.tool("agent", "nope.nothing", json!({})).await;
    assert!(failed);
    assert_eq!(body["code"], "NOT_FOUND");
}

fn packages(default: &str, version: &str) -> apex_control::packages::LoadedConfig {
    use apex_control::packages::{LoadedConfig, Package};
    LoadedConfig {
        default: default.into(),
        packages: ["alpha", "beta"]
            .into_iter()
            .map(|id| {
                (
                    id.into(),
                    Package {
                        id: id.into(),
                        version: version.into(),
                    },
                )
            })
            .collect(),
    }
}

#[tokio::test]
async fn dashboards_are_bounded_read_only_and_bound_to_the_saved_revision() {
    use apex_control::packages::{LoadedConfig, Package};
    let mut h = Harness::new().await;
    h.state.control = h.state.control.clone().with_packages(LoadedConfig {
        default: "demo".into(),
        packages: [(
            "demo".into(),
            Package {
                id: "demo".into(),
                version: "1".into(),
            },
        )]
        .into(),
    });
    h.app = http::router(h.state.clone());
    let mut facts = serde_json::to_value(apex_engine::demo::problem(60)).unwrap();
    for (index, task) in facts["tasks"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        task["stage"] = json!(format!("Stage {index}"));
    }
    let summary = json!({"sources":[{"label":"MES","revision":"7","sha256":"a".repeat(64)},{"label":"Excel","sha256":"b".repeat(64)}],
        "counts":{"mes_operations":65,"excel_operations":45,"planned_operations":60,"closed_operations":5,"operations_without_excel":20},
        "notices":[{"code":"SYNTHETIC","message":"Check the imported source"}],"notice_count":1});
    let input = json!({"name":"Showcase", "engine":"apex", "content":{"facts":facts, "source_summary":summary}});
    let (failed, created) = h.tool("agent", "scenarios.create", input.clone()).await;
    assert!(!failed, "{created}");
    let id = created["scenario"]["id"].clone();
    let mut events = h.state.events.subscribe();
    let (failed, view) = h
        .tool("display", "views.get", json!({"scenario_id":id}))
        .await;
    assert!(!failed, "{view}");
    assert!(
        events.try_recv().is_err(),
        "Reading a view must not emit mutations"
    );
    assert_eq!(view["kind"], "apex_view");
    assert_eq!(view["view_id"], "overview");
    assert_eq!(view["revision"], 1);
    assert_eq!(view["content_hash"], created["content_hash"]);
    assert!(view["result_id"].is_null());
    assert!(view.get("facts").is_none() && view.get("schedule").is_none());
    assert!(view.to_string().len() < apex_control::views::MAX_DOCUMENT_BYTES);
    let panels = view["dashboard"]["panels"].as_array().unwrap();
    let stages = panels.iter().find(|p| p["id"] == "work-mix").unwrap();
    let points = stages["points"].as_array().unwrap();
    assert_eq!(points.len(), 24);
    assert_eq!(
        points
            .iter()
            .map(|p| p["value"].as_f64().unwrap())
            .sum::<f64>(),
        60.0
    );
    let (_, sources) = h
        .tool(
            "display",
            "views.get",
            json!({"scenario_id":id,"view_id":"demo.sources"}),
        )
        .await;
    let flow = sources["dashboard"]["panels"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "source-flow")
        .unwrap();
    assert_eq!(flow["renderer"], "demo.import-flow");
    assert_eq!(flow["data"]["matched"], 40);
    let (status, http_view) = h
        .send(
            "POST",
            "/v1/views",
            Some("display"),
            Some(json!({"scenario_id":id})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(http_view, view);
    assert_eq!(
        h.send("POST", "/v1/views", None, Some(json!({"scenario_id":id})))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        h.tool("other", "views.get", json!({"scenario_id":id}))
            .await
            .1["code"],
        "NOT_FOUND"
    );
    assert!(
        h.tool(
            "display",
            "views.get",
            json!({"scenario_id":id,"view_id":"../private"})
        )
        .await
        .0
    );
    assert!(
        h.tool(
            "display",
            "views.get",
            json!({"scenario_id":id,"revision":0})
        )
        .await
        .0
    );
    assert!(
        h.tool("display", "views.get", json!({"scenario_id":id,"facts":{}}))
            .await
            .0
    );
    let (_, overview) = h
        .tool(
            "display",
            "views.get",
            json!({"scenario_id":id,"view_id":"overview"}),
        )
        .await;
    assert_eq!(overview["view_id"], "overview");

    let (_, queued) = h
        .tool("agent", "runs.start", json!({"scenario_id":id}))
        .await;
    h.work().await;
    let (_, done) = h
        .tool("display", "runs.get", json!({"run_id":queued["id"]}))
        .await;
    assert_eq!(done["state"], "succeeded", "{done}");
    let result_id = done["result"].clone();
    let (_, revised) = h
        .tool(
            "agent",
            "scenarios.revise",
            json!({"scenario_id":id,"expected_revision":1,"content":{"facts":facts}}),
        )
        .await;
    assert_eq!(revised["revision"], 2);
    let (failed, saved) = h
        .tool(
            "display",
            "views.get",
            json!({"scenario_id":id,"result_id":result_id}),
        )
        .await;
    assert!(!failed, "{saved}");
    assert_eq!(saved["revision"], 1);
    assert_eq!(saved["source_summary"], summary);
    assert_eq!(saved["validation_valid"], true);
    assert!(
        saved["dashboard"]["panels"]
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p["section"] == "result")
    );
    assert!(
        h.tool(
            "display",
            "views.get",
            json!({"scenario_id":id,"result_id":result_id,"revision":2})
        )
        .await
        .0
    );
    let (_, latest) = h
        .tool("display", "views.get", json!({"scenario_id":id}))
        .await;
    assert_eq!(latest["revision"], 2);
    assert!(latest["source_summary"].is_null());
    let (_, no_sources) = h
        .tool(
            "display",
            "views.get",
            json!({"scenario_id":id,"view_id":"demo.sources"}),
        )
        .await;
    assert!(
        no_sources["dashboard"]["notes"]
            .to_string()
            .contains("coverage is unknown")
    );
    let (_, another) = h.tool("agent", "scenarios.create", input.clone()).await;
    assert!(
        h.tool(
            "display",
            "views.get",
            json!({"scenario_id":another["scenario"]["id"],"result_id":result_id})
        )
        .await
        .0
    );
    let (_, foreign) = h.tool("other", "scenarios.create", input).await;
    assert_eq!(
        h.tool(
            "display",
            "views.get",
            json!({"scenario_id":foreign["scenario"]["id"],"result_id":result_id})
        )
        .await
        .1["code"],
        "NOT_FOUND"
    );
}

#[tokio::test]
async fn view_discovery_evidence_limits_and_package_fallback() {
    let mut h = Harness::new().await;
    let catalog = h.mcp("display", "tools/list", json!({})).await;
    let tool = catalog["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "views.get")
        .unwrap();
    assert_eq!(tool["annotations"]["readOnlyHint"], true);
    let uri = &tool["_meta"]["ui"]["resourceUri"];
    let resource = h.mcp("display", "resources/read", json!({"uri":uri})).await;
    let resource = &resource["result"]["contents"][0];
    assert_eq!(resource["mimeType"], "text/html;profile=mcp-app");
    let html = resource["text"].as_str().unwrap();
    assert!(html.contains("demo.import-flow") && html.contains("views.get"));
    assert!(!html.contains("/* APEX_"));
    assert_eq!(resource["_meta"]["ui"]["csp"]["connectDomains"], json!([]));
    let facts = serde_json::to_value(apex_engine::demo::problem(2)).unwrap();
    for version in ["1", "2"] {
        h.configure("alpha", version);
        let (_, created) = h
            .tool(
                "agent",
                "scenarios.create",
                json!({"name":"Generic", "engine":"apex", "content":{"facts":facts}}),
            )
            .await;
        let id = created["scenario"]["id"].clone();
        let (_, view) = h
            .tool("display", "views.get", json!({"scenario_id":id}))
            .await;
        assert_eq!(view["view_id"], "overview");
        assert_eq!(view["available_views"].as_array().unwrap().len(), 1);
        assert!(
            h.tool(
                "display",
                "views.get",
                json!({"scenario_id":id,"view_id":"demo.sources"})
            )
            .await
            .0
        );
    }
    let summary = json!({"sources":[],"counts":{},"notices":[],"notice_count":0});
    let mut variants = vec![summary.clone(); 4];
    variants[0]["sources"] = json!([{"label":"Source","sha256":"bad"}]);
    variants[1]["notices"] = json!([{"code":"X","message":"x".repeat(501)}]);
    variants[1]["notice_count"] = json!(1);
    variants[2]["notices"] = json!([{"code":"X","message":"m"}]);
    variants[3]["counts"] =
        Value::Object((0..33).map(|i| (format!("count{i}"), json!(1))).collect());
    for invalid in variants {
        let (failed, body) = h.tool("agent", "scenarios.create", json!({"name":"Invalid evidence", "engine":"apex", "content":{"facts":facts,"source_summary":invalid}})).await;
        assert!(failed, "{body}");
        assert_eq!(body["code"], "INVALID");
    }
}

impl Harness {
    fn configure(&mut self, default: &str, version: &str) {
        self.state.control = self
            .state
            .control
            .clone()
            .with_packages(packages(default, version));
        self.app = http::router(self.state.clone());
    }
}

#[tokio::test]
async fn configured_scenarios_pin_packages_across_revisions_and_results() {
    let mut h = Harness::new().await;
    let facts = serde_json::to_value(apex_engine::demo::problem(3)).unwrap();
    let request = json!({"name":"Synthetic","engine":"apex","content":{"facts": facts}});
    let mut explicit = request.clone();
    explicit["customization"] = json!("beta");
    assert!(
        h.tool("agent", "scenarios.create", explicit.clone())
            .await
            .0
    );
    h.configure("alpha", "1");
    let (failed, created) = h.tool("agent", "scenarios.create", request.clone()).await;
    assert!(!failed, "{created}");
    let id = created["scenario"]["id"].as_str().unwrap();
    let (_, first) = h
        .tool(
            "display",
            "revisions.get",
            json!({"scenario_id": id, "number":1}),
        )
        .await;
    assert_eq!(
        first["content"]["customization_package"],
        json!({"id":"alpha","version":"1"})
    );
    let (status, beta) = h
        .send(
            "POST",
            "/v1/scenarios",
            Some("agent"),
            Some(explicit.clone()),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, beta_revision) = h
        .tool(
            "display",
            "revisions.get",
            json!({"scenario_id": beta["scenario"]["id"],"number":1}),
        )
        .await;
    assert_eq!(
        beta_revision["content"]["customization_package"]["id"],
        "beta"
    );
    for selection in ["missing", "../alpha"] {
        explicit["customization"] = json!(selection);
        assert!(
            h.tool("agent", "scenarios.create", explicit.clone())
                .await
                .0
        );
    }
    let mut forged = request.clone();
    forged["content"]["customization_package"] = json!({"id":"alpha","version":"forged"});
    assert!(h.tool("agent", "scenarios.create", forged.clone()).await.0);

    // Changing the default does not rebind existing scenarios.
    h.configure("beta", "1");
    let revise = json!({"scenario_id":id,"expected_revision":1,"content":{"facts":facts}});
    let mut switched = revise.clone();
    switched["content"]["customization_package"] = json!({"id":"beta","version":"1"});
    assert!(h.tool("agent", "scenarios.revise", switched).await.0);
    assert!(!h.tool("agent", "scenarios.revise", revise).await.0);
    let (_, second) = h
        .tool(
            "display",
            "revisions.get",
            json!({"scenario_id":id,"number":2}),
        )
        .await;
    assert_eq!(first["content_hash"], second["content_hash"]);
    let (failed, run) = h
        .tool("agent", "runs.start", json!({"scenario_id":id}))
        .await;
    assert!(!failed, "{run}");
    h.work().await;
    let (_, done) = h
        .tool("display", "runs.get", json!({"run_id":run["id"]}))
        .await;
    assert_eq!(done["state"], "succeeded", "{done}");
    let (_, result) = h
        .tool(
            "display",
            "results.get",
            json!({"result_id":done["result"]}),
        )
        .await;
    assert_eq!(
        result["provenance"]["customization_package"],
        first["content"]["customization_package"]
    );
    assert_eq!(result["provenance"]["content_hash"], second["content_hash"]);
    assert_eq!(result["revision"], 2);

    // An upgraded installation retains historical reads but refuses new work on a missing version.
    h.configure("beta", "2");
    assert!(
        h.tool("agent", "runs.start", json!({"scenario_id":id}))
            .await
            .0
    );
    assert!(
        h.tool(
            "agent",
            "scenarios.revise",
            json!({"scenario_id":id,"expected_revision":2,"content":{"facts":facts}})
        )
        .await
        .0
    );
    assert!(
        !h.tool(
            "display",
            "results.get",
            json!({"result_id":done["result"]})
        )
        .await
        .0
    );
    let (failed, other) = h
        .tool(
            "other",
            "revisions.get",
            json!({"scenario_id":id,"number":1}),
        )
        .await;
    assert!(failed);
    assert_eq!(other["code"], "NOT_FOUND");
}

#[tokio::test]
async fn overview_uses_product_ready_and_withholds_unknown_outcomes() {
    use apex_control::views::{Context, Overview, Provider};
    let h = Harness::new().await;
    let facts = json!({
        "id":"overview-evidence", "epoch":"2026-10-05T06:00:00Z", "horizon":86400,
        "resources":[{"id":"R","calendar":[{"start":0,"end":86400}]}],
        "jobs":[{"id":"J1","item":"A","quantity":1,"due":7200},{"id":"J2","item":"B","quantity":1}],
        "orders":[{"id":"O1","jobs":["J1"],"due":7200},{"id":"O2","jobs":["J2"]}],
        "tasks":[
            {"id":"T1","job":"J1","due":7200,"modes":[{"id":"M","primary":"R","phases":[{"id":"run","work":3600,"requirements":[{"resource":"R","retain":true}]}]}],
             "post":[{"id":"finish","releases_product":true,"modes":[{"id":"F","primary":"R","phases":[{"id":"finish","work":7200,"requirements":[{"resource":"R","retain":true}]}]}]}]},
            {"id":"T2","job":"J2","modes":[{"id":"M","primary":"R","phases":[{"id":"run","work":60,"requirements":[{"resource":"R","retain":true}]}]}]}
        ]
    });
    let (failed, created) = h
        .tool(
            "agent",
            "scenarios.create",
            json!({"name":"Standard overview", "engine":"apex", "content":{"facts":facts}}),
        )
        .await;
    assert!(!failed, "{created}");
    let id = created["scenario"]["id"].clone();
    let (_, before) = h
        .tool("display", "views.get", json!({"scenario_id":id}))
        .await;
    assert_eq!(before["view_id"], "overview");
    assert!(before["package"].is_null());
    let metric = |d: &Value, name: &str| {
        d["metrics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|m| m["label"] == name)
            .unwrap()["value"]
            .clone()
    };
    assert!(metric(&before["dashboard"], "Late orders").is_null());
    let (failed, run) = h
        .tool("agent", "runs.start", json!({"scenario_id":id}))
        .await;
    assert!(!failed, "{run}");
    h.work().await;
    let (_, run) = h
        .tool("display", "runs.get", json!({"run_id":run["id"]}))
        .await;
    assert_eq!(run["state"], "succeeded", "{run}");
    let (failed, view) = h
        .tool(
            "display",
            "views.get",
            json!({"scenario_id":id,"result_id":run["result"]}),
        )
        .await;
    assert!(!failed, "{view}");
    assert_eq!(metric(&view["dashboard"], "Late orders"), 1.0);
    assert_eq!(metric(&view["dashboard"], "On-time orders"), 0.0);
    let panels = view["dashboard"]["panels"].as_array().unwrap();
    let critical = panels
        .iter()
        .find(|p| p["id"] == "critical-orders")
        .unwrap();
    assert_eq!(critical["rows"][0][0], "O1");
    assert_eq!(critical["rows"][0][5], "Late");
    assert!(
        view["dashboard"]["notes"]
            .to_string()
            .contains("1 orders have no due date")
    );
    let actor = h.state.tokens.actor("agent").unwrap();
    let scenario = h
        .state
        .control
        .get_scenario(actor, serde_json::from_value(id).unwrap())
        .await
        .unwrap();
    let revision = h
        .state
        .control
        .get_revision(actor, scenario.id, 1)
        .await
        .unwrap();
    let mut result = h
        .state
        .control
        .get_result(
            actor,
            serde_json::from_value(run["result"].clone()).unwrap(),
        )
        .await
        .unwrap();
    let op = result
        .view
        .operations
        .iter()
        .find(|o| o.id == "T1")
        .unwrap();
    assert!(
        op.end <= 7200,
        "The main operation alone would falsely look on time"
    );
    assert!(result.schedule["order_completions"]["O1"].as_i64().unwrap() > 7200);

    // Reject outcome claims when independent validation failed, even if metrics remain present.
    result.validation.valid = false;
    let invalid = serde_json::to_value(
        Overview
            .build(&Context {
                scenario: &scenario,
                revision: &revision,
                result: Some(&result),
            })
            .unwrap(),
    )
    .unwrap();
    assert!(metric(&invalid, "Late orders").is_null());
    assert!(metric(&invalid, "On-time orders").is_null());
    assert!(invalid["notes"].to_string().contains("failed validation"));

    // A missing saved completion must stay visible rather than becoming zero/on time.
    result.validation.valid = true;
    result.schedule["order_completions"]
        .as_object_mut()
        .unwrap()
        .remove("O1");
    let missing = serde_json::to_value(
        Overview
            .build(&Context {
                scenario: &scenario,
                revision: &revision,
                result: Some(&result),
            })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(missing["panels"][0]["rows"][0][5], "Not scheduled");
    result.metrics.insert("order_due_count".into(), 0.0);
    let undated = serde_json::to_value(
        Overview
            .build(&Context {
                scenario: &scenario,
                revision: &revision,
                result: Some(&result),
            })
            .unwrap(),
    )
    .unwrap();
    assert!(metric(&undated, "On-time orders").is_null());
}

#[tokio::test]
async fn worker_rechecks_package_after_configuration_changes() {
    let mut h = Harness::new().await;
    h.configure("alpha", "1");
    let (_, scenario) = h.tool("agent", "scenarios.create", json!({"name":"Queued","engine":"apex","content":{"facts":apex_engine::demo::problem(2)}})).await;
    let (_, run) = h
        .tool(
            "agent",
            "runs.start",
            json!({"scenario_id":scenario["scenario"]["id"]}),
        )
        .await;
    h.configure("alpha", "2");
    let w = worker(&h.state, engines(), "new-installation");
    assert!(w.claim().await.unwrap().is_none());
    let (_, failed) = h
        .tool("display", "runs.get", json!({"run_id":run["id"]}))
        .await;
    assert_eq!(failed["state"], "failed");
    assert_eq!(
        failed["diagnostics"][0]["code"],
        "CUSTOMIZATION_UNAVAILABLE"
    );
    assert!(failed.get("result").is_none());
}
