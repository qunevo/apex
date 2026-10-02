//! Control-platform records. Engine-specific facts and schedules stay opaque JSON
//! tagged with the engine that understands them; the common schedule view does not.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt};
use uuid::Uuid;

macro_rules! id_type {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub Uuid);
        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }
        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
        impl std::str::FromStr for $name {
            type Err = uuid::Error;
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s).map(Self)
            }
        }
    };
}
id_type!(TenantId);
id_type!(ScenarioId);
id_type!(RunId);
id_type!(ResultId);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tenant {
    pub id: TenantId,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
}
impl Diagnostic {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            entity: None,
        }
    }
    pub fn at(mut self, entity: impl Into<String>) -> Self {
        self.entity = Some(entity.into());
        self
    }
}

/// A planner's declaration about the desired plan, kept separate from imported facts.
/// The engine adapter decides which kinds it can represent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Declaration {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub target: Value,
    #[serde(default)]
    pub parameters: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PlanningIntent {
    #[serde(default)]
    pub declarations: Vec<Declaration>,
}

/// Everything a run plans from: production facts plus planning intent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScenarioContent {
    /// Production facts in the scenario engine's model.
    pub facts: Value,
    #[serde(default)]
    pub intent: PlanningIntent,
    /// Server-selected package identity, pinned for every revision of this scenario.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customization_package: Option<crate::packages::Package>,
}
impl ScenarioContent {
    pub fn hash(&self) -> String {
        let bytes = serde_json::to_vec(self).expect("scenario content serializes");
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Scenario {
    pub id: ScenarioId,
    pub tenant: TenantId,
    pub name: String,
    /// Engine adapter that owns the facts model.
    pub engine: String,
    pub current_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub published_result: Option<ResultId>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An immutable snapshot. Edits append a new revision; runs keep their revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub scenario: ScenarioId,
    pub tenant: TenantId,
    pub number: u64,
    pub content: ScenarioContent,
    pub content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub author: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}
impl RunState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => return None,
        })
    }
}

/// A background optimization of one scenario revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub tenant: TenantId,
    pub scenario: ScenarioId,
    pub revision: u64,
    pub engine: String,
    /// Engine-specific run options, checked by the adapter when the run is started.
    pub options: Value,
    pub state: RunState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    pub cancel_requested: bool,
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worker: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_until: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<Diagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<ResultId>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComponentRef {
    pub id: String,
    pub version: String,
}

/// What produced a result, sufficient to reproduce and audit it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    pub engine: ComponentRef,
    #[serde(default)]
    pub extensions: Vec<ComponentRef>,
    pub options: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    pub content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub customization_package: Option<crate::packages::Package>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub valid: bool,
    /// Who checked the schedule, e.g. the engine's independent validator.
    pub validator: String,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

/// Engine-neutral schedule representation for Gantt charts and comparison.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ScheduleView {
    /// Optional absolute time origin; times are seconds relative to it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epoch: Option<String>,
    pub resources: Vec<ViewResource>,
    pub operations: Vec<ViewOperation>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewResource {
    pub id: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewOperation {
    pub id: String,
    pub resource: String,
    pub start: i64,
    pub end: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Proposed,
    Approved,
    Rejected,
    Published,
    /// Replaced by a later publication of the same scenario.
    Superseded,
}
impl ResultStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Proposed => "proposed",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Published => "published",
            Self::Superseded => "superseded",
        }
    }
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "proposed" => Self::Proposed,
            "approved" => Self::Approved,
            "rejected" => Self::Rejected,
            "published" => Self::Published,
            "superseded" => Self::Superseded,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Decision {
    pub status: ResultStatus,
    pub by: String,
    pub at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A validated plan for one scenario revision. It never changes its revision.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanResult {
    pub id: ResultId,
    pub tenant: TenantId,
    pub scenario: ScenarioId,
    pub revision: u64,
    pub run: RunId,
    pub provenance: Provenance,
    pub validation: ValidationReport,
    pub metrics: BTreeMap<String, f64>,
    pub view: ScheduleView,
    /// The engine's native schedule; interfaces may omit it from responses.
    #[serde(default)]
    pub schedule: Value,
    pub status: ResultStatus,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    pub created_at: DateTime<Utc>,
}

/// Compact result listing without the schedule payloads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResultSummary {
    pub id: ResultId,
    pub scenario: ScenarioId,
    pub revision: u64,
    pub run: RunId,
    pub status: ResultStatus,
    pub valid: bool,
    pub metrics: BTreeMap<String, f64>,
    pub created_at: DateTime<Utc>,
}
impl From<&PlanResult> for ResultSummary {
    fn from(r: &PlanResult) -> Self {
        Self {
            id: r.id,
            scenario: r.scenario,
            revision: r.revision,
            run: r.run,
            status: r.status,
            valid: r.validation.valid,
            metrics: r.metrics.clone(),
            created_at: r.created_at,
        }
    }
}
