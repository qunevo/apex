//! HTTP API, MCP endpoint and server-sent change events.
use crate::{auth::Tokens, events::Events, tools, ui};
use apex_control::{Actor, Control, Error};
use axum::{
    Json, Router,
    extract::{FromRequestParts, Path, State},
    http::{StatusCode, request::Parts},
    response::{
        IntoResponse, Response,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{convert::Infallible, sync::Arc};
use tokio::sync::Notify;
use tokio_stream::{Stream, StreamExt, wrappers::BroadcastStream};

#[derive(Clone)]
pub struct AppState {
    pub control: Control,
    pub events: Events,
    pub tokens: Arc<Tokens>,
    /// Wakes idle workers when a run is queued.
    pub wake: Arc<Notify>,
}

pub struct ApiError(Error);
impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        Self(e)
    }
}
fn error_body(e: &Error) -> Value {
    let diagnostics = match e {
        Error::Invalid { diagnostics, .. } => diagnostics.clone(),
        _ => Vec::new(),
    };
    json!({"code": e.code(), "message": e.to_string(), "diagnostics": diagnostics})
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            Error::Forbidden(_) => StatusCode::FORBIDDEN,
            Error::Conflict(_) => StatusCode::CONFLICT,
            Error::Invalid { .. } => StatusCode::UNPROCESSABLE_ENTITY,
            Error::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(error_body(&self.0))).into_response()
    }
}

/// The authenticated actor from `Authorization: Bearer <token>`.
pub struct Authenticated(pub Actor);
impl FromRequestParts<AppState> for Authenticated {
    type Rejection = Response;
    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .and_then(|token| state.tokens.actor(token.trim()))
            .cloned()
            .map(Authenticated)
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(
                        json!({"code":"UNAUTHORIZED","message":"A valid bearer token is required"}),
                    ),
                )
                    .into_response()
            })
    }
}

async fn op(
    state: &AppState,
    actor: &Actor,
    name: &str,
    args: Value,
) -> Result<Json<Value>, ApiError> {
    let value = tools::call(&state.control, &state.events, actor, name, args).await?;
    if name == "runs.start" {
        state.wake.notify_waiters();
    }
    Ok(Json(value))
}

fn merge(mut body: Value, extra: Value) -> Value {
    if !body.is_object() {
        body = json!({});
    }
    for (k, v) in extra.as_object().unwrap() {
        body[k] = v.clone();
    }
    body
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(|| async { Json(json!({"status":"ok"})) }))
        .route("/v1/engines", get(engines))
        .route("/v1/scenarios", get(list_scenarios).post(create_scenario))
        .route("/v1/scenarios/{id}", get(get_scenario))
        .route("/v1/scenarios/{id}/revisions", post(revise_scenario))
        .route("/v1/scenarios/{id}/revisions/{number}", get(get_revision))
        .route("/v1/scenarios/{id}/runs", get(list_runs).post(start_run))
        .route("/v1/scenarios/{id}/results", get(list_results))
        .route("/v1/runs/{id}", get(get_run))
        .route("/v1/runs/{id}/cancel", post(cancel_run))
        .route("/v1/results/{id}", get(get_result))
        .route("/v1/results/{id}/{action}", post(decide_result))
        .route("/v1/events", get(events))
        .route("/mcp", post(mcp))
        .with_state(state)
}

type Reply = Result<Json<Value>, ApiError>;

