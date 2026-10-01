//! Typed application operations shared by every interface. Each checks the actor's
//! permission, scopes access to the actor's tenant and applies workflow rules.
use crate::{
    auth::{Actor, Permission},
    engine::{EngineAdapter, EngineDescriptor, EngineRegistry, unsupported_declarations},
    error::{Error, Result},
    model::*,
    store::Store,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateScenario {
    pub name: String,
    pub engine: String,
    pub content: ScenarioContent,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub customization: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviseScenario {
    pub expected_revision: u64,
    pub content: ScenarioContent,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StartRun {
    /// Defaults to the scenario's current revision.
    #[serde(default)]
    pub revision: Option<u64>,
    #[serde(default)]
    pub options: Value,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DecideResult {
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone)]
pub struct Control {
    store: Arc<dyn Store>,
    engines: EngineRegistry,
    packages: Option<crate::packages::LoadedConfig>,
}

fn not_found(kind: &str, id: impl std::fmt::Display) -> Error {
    Error::NotFound(format!("{kind} {id}"))
}

impl Control {
    pub fn new(store: Arc<dyn Store>, engines: EngineRegistry) -> Self {
        Self {
            store,
            engines,
            packages: None,
        }
    }
    pub fn with_packages(mut self, packages: crate::packages::LoadedConfig) -> Self {
        self.packages = Some(packages);
        self
    }
    pub fn packages(&self) -> Option<&crate::packages::LoadedConfig> {
        self.packages.as_ref()
    }
    fn check_package(&self, content: &ScenarioContent) -> Result<()> {
        match &self.packages {
            Some(config) => config
                .check(content.customization_package.as_ref())
                .map_err(Error::invalid),
            None if content.customization_package.is_some() => Err(Error::invalid(
                "This server has no customization configuration",
            )),
            None => Ok(()),
        }
    }
    pub fn store(&self) -> &Arc<dyn Store> {
        &self.store
    }
    pub fn engine(&self, id: &str) -> Result<&Arc<dyn EngineAdapter>> {
        self.engines
            .get(id)
            .ok_or_else(|| Error::invalid(format!("Unknown engine {id}")))
    }

    pub fn engines(&self, actor: &Actor) -> Result<Vec<EngineDescriptor>> {
        actor.require(Permission::Read)?;
        Ok(self.engines.descriptors())
    }

    fn check_content(&self, engine: &str, content: &ScenarioContent) -> Result<()> {
        let adapter = self.engine(engine)?;
        let mut diagnostics = adapter
            .check_facts(&content.facts)
            .err()
            .unwrap_or_default();
        diagnostics.extend(unsupported_declarations(&adapter.descriptor(), content));
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(Error::Invalid {
                message: "Scenario content is not supported by its engine".into(),
                diagnostics,
            })
        }
    }

    pub async fn create_scenario(
        &self,
        actor: &Actor,
        mut request: CreateScenario,
    ) -> Result<(Scenario, Revision)> {
        actor.require(Permission::EditScenario)?;
        if request.content.customization_package.is_some() {
            return Err(Error::invalid(
                "Select customization by ID; the server assigns its package version",
            ));
        }
        request.content.customization_package = match &self.packages {
            Some(config) => Some(
                config
                    .select(request.customization.as_deref())
                    .map_err(Error::invalid)?,
            ),
            None if request.customization.is_some() => {
                return Err(Error::invalid(
                    "This server has no customization configuration",
                ));
            }
            None => None,
        };
        if request.name.trim().is_empty() {
            return Err(Error::invalid("Scenario name must not be empty"));
        }
        self.check_content(&request.engine, &request.content)?;
        let now = Utc::now();
        let scenario = Scenario {
            id: ScenarioId::new(),
            tenant: actor.tenant,
            name: request.name,
            engine: request.engine,
            current_revision: 1,
            published_result: None,
            created_by: actor.id.clone(),
            created_at: now,
            updated_at: now,
        };
        let revision = Revision {
            scenario: scenario.id,
            tenant: actor.tenant,
            number: 1,
            content_hash: request.content.hash(),
            content: request.content,
            note: request.note,
            author: actor.id.clone(),
            created_at: now,
        };
        self.store.insert_scenario(&scenario, &revision).await?;
        Ok((scenario, revision))
    }

    pub async fn list_scenarios(&self, actor: &Actor) -> Result<Vec<Scenario>> {
        actor.require(Permission::Read)?;
        self.store.list_scenarios(actor.tenant).await
    }

    pub async fn get_scenario(&self, actor: &Actor, id: ScenarioId) -> Result<Scenario> {
        actor.require(Permission::Read)?;
        self.store
            .get_scenario(actor.tenant, id)
            .await?
            .ok_or_else(|| not_found("scenario", id))
    }

    pub async fn revise_scenario(
        &self,
        actor: &Actor,
        id: ScenarioId,
        mut request: ReviseScenario,
    ) -> Result<Revision> {
        actor.require(Permission::EditScenario)?;
        let scenario = self.get_scenario(actor, id).await?;
        let previous = self
            .get_revision(actor, id, scenario.current_revision)
            .await?;
        self.check_package(&previous.content)?;
        if request.content.customization_package.is_some()
            && request.content.customization_package != previous.content.customization_package
        {
            return Err(Error::invalid(
                "A scenario's customization package cannot change; create a new scenario",
            ));
        }
        request.content.customization_package = previous.content.customization_package;
        self.check_content(&scenario.engine, &request.content)?;
        let revision = Revision {
            scenario: id,
            tenant: actor.tenant,
            number: request.expected_revision + 1,
            content_hash: request.content.hash(),
            content: request.content,
            note: request.note,
            author: actor.id.clone(),
            created_at: Utc::now(),
        };
        self.store
            .append_revision(actor.tenant, request.expected_revision, &revision)
            .await?;
        Ok(revision)
    }

    pub async fn get_revision(
        &self,
        actor: &Actor,
        id: ScenarioId,
        number: u64,
    ) -> Result<Revision> {
        actor.require(Permission::Read)?;
        self.store
            .get_revision(actor.tenant, id, number)
            .await?
            .ok_or_else(|| not_found("revision", format!("{id}@{number}")))
    }

    pub async fn start_run(&self, actor: &Actor, id: ScenarioId, request: StartRun) -> Result<Run> {
        actor.require(Permission::Run)?;
        let scenario = self.get_scenario(actor, id).await?;
        let revision = request.revision.unwrap_or(scenario.current_revision);
        if revision == 0 || revision > scenario.current_revision {
            return Err(not_found("revision", format!("{id}@{revision}")));
        }
        let input = self.get_revision(actor, id, revision).await?;
        self.check_package(&input.content)?;
        self.engine(&scenario.engine)?
            .check_options(&request.options)
            .map_err(|diagnostics| Error::Invalid {
                message: "Run options are not supported by the engine".into(),
                diagnostics,
            })?;
        let now = Utc::now();
        let run = Run {
            id: RunId::new(),
            tenant: actor.tenant,
            scenario: id,
            revision,
            engine: scenario.engine,
            options: request.options,
            state: RunState::Queued,
            phase: None,
            cancel_requested: false,
            attempts: 0,
            worker: None,
            lease_until: None,
            diagnostics: Vec::new(),
            result: None,
            created_by: actor.id.clone(),
            created_at: now,
            updated_at: now,
        };
        self.store.insert_run(&run).await?;
        Ok(run)
    }

    pub async fn get_run(&self, actor: &Actor, id: RunId) -> Result<Run> {
        actor.require(Permission::Read)?;
        self.store
            .get_run(actor.tenant, id)
            .await?
            .ok_or_else(|| not_found("run", id))
    }

    pub async fn list_runs(&self, actor: &Actor, scenario: ScenarioId) -> Result<Vec<Run>> {
        self.get_scenario(actor, scenario).await?;
        self.store.list_runs(actor.tenant, scenario).await
    }

    pub async fn cancel_run(&self, actor: &Actor, id: RunId) -> Result<Run> {
        actor.require(Permission::Run)?;
        self.store
            .request_cancel(actor.tenant, id, Utc::now())
            .await
    }

    pub async fn get_result(&self, actor: &Actor, id: ResultId) -> Result<PlanResult> {
        actor.require(Permission::Read)?;
        self.store
            .get_result(actor.tenant, id)
            .await?
            .ok_or_else(|| not_found("result", id))
    }

    pub async fn list_results(
        &self,
        actor: &Actor,
        scenario: ScenarioId,
    ) -> Result<Vec<ResultSummary>> {
        self.get_scenario(actor, scenario).await?;
        self.store.list_results(actor.tenant, scenario).await
    }

    fn decision(actor: &Actor, status: ResultStatus, request: DecideResult) -> Decision {
        Decision {
            status,
            by: actor.id.clone(),
            at: Utc::now(),
            note: request.note,
        }
    }

    pub async fn approve_result(
        &self,
        actor: &Actor,
        id: ResultId,
        request: DecideResult,
    ) -> Result<PlanResult> {
        actor.require(Permission::Decide)?;
        let result = self.get_result(actor, id).await?;
        if !result.validation.valid {
            return Err(Error::Invalid {
                message: "Only an independently validated result can be approved".into(),
                diagnostics: result.validation.diagnostics,
            });
        }
        let decision = Self::decision(actor, ResultStatus::Approved, request);
        self.store.decide_result(actor.tenant, id, &decision).await
    }

    pub async fn reject_result(
        &self,
        actor: &Actor,
        id: ResultId,
        request: DecideResult,
    ) -> Result<PlanResult> {
        actor.require(Permission::Decide)?;
        let decision = Self::decision(actor, ResultStatus::Rejected, request);
        self.store.decide_result(actor.tenant, id, &decision).await
    }

    pub async fn publish_result(
        &self,
        actor: &Actor,
        id: ResultId,
        request: DecideResult,
    ) -> Result<PlanResult> {
        actor.require(Permission::Publish)?;
        let decision = Self::decision(actor, ResultStatus::Published, request);
        self.store.publish_result(actor.tenant, id, &decision).await
    }
}
