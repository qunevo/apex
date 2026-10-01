//! Persistence contract. Tenant-scoped methods must never return another tenant's
//! records; state-changing methods enforce their preconditions atomically.
use crate::{error::Result, model::*};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Fairness and recovery limits for claiming queued runs.
#[derive(Clone, Copy, Debug)]
pub struct ClaimPolicy {
    pub max_running_per_tenant: u32,
    pub max_attempts: u32,
}

/// Outcome written by the worker that holds a run's lease.
#[derive(Clone, Debug)]
pub enum Completion {
    Succeeded(Box<PlanResult>),
    Failed(Vec<Diagnostic>),
    Cancelled,
}

#[async_trait]
pub trait Store: Send + Sync {
    async fn ensure_tenant(&self, tenant: &Tenant) -> Result<()>;

    async fn insert_scenario(&self, scenario: &Scenario, first: &Revision) -> Result<()>;
    async fn list_scenarios(&self, tenant: TenantId) -> Result<Vec<Scenario>>;
    async fn get_scenario(&self, tenant: TenantId, id: ScenarioId) -> Result<Option<Scenario>>;
    /// Append `revision` if the scenario's current revision equals `expected`.
    async fn append_revision(
        &self,
        tenant: TenantId,
        expected: u64,
        revision: &Revision,
    ) -> Result<Scenario>;
    async fn get_revision(
        &self,
        tenant: TenantId,
        id: ScenarioId,
        number: u64,
    ) -> Result<Option<Revision>>;

    async fn insert_run(&self, run: &Run) -> Result<()>;
    async fn get_run(&self, tenant: TenantId, id: RunId) -> Result<Option<Run>>;
    async fn list_runs(&self, tenant: TenantId, scenario: ScenarioId) -> Result<Vec<Run>>;
    /// Cancel a queued run immediately, or flag a running one for its worker.
    async fn request_cancel(&self, tenant: TenantId, id: RunId, now: DateTime<Utc>) -> Result<Run>;
    /// Claim the oldest eligible queued run, or a running run whose lease expired.
    /// This is the only cross-tenant operation and is reserved for workers.
    async fn claim_run(
        &self,
        worker: &str,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
        policy: ClaimPolicy,
    ) -> Result<Option<Run>>;
    /// Record progress and extend the lease. Returns the run while the worker
    /// still owns it, including whether cancellation was requested.
    async fn heartbeat(
        &self,
        run: &Run,
        worker: &str,
        phase: &str,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
    ) -> Result<Option<Run>>;
    /// Finish a run owned by `worker`, saving its result in the same transaction.
    async fn complete_run(
        &self,
        run: &Run,
        worker: &str,
        completion: Completion,
        now: DateTime<Utc>,
    ) -> Result<Run>;

    async fn get_result(&self, tenant: TenantId, id: ResultId) -> Result<Option<PlanResult>>;
    async fn list_results(
        &self,
        tenant: TenantId,
        scenario: ScenarioId,
    ) -> Result<Vec<ResultSummary>>;
    /// Approve or reject a result that is still proposed.
    async fn decide_result(
        &self,
        tenant: TenantId,
        id: ResultId,
        decision: &Decision,
    ) -> Result<PlanResult>;
    /// Publish an approved result if its revision is still the scenario's current
    /// revision. A previously published result becomes superseded.
    async fn publish_result(
        &self,
        tenant: TenantId,
        id: ResultId,
        decision: &Decision,
    ) -> Result<PlanResult>;
}
