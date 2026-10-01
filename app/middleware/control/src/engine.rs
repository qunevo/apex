//! The contract between the control platform and any scheduling engine.
use crate::model::{ComponentRef, Diagnostic, ScenarioContent, ScheduleView, ValidationReport};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// What an engine can do, used by clients to offer only supported controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EngineDescriptor {
    pub engine: ComponentRef,
    /// Identifier of the facts model, e.g. a schema version.
    pub model: String,
    /// Planning-intent declaration kinds this engine can represent.
    pub declarations: Vec<String>,
    /// Run methods, e.g. a fast construction or a longer search.
    pub methods: Vec<String>,
    pub extensions: Vec<ComponentRef>,
}

/// Facts and intent translated into the engine's executable input.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub model: Value,
    pub extensions: Vec<ComponentRef>,
    pub warnings: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
pub struct EngineOutput {
    pub schedule: Value,
    pub view: ScheduleView,
    pub metrics: BTreeMap<String, f64>,
    pub seed: Option<u64>,
}

/// Cooperative cancellation shared between the worker and the engine call.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);
impl CancelToken {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// Synchronous engine contract. Workers call it off the async runtime.
pub trait EngineAdapter: Send + Sync {
    fn descriptor(&self) -> EngineDescriptor;
    /// Check run options when a run is requested, before it is queued.
    fn check_options(&self, options: &Value) -> Result<(), Vec<Diagnostic>>;
    /// Check facts when a revision is saved. Preparation repeats this check.
    fn check_facts(&self, facts: &Value) -> Result<(), Vec<Diagnostic>>;
    /// Translate facts and supported declarations into executable input.
    fn prepare(&self, content: &ScenarioContent) -> Result<Prepared, Vec<Diagnostic>>;
    fn run(
        &self,
        prepared: &Prepared,
        options: &Value,
        cancel: &CancelToken,
    ) -> Result<EngineOutput, Vec<Diagnostic>>;
    /// Independently check a produced schedule against the prepared input.
    fn validate(&self, prepared: &Prepared, schedule: &Value) -> ValidationReport;
}

#[derive(Clone, Default)]
pub struct EngineRegistry(BTreeMap<String, Arc<dyn EngineAdapter>>);
impl EngineRegistry {
    pub fn with(mut self, adapter: Arc<dyn EngineAdapter>) -> Self {
        self.0.insert(adapter.descriptor().engine.id, adapter);
        self
    }
    pub fn get(&self, id: &str) -> Option<&Arc<dyn EngineAdapter>> {
        self.0.get(id)
    }
    pub fn descriptors(&self) -> Vec<EngineDescriptor> {
        self.0.values().map(|a| a.descriptor()).collect()
    }
}

/// Reject declarations the engine cannot represent instead of silently ignoring them.
pub fn unsupported_declarations(
    descriptor: &EngineDescriptor,
    content: &ScenarioContent,
) -> Vec<Diagnostic> {
    content
        .intent
        .declarations
        .iter()
        .filter(|d| !descriptor.declarations.contains(&d.kind))
        .map(|d| {
            Diagnostic::new(
                "UNSUPPORTED_DECLARATION",
                format!(
                    "Engine {} cannot represent declarations of kind {}",
                    descriptor.engine.id, d.kind
                ),
            )
            .at(&d.id)
        })
        .collect()
}
