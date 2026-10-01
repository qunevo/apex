//! APEX control platform: the shared application layer behind HTTP, MCP, CLI and UI.
//!
//! It owns tenants, versioned planning scenarios, background runs, result
//! provenance, approval and publication. Engines are reached only through
//! [`EngineAdapter`]; persistence only through [`Store`]. This crate has no
//! database, transport or scheduling-engine dependency.
pub mod auth;
pub mod engine;
pub mod error;
pub mod memory;
pub mod model;
pub mod ops;
pub mod store;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod worker;

pub use auth::{Actor, Permission, Role};
pub use engine::{EngineAdapter, EngineRegistry};
pub use error::{Error, Result};
pub use model::*;
pub use ops::Control;
pub use store::Store;
