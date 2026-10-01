//! In-memory store for tests, demos and single-process use. It follows the same
//! tenant scoping and atomic preconditions as durable stores.
use crate::{
    error::{Error, Result},
    model::*,
    store::{ClaimPolicy, Completion, Store},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
};

#[derive(Default)]
struct State {
    tenants: HashMap<TenantId, Tenant>,
    scenarios: HashMap<ScenarioId, Scenario>,
    revisions: BTreeMap<(ScenarioId, u64), Revision>,
    runs: HashMap<RunId, Run>,
    results: HashMap<ResultId, PlanResult>,
}

#[derive(Default)]
pub struct MemoryStore(Mutex<State>);

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
    fn with<T>(&self, f: impl FnOnce(&mut State) -> Result<T>) -> Result<T> {
        let mut state = self
            .0
            .lock()
            .map_err(|_| Error::Store("memory store poisoned".into()))?;
        f(&mut state)
    }
}

fn missing(kind: &str, id: impl std::fmt::Display) -> Error {
    Error::NotFound(format!("{kind} {id}"))
}

fn owned_run<'a>(state: &'a mut State, run: &Run, worker: &str) -> Result<&'a mut Run> {
    state
        .runs
        .get_mut(&run.id)
        .filter(|r| r.state == RunState::Running && r.worker.as_deref() == Some(worker))
        .ok_or_else(|| Error::Conflict(format!("Worker {worker} no longer owns run {}", run.id)))
}

#[async_trait]
impl Store for MemoryStore {
    async fn ensure_tenant(&self, tenant: &Tenant) -> Result<()> {
        self.with(|s| {
            s.tenants.entry(tenant.id).or_insert_with(|| tenant.clone());
            Ok(())
        })
    }

    async fn insert_scenario(&self, scenario: &Scenario, first: &Revision) -> Result<()> {
        self.with(|s| {
            if !s.tenants.contains_key(&scenario.tenant) {
                return Err(missing("tenant", scenario.tenant));
            }
            s.scenarios.insert(scenario.id, scenario.clone());
            s.revisions
                .insert((scenario.id, first.number), first.clone());
            Ok(())
        })
    }

    async fn list_scenarios(&self, tenant: TenantId) -> Result<Vec<Scenario>> {
        self.with(|s| {
            let mut list: Vec<_> = s
                .scenarios
                .values()
                .filter(|x| x.tenant == tenant)
                .cloned()
                .collect();
            list.sort_by_key(|x| x.created_at);
            Ok(list)
        })
    }

    async fn get_scenario(&self, tenant: TenantId, id: ScenarioId) -> Result<Option<Scenario>> {
        self.with(|s| Ok(s.scenarios.get(&id).filter(|x| x.tenant == tenant).cloned()))
    }

    async fn append_revision(
        &self,
        tenant: TenantId,
        expected: u64,
        revision: &Revision,
    ) -> Result<Scenario> {
        self.with(|s| {
            let scenario = s
                .scenarios
                .get_mut(&revision.scenario)
                .filter(|x| x.tenant == tenant)
                .ok_or_else(|| missing("scenario", revision.scenario))?;
            if scenario.current_revision != expected || revision.number != expected + 1 {
                return Err(Error::Conflict(format!(
                    "Expected revision {expected}, current revision is {}",
                    scenario.current_revision
                )));
            }
            scenario.current_revision = revision.number;
            scenario.updated_at = revision.created_at;
            let updated = scenario.clone();
            s.revisions
                .insert((revision.scenario, revision.number), revision.clone());
            Ok(updated)
        })
    }

    async fn get_revision(
        &self,
        tenant: TenantId,
        id: ScenarioId,
        number: u64,
    ) -> Result<Option<Revision>> {
        self.with(|s| {
            Ok(s.revisions
                .get(&(id, number))
                .filter(|x| x.tenant == tenant)
                .cloned())
        })
    }

    async fn insert_run(&self, run: &Run) -> Result<()> {
        self.with(|s| {
            s.runs.insert(run.id, run.clone());
            Ok(())
        })
    }

    async fn get_run(&self, tenant: TenantId, id: RunId) -> Result<Option<Run>> {
        self.with(|s| Ok(s.runs.get(&id).filter(|x| x.tenant == tenant).cloned()))
    }

    async fn list_runs(&self, tenant: TenantId, scenario: ScenarioId) -> Result<Vec<Run>> {
        self.with(|s| {
            let mut list: Vec<_> = s
                .runs
                .values()
                .filter(|x| x.tenant == tenant && x.scenario == scenario)
                .cloned()
                .collect();
            list.sort_by_key(|x| x.created_at);
            Ok(list)
        })
    }

    async fn request_cancel(&self, tenant: TenantId, id: RunId, now: DateTime<Utc>) -> Result<Run> {
        self.with(|s| {
            let run = s
                .runs
                .get_mut(&id)
                .filter(|x| x.tenant == tenant)
                .ok_or_else(|| missing("run", id))?;
            match run.state {
                RunState::Queued => run.state = RunState::Cancelled,
                RunState::Running => run.cancel_requested = true,
                _ => return Err(Error::Conflict(format!("Run {id} has already finished"))),
            }
            run.updated_at = now;
            Ok(run.clone())
        })
    }

