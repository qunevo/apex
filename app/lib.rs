//! APEX application: independent scheduling core and agent-facing server.
pub use apex_engine as core;
pub mod customization;
pub mod data;
pub mod server;

// Retain the public library paths used by existing callers.
pub use apex_engine::*;
pub use server::{service, transport};
