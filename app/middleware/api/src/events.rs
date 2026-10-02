//! Change notifications for clients that display state, such as the desktop UI.
//! Events identify what changed; clients read the current state through the API.
use apex_control::TenantId;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    #[serde(skip)]
    pub tenant: TenantId,
    /// `scenario`, `run` or `result`.
    pub kind: &'static str,
    pub id: String,
    pub scenario: String,
    pub at: DateTime<Utc>,
}

/// In-process fan-out. Multiple server instances need a shared channel
/// (e.g. PostgreSQL LISTEN/NOTIFY) before they can share subscribers.
#[derive(Clone)]
pub struct Events(broadcast::Sender<Event>);

impl Default for Events {
    fn default() -> Self {
        Self(broadcast::channel(1024).0)
    }
}

impl Events {
    pub fn publish(&self, tenant: TenantId, kind: &'static str, id: String, scenario: String) {
        // No subscribers is fine; events are hints, not state.
        let _ = self.0.send(Event {
            tenant,
            kind,
            id,
            scenario,
            at: Utc::now(),
        });
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.0.subscribe()
    }
}