    async fn claim_run(
        &self,
        worker: &str,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
        policy: ClaimPolicy,
    ) -> Result<Option<Run>> {
        self.with(|s| {
            let expired =
                |r: &Run| r.state == RunState::Running && r.lease_until.is_some_and(|l| l < now);
            // Runs with an expired lease were abandoned and are retried or failed.
            let mut busy: HashMap<TenantId, u32> = HashMap::new();
            for r in s.runs.values() {
                if r.state == RunState::Running && !expired(r) {
                    *busy.entry(r.tenant).or_default() += 1;
                }
            }
            let mut candidates: Vec<_> = s
                .runs
                .values()
                .filter(|r| r.state == RunState::Queued || expired(r))
                .filter(|r| {
                    busy.get(&r.tenant).copied().unwrap_or(0) < policy.max_running_per_tenant
                })
                .map(|r| (r.created_at, r.id))
                .collect();
            candidates.sort();
            for (_, id) in candidates {
                let run = s.runs.get_mut(&id).unwrap();
                if run.cancel_requested {
                    run.state = RunState::Cancelled;
                    run.worker = None;
                    run.lease_until = None;
                    run.updated_at = now;
                    continue;
                }
                if run.attempts >= policy.max_attempts {
                    run.state = RunState::Failed;
                    run.worker = None;
                    run.lease_until = None;
                    run.diagnostics = vec![Diagnostic::new(
                        "ATTEMPTS_EXHAUSTED",
                        "The run was abandoned by its workers too often",
                    )];
                    run.updated_at = now;
                    continue;
                }
                run.state = RunState::Running;
                run.attempts += 1;
                run.worker = Some(worker.into());
                run.lease_until = Some(lease_until);
                run.phase = Some("claimed".into());
                run.updated_at = now;
                return Ok(Some(run.clone()));
            }
            Ok(None)
        })
    }

    async fn heartbeat(
        &self,
        run: &Run,
        worker: &str,
        phase: &str,
        now: DateTime<Utc>,
        lease_until: DateTime<Utc>,
    ) -> Result<Option<Run>> {
        self.with(|s| {
            let Ok(current) = owned_run(s, run, worker) else {
                return Ok(None);
            };
            current.phase = Some(phase.into());
            current.lease_until = Some(lease_until);
            current.updated_at = now;
            Ok(Some(current.clone()))
        })
    }

    async fn complete_run(
        &self,
        run: &Run,
        worker: &str,
        completion: Completion,
        now: DateTime<Utc>,
    ) -> Result<Run> {
        self.with(|s| {
            let current = owned_run(s, run, worker)?;
            current.worker = None;
            current.lease_until = None;
            current.phase = None;
            current.updated_at = now;
            let result = match completion {
                Completion::Succeeded(result) => {
                    current.state = RunState::Succeeded;
                    current.result = Some(result.id);
                    Some(*result)
                }
                Completion::Failed(diagnostics) => {
                    current.state = RunState::Failed;
                    current.diagnostics = diagnostics;
                    None
                }
                Completion::Cancelled => {
                    current.state = RunState::Cancelled;
                    None
                }
            };
            let finished = current.clone();
            if let Some(result) = result {
                s.results.insert(result.id, result);
            }
            Ok(finished)
        })
    }

    async fn get_result(&self, tenant: TenantId, id: ResultId) -> Result<Option<PlanResult>> {
        self.with(|s| Ok(s.results.get(&id).filter(|x| x.tenant == tenant).cloned()))
    }

    async fn list_results(
        &self,
        tenant: TenantId,
        scenario: ScenarioId,
    ) -> Result<Vec<ResultSummary>> {
        self.with(|s| {
            let mut list: Vec<_> = s
                .results
                .values()
                .filter(|x| x.tenant == tenant && x.scenario == scenario)
                .map(ResultSummary::from)
                .collect();
            list.sort_by_key(|x| x.created_at);
            Ok(list)
        })
    }

    async fn decide_result(
        &self,
        tenant: TenantId,
        id: ResultId,
        decision: &Decision,
    ) -> Result<PlanResult> {
        self.with(|s| {
            let result = s
                .results
                .get_mut(&id)
                .filter(|x| x.tenant == tenant)
                .ok_or_else(|| missing("result", id))?;
            if result.status != ResultStatus::Proposed {
                return Err(Error::Conflict(format!(
                    "Result {id} is {}, not proposed",
                    result.status.as_str()
                )));
            }
            result.status = decision.status;
            result.decisions.push(decision.clone());
            Ok(result.clone())
        })
    }

    async fn publish_result(
        &self,
        tenant: TenantId,
        id: ResultId,
        decision: &Decision,
    ) -> Result<PlanResult> {
        self.with(|s| {
            let result = s
                .results
                .get(&id)
                .filter(|x| x.tenant == tenant)
                .ok_or_else(|| missing("result", id))?;
            if result.status != ResultStatus::Approved {
                return Err(Error::Conflict(format!(
                    "Result {id} is {}; only approved results can be published",
                    result.status.as_str()
                )));
            }
            let scenario = s
                .scenarios
                .get(&result.scenario)
                .ok_or_else(|| missing("scenario", result.scenario))?;
            if scenario.current_revision != result.revision {
                return Err(Error::Conflict(format!(
                    "Result {id} was planned for revision {}, but the scenario is at revision {}",
                    result.revision, scenario.current_revision
                )));
            }
            let (scenario_id, previous) = (scenario.id, scenario.published_result);
            if let Some(old) = previous.and_then(|p| s.results.get_mut(&p)) {
                old.status = ResultStatus::Superseded;
            }
            let scenario = s.scenarios.get_mut(&scenario_id).unwrap();
            scenario.published_result = Some(id);
            scenario.updated_at = decision.at;
            let result = s.results.get_mut(&id).unwrap();
            result.status = ResultStatus::Published;
            result.decisions.push(decision.clone());
            Ok(result.clone())
        })
    }
}
