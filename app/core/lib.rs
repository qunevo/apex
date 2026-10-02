//! Scheduling semantics and algorithms; no transport or persistent state.

/// Engine version recorded in result provenance.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub mod activities;
pub mod calendar;
pub mod compile;
pub mod conditionals;
pub mod demo;
pub mod dispatch;
pub mod domain;
pub mod extensions;
pub mod improve;
pub mod language;
pub mod material;
pub mod metrics;
pub mod model;
pub mod policy;
pub mod production;
pub mod queues;
pub mod rules;
pub mod search;
pub mod urgency;
pub mod validate;
pub mod xe;
pub mod xg;
pub mod xh;
pub mod xt;

pub(crate) mod placement;

// The bundled synthetic customization is statically registered by `extensions`.
#[path = "../customization/demo/model/policy.rs"]
pub mod demo_policy;
