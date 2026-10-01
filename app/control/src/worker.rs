//! Background run execution, independent of any async runtime. A host claims a
//! run, calls [`compute`] off its async executor while sending heartbeats, then
//! finishes the run. Persisted run state stays authoritative.
use crate::{
    engine::{CancelToken, EngineAdapter, EngineRegistry, unsupported_declarations},
    error::Result,
    model::*,
    store::{ClaimPolicy, Completion, Store},
};
use chrono::{Duration, Utc};
use std::sync::Arc;

/// A run whose lease this worker holds, with the immutable input it plans from.
pub struct Claimed {
    pub run: Run,
    pub revision: Revision,
    pub adapter: Arc<dyn EngineAdapter>,
}

#[derive(Clone)]
pub struct Worker {
    store: Arc<dyn Store>,
    engines: EngineRegistry,
    pub id: String,
    pub lease: Duration,
    pub policy: ClaimPolicy,
}

impl Worker {
    pub fn new(store: Arc<dyn Store>, engines: EngineRegistry, id: impl Into<String>) -> Self {
        Self {
            store,
            engines,
            id: id.into(),
            lease: Duration::seconds(60),
            policy: ClaimPolicy {
                max_running_per_tenant: 2,
                max_attempts: 3,
            },
        }
    }

    pub async fn claim(&self) -> Result<Option<Claimed>> {
        let now = Utc::now();
        let Some(run) = self
            .store
            .claim_run(&self.id, now, now + self.lease, self.policy)
            .await?
        else {
            return Ok(None);
        };
        let loaded = self
            .store
            .get_revision(run.tenant, run.scenario, run.revision)
            .await?;
        let adapter = self.engines.get(&run.engine).cloned();
        match (loaded, adapter) {
            (Some(revision), Some(adapter)) => Ok(Some(Claimed {
                run,
                revision,
                adapter,
            })),
            (revision, _) => {
                let message = if revision.is_none() {
                    "Run revision is missing"
                } else {
                    "Run engine is not available on this worker"
                };
                let failure = Completion::Failed(vec![Diagnostic::new("RUN_SETUP", message)]);
                self.store
                    .complete_run(&run, &self.id, failure, Utc::now())
                    .await?;
                Ok(None)
            }
        }
    }

    /// Extend the lease and report the phase. Returns `false` when the worker no
    /// longer owns the run or cancellation was requested.
    pub async fn heartbeat(&self, claimed: &Claimed, phase: &str) -> Result<bool> {
        let now = Utc::now();
        let current = self
            .store
            .heartbeat(&claimed.run, &self.id, phase, now, now + self.lease)
            .await?;
        Ok(current.is_some_and(|r| !r.cancel_requested))
    }

    pub async fn finish(&self, claimed: &Claimed, completion: Completion) -> Result<Run> {
        self.store
            .complete_run(&claimed.run, &self.id, completion, Utc::now())
            .await
    }

    /// Claim, compute and finish one run on the current thread. Intended for
    /// tests and single-process tools; servers compute off their executor.
    pub async fn run_once(&self) -> Result<Option<Run>> {
        let Some(claimed) = self.claim().await? else {
            return Ok(None);
        };
        let completion = compute(&claimed, &CancelToken::default(), &|_| {});
        self.finish(&claimed, completion).await.map(Some)
    }
}

/// Prepare, run and independently validate one claimed run. Only a valid
/// schedule becomes a result; anything else fails the run with diagnostics.
pub fn compute(claimed: &Claimed, cancel: &CancelToken, phase: &dyn Fn(&str)) -> Completion {
    let adapter = claimed.adapter.as_ref();
    let content = &claimed.revision.content;
    let descriptor = adapter.descriptor();
    let unsupported = unsupported_declarations(&descriptor, content);
    if !unsupported.is_empty() {
        return Completion::Failed(unsupported);
    }
    phase("preparing");
    let prepared = match adapter.prepare(content) {
        Ok(p) => p,
        Err(d) => return Completion::Failed(d),
    };
    if cancel.is_cancelled() {
        return Completion::Cancelled;
    }
    phase("computing");
    let output = match adapter.run(&prepared, &claimed.run.options, cancel) {
        Ok(o) => o,
        Err(d) => return Completion::Failed(d),
    };
    if cancel.is_cancelled() {
        return Completion::Cancelled;
    }
    phase("validating");
    let validation = adapter.validate(&prepared, &output.schedule);
    if !validation.valid {
        let mut diagnostics = vec![Diagnostic::new(
            "VALIDATION_FAILED",
            format!("{} rejected the engine's schedule", validation.validator),
        )];
        diagnostics.extend(validation.diagnostics);
        return Completion::Failed(diagnostics);
    }
    let run = &claimed.run;
    Completion::Succeeded(Box::new(PlanResult {
        id: ResultId::new(),
        tenant: run.tenant,
        scenario: run.scenario,
        revision: run.revision,
        run: run.id,
        provenance: Provenance {
            engine: descriptor.engine,
            extensions: prepared.extensions,
            options: run.options.clone(),
            seed: output.seed,
            content_hash: claimed.revision.content_hash.clone(),
        },
        validation,
        metrics: output.metrics,
        view: output.view,
        schedule: output.schedule,
        status: ResultStatus::Proposed,
        decisions: Vec::new(),
        created_at: Utc::now(),
    }))
}
