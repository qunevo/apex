//! APEX control server: one process exposing the control operations over HTTP
//! and MCP, streaming change events and running background workers.
pub mod auth;
pub mod events;
pub mod http;
pub mod tools;
pub mod workers;

use apex_control::{Control, EngineRegistry, Store, worker::Worker};
use std::sync::Arc;

/// Engines compiled into this server.
pub fn engines() -> EngineRegistry {
    EngineRegistry::default().with(Arc::new(apex_engine_adapter::ApexEngine))
}

/// Shared state for a store and set of engines.
pub fn state(
    store: Arc<dyn Store>,
    engines: EngineRegistry,
    tokens: auth::Tokens,
) -> http::AppState {
    http::AppState {
        control: Control::new(store, engines),
        events: events::Events::default(),
        tokens: Arc::new(tokens),
        wake: Arc::new(tokio::sync::Notify::new()),
    }
}

pub fn worker(state: &http::AppState, engines: EngineRegistry, id: &str) -> Worker {
    Worker::new(state.control.store().clone(), engines, id)
}
