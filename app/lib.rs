//! APEX application: independent scheduling core and agent-facing server.
pub mod core;
pub mod customization;
pub mod data;
pub mod server;

// Retain the public library paths used by existing callers.
pub(crate) use core::placement;
pub use core::*;
pub use server::{service, transport};
#[path = "customization/dummy_customer/model/policy.rs"]
pub mod dummy_customer;
