//! Application orchestration, persistence and client-facing protocols.
//! Existing `apex` library paths remain available through these compatibility modules.
#[path = "data/files.rs"]
pub mod data;
#[path = "control/compat.rs"]
pub mod service;
#[path = "api/compat.rs"]
pub mod transport;

pub mod server {
    pub use super::{service, transport};
    pub use apex_control::packages as config;
}
