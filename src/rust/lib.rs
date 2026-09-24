//! A deterministic scheduling kernel and durable, agent-facing tools.
pub mod activities;
pub mod calendar;
pub mod compile;
pub mod conditionals;
pub mod demo;
pub mod dispatch;
pub mod domain;
#[path = "../../customizations/dummy_customer/policy.rs"]
pub mod dummy_customer;
pub mod engine;
pub mod evolution;
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
pub mod service;
pub mod transport;
pub mod urgency;
pub mod validate;

mod placement;