async fn engines(State(s): State<AppState>, Authenticated(a): Authenticated) -> Reply {
    op(&s, &a, "engines.list", json!({})).await
}
async fn list_scenarios(State(s): State<AppState>, Authenticated(a): Authenticated) -> Reply {
    op(&s, &a, "scenarios.list", json!({})).await
}
async fn create_scenario(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Json(body): Json<Value>,
) -> Reply {
    op(&s, &a, "scenarios.create", body).await
}
async fn get_scenario(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path(id): Path<String>,
) -> Reply {
    op(&s, &a, "scenarios.get", json!({"scenario_id": id})).await
}
async fn revise_scenario(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Reply {
    op(
        &s,
        &a,
        "scenarios.revise",
        merge(body, json!({"scenario_id": id})),
    )
    .await
}
async fn get_revision(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path((id, number)): Path<(String, u64)>,
) -> Reply {
    op(
        &s,
        &a,
        "revisions.get",
        json!({"scenario_id": id, "number": number}),
    )
    .await
}
async fn list_runs(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path(id): Path<String>,
) -> Reply {
    op(&s, &a, "runs.list", json!({"scenario_id": id})).await
}
async fn start_run(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Reply {
    let body = body.map(|Json(b)| b).unwrap_or(json!({}));
    op(
        &s,
        &a,
        "runs.start",
        merge(body, json!({"scenario_id": id})),
    )
    .await
}
async fn list_results(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path(id): Path<String>,
) -> Reply {
    op(&s, &a, "results.list", json!({"scenario_id": id})).await
}
async fn get_run(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path(id): Path<String>,
) -> Reply {
    op(&s, &a, "runs.get", json!({"run_id": id})).await
}
async fn cancel_run(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path(id): Path<String>,
) -> Reply {
    op(&s, &a, "runs.cancel", json!({"run_id": id})).await
}
async fn get_result(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path(id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Reply {
    let include = query.get("include_schedule").is_some_and(|v| v == "true");
    op(
        &s,
        &a,
        "results.get",
        json!({"result_id": id, "include_schedule": include}),
    )
    .await
}
async fn decide_result(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Path((id, action)): Path<(String, String)>,
    body: Option<Json<Value>>,
) -> Reply {
    let name = match action.as_str() {
        "approve" => "results.approve",
        "reject" => "results.reject",
        "publish" => "results.publish",
        _ => return Err(Error::NotFound(format!("action {action}")).into()),
    };
    let body = body.map(|Json(b)| b).unwrap_or(json!({}));
    op(&s, &a, name, merge(body, json!({"result_id": id}))).await
}

/// Server-sent events for the actor's tenant. Each event names what changed.
async fn events(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    a.require(apex_control::Permission::Read)?;
    let tenant = a.tenant;
    let stream = BroadcastStream::new(s.events.subscribe()).filter_map(move |item| match item {
        Ok(e) if e.tenant == tenant => Some(Ok(SseEvent::default()
            .event(e.kind)
            .json_data(&e)
            .expect("event serializes"))),
        Ok(_) => None,
        // A slow client missed events; tell it to reload everything.
        Err(_) => Some(Ok(SseEvent::default().event("resync").data("{}"))),
    });
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

const MCP_VERSION: &str = "2025-06-18";

/// Stateless MCP over streamable HTTP (JSON responses). Tools mirror the HTTP API.
async fn mcp(
    State(s): State<AppState>,
    Authenticated(a): Authenticated,
    Json(request): Json<Value>,
) -> Response {
    let id = request.get("id").cloned();
    let method = request["method"].as_str().unwrap_or_default();
    let Some(id) = id else {
        // Notifications such as notifications/initialized need no response.
        return StatusCode::ACCEPTED.into_response();
    };
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": request["params"]["protocolVersion"].as_str().unwrap_or(MCP_VERSION),
            "capabilities": {"tools": {}, "resources": {}},
            "serverInfo": {"name": "apex-control", "version": env!("CARGO_PKG_VERSION")},
            "instructions": "APEX control platform. Scenarios hold immutable revisions of facts and planning intent. Runs plan one revision in the background; results are validated, then approved and published by an approver. Use scenarios.revise with expected_revision; publishing fails for results of superseded revisions."
        })),
        "ping" => Ok(json!({})),
        "resources/list" => Ok(json!({"resources": [ui::descriptor()]})),
        "resources/read" if request["params"]["uri"] == ui::URI => Ok(ui::read()),
        "resources/read" => Err(json!({"code": -32002, "message": "Resource not found"})),
        "tools/list" => Ok(json!({"tools": tools::catalog().iter().map(|t| {
            let mut tool = json!({
            "name": t.name,
            "description": t.description,
            "inputSchema": t.input,
            "annotations": {"readOnlyHint": !t.mutates},
            });
            if t.name == "results.get" {
                tool["_meta"] = json!({"ui": {"resourceUri": ui::URI}});
            }
            tool
        }).collect::<Vec<_>>()})),
        "tools/call" => {
            let name = request["params"]["name"].as_str().unwrap_or_default();
            let args = request["params"]["arguments"].clone();
            let outcome = tools::call(&s.control, &s.events, &a, name, args).await;
            if name == "runs.start" && outcome.is_ok() {
                s.wake.notify_waiters();
            }
            let (value, is_error) = match outcome {
                Ok(v) => (v, false),
                Err(e) => (error_body(&e), true),
            };
            Ok(json!({
                "content": [{"type": "text", "text": value.to_string()}],
                "structuredContent": if value.is_object() { value.clone() } else { json!({"items": value}) },
                "isError": is_error,
            }))
        }
        _ => Err(json!({"code": -32601, "message": format!("Method not found: {method}")})),
    };
    let body = match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    };
    Json(body).into_response()
}
