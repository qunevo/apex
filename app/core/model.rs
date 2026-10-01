use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Time = i64;
fn one() -> f64 {
    1.0
}
fn version() -> String {
    "apex.v3.4".into()
}
fn default_stage() -> String {
    "default".into()
}

#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Problem {
    #[serde(default)]
    pub planning: crate::language::PlanningModel,
    #[serde(default = "version")]
    pub schema_version: String,
    pub id: String,
    pub horizon: Time,
    #[serde(default)]
    pub epoch: Option<String>,
    pub resources: Vec<Resource>,
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    #[serde(default)]
    pub locks: Vec<Lock>,
    #[serde(default)]
    pub transitions: Vec<Transition>,
    #[serde(default)]
    pub inventory: BTreeMap<String, f64>,
    #[serde(default)]
    pub receipts: Vec<Receipt>,
    /// Reallocate existing supply after selecting an active workplan for each evaluation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_policy: Option<crate::material::MaterialPolicy>,
    #[serde(skip)]
    #[schemars(skip)]
    pub material_report: Option<crate::material::DispatchReport>,
    #[serde(default)]
    pub objectives: Vec<Objective>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub routes: Vec<RouteChoice>,
    #[serde(default)]
    pub jobs: Vec<Job>,
    #[serde(default)]
    pub orders: Vec<Order>,
    #[serde(default)]
    pub customization: Option<CustomizationRef>,
    #[serde(default)]
    pub queue_policy: Option<QueuePolicy>,
    #[serde(default)]
    pub queue_definitions: Vec<QueueDefinition>,
    #[serde(default)]
    pub freeze_zones: Vec<FreezeZone>,
    /// Populated only by linked native code, never accepted from input JSON.
    #[serde(skip)]
    #[schemars(skip)]
    pub native_objectives: Vec<Objective>,
    /// Advance stable names for conditionals emitted by a linked sequence hook.
    #[serde(skip)]
    #[schemars(skip)]
    pub native_conditionals: crate::conditionals::Catalog,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub start: Time,
    pub end: Time,
    #[serde(default = "one")]
    pub capacity: f64,
    #[serde(default = "one")]
    pub rate: f64,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub id: String,
    #[serde(default = "one")]
    pub capacity: f64,
    pub calendar: Vec<Window>,
    /// Empty means retention is allowed throughout the horizon at physical capacity.
    #[serde(default)]
    pub retention_calendar: Vec<Window>,
    #[serde(default)]
    pub initial_state: String,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Requirement {
    pub resource: String,
    #[serde(default = "one")]
    pub amount: f64,
    #[serde(default)]
    pub retain: bool,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Interrupt {
    #[default]
    NonInterruptible,
    CalendarResumable,
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub id: String,
    /// None remains a readiness error, never zero work.
    pub work: Option<f64>,
    #[serde(default)]
    pub work_per_unit: Option<f64>,
    #[serde(default)]
    pub interruption: Interrupt,
    #[serde(default)]
    pub rate_resource: Option<String>,
    pub requirements: Vec<Requirement>,
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mode {
    pub id: String,
    pub primary: String,
    pub phases: Vec<Phase>,
    #[serde(default)]
    pub contiguous: bool,
    #[serde(default)]
    pub cost: f64,
    #[serde(default)]
    pub pre: Vec<Conditional>,
    #[serde(default)]
    pub post: Vec<Conditional>,
    #[serde(default)]
    pub consume: Option<BTreeMap<String, f64>>,
    #[serde(default)]
    pub produce: Option<BTreeMap<String, f64>>,
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Conditional {
    pub id: String,
    pub modes: Vec<Mode>,
    #[serde(default)]
    pub releases_product: bool,
    /// None retains legacy serial order. Some([]) starts an independent branch.
    #[serde(default)]
    pub after: Option<Vec<ActivityDependency>>,
    #[serde(default)]
    pub owner_resource: Option<String>,
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub id: String,
    #[serde(default)]
    pub release: Time,
    #[serde(default)]
    pub due: Option<Time>,
    #[serde(default)]
    pub deadline: Option<Time>,
    #[serde(default = "one")]
    pub priority: f64,
    #[serde(default)]
    pub family: String,
    pub modes: Vec<Mode>,
    #[serde(default)]
    pub pre: Vec<Conditional>,
    #[serde(default)]
    pub post: Vec<Conditional>,
    #[serde(default)]
    pub consume: BTreeMap<String, f64>,
    #[serde(default)]
    pub produce: BTreeMap<String, f64>,
    #[serde(default)]
    pub attributes: BTreeMap<String, f64>,
    #[serde(default)]
    pub execution: Option<Execution>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default = "one")]
    pub quantity: f64,
    #[serde(default)]
    pub job: Option<String>,
    #[serde(default = "default_stage")]
    pub stage: String,
    /// Explicit conditional mode commitments, e.g. "pre:setup" -> "crew-b".
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub conditional_modes: BTreeMap<String, String>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    pub mode: String,
    pub as_of: Time,
    /// Actual main activity segments and reservations; retained as immutable facts.
    pub actual: Activity,
    /// One entry per phase. Explicit zero marks completed phases/preparation.
    pub remaining_work: BTreeMap<String, f64>,
    #[serde(default)]
    pub restart: Vec<Conditional>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dependency {
    pub before: String,
    pub after: String,
    #[serde(default)]
    pub min_lag: Time,
    #[serde(default)]
    pub max_lag: Option<Time>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Lock {
    Mode {
        task: String,
        mode: String,
    },
    Resource {
        task: String,
        resource: String,
    },
    Start {
        task: String,
        at: Time,
    },
    Order {
        resource: String,
        tasks: Vec<String>,
        #[serde(default)]
        consecutive: bool,
    },
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transition {
    pub id: String,
    pub resource: String,
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub previous_post: Vec<Conditional>,
    #[serde(default)]
    pub next_pre: Vec<Conditional>,
    #[serde(default)]
    pub penalty: f64,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub item: String,
    pub at: Time,
    pub amount: f64,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Objective {
    pub metric: String,
    #[serde(default = "one")]
    pub weight: f64,
    #[serde(default)]
    pub priority: u32,
    #[serde(default)]
    pub maximize: bool,
    #[serde(default = "one")]
    pub scale: f64,
}
impl Default for Objective {
    fn default() -> Self {
        Self {
            metric: String::new(),
            weight: 1.0,
            priority: 0,
            maximize: false,
            scale: 1.0,
        }
    }
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rule {
    /// Match a suffix of families on a primary-resource sequence, including current work.
    SequencePattern {
        id: String,
        resource: String,
        pattern: Vec<String>,
        #[serde(default)]
        previous_post: Vec<Conditional>,
        #[serde(default)]
        next_pre: Vec<Conditional>,
        #[serde(default)]
        penalty: f64,
    },
    SetupCharge {
        id: String,
        task: String,
        value: f64,
    },
    TaskWindow {
        id: String,
        task: String,
        earliest: Time,
        latest: Time,
    },
    AttributeObjective {
        id: String,
        attribute: String,
        #[serde(default = "one")]
        weight: f64,
        #[serde(default)]
        priority: u32,
    },
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub phase: String,
    pub start: Time,
    pub end: Time,
    pub work: f64,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Reservation {
    pub resource: String,
    pub start: Time,
    pub end: Time,
    pub amount: f64,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub id: String,
    pub task: String,
    pub role: String,
    pub mode: String,
    pub start: Time,
    pub end: Time,
    pub segments: Vec<Segment>,
    pub reservations: Vec<Reservation>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub task: String,
    pub mode: String,
    pub primary: String,
    pub start: Time,
    pub end: Time,
    pub ready: Time,
    pub activities: Vec<Activity>,
    /// Occupancy carried between observed execution and resumed work.
    #[serde(default)]
    pub retained: Vec<Reservation>,
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Schedule {
    /// Actual decoder decision order (assignments themselves retain input order).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub construction: Vec<Decision>,
    #[serde(default)]
    pub metric_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material_report: Option<crate::material::DispatchReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dispatch: Option<crate::policy::Evidence>,
    pub problem_id: String,
    pub assignments: Vec<Assignment>,
    pub metrics: BTreeMap<String, f64>,
    pub score: Vec<f64>,
    pub elapsed_ms: f64,
    pub evaluations: usize,
    pub strategy: String,
    pub seed: u64,
    pub dispatch_weights: Option<HeuristicWeights>,
    #[serde(default)]
    pub route_choices: BTreeMap<String, String>,
    #[serde(default)]
    pub job_completions: BTreeMap<String, Time>,
    #[serde(default)]
    pub order_completions: BTreeMap<String, Time>,
    #[serde(default)]
    pub replay: Option<Box<Options>>,
    #[serde(default)]
    pub search: Option<SearchReport>,
    #[serde(default)]
    pub objective_values: Vec<f64>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub entity: String,
    pub message: String,
    pub source: Option<String>,
}
impl Diagnostic {
    pub fn new(code: &str, entity: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            entity: entity.into(),
            message: message.into(),
            source: None,
        }
    }
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Validation {
    pub valid: bool,
    pub diagnostics: Vec<Diagnostic>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub strategy: String,
    pub iterations: usize,
    pub seed: u64,
    pub budget_ms: u64,
    pub weights: Option<HeuristicWeights>,
    pub route_choices: BTreeMap<String, String>,
    pub mode_choices: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub conditional_choices: BTreeMap<String, BTreeMap<String, String>>,
    pub queue_policy: Option<QueuePolicy>,
    pub decision_prefix: Vec<Decision>,
    /// Soft operation priority permutation. Readiness and mandatory policies still apply.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decision_order: Vec<String>,
    pub top_k: usize,
    /// XH hypersearch configuration; shared population/worker defaults also serve XE.
    #[serde(alias = "trainer")]
    pub xh: XhConfig,
    /// XT tree-search configuration.
    #[serde(alias = "plus")]
    pub xt: XtConfig,
    pub improve: ImproveConfig,
    /// XE direct schedule evolution configuration.
    #[serde(alias = "evolution")]
    pub xe: crate::xe::Config,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeuristicWeights {
    pub due: f64,
    pub work: f64,
    pub priority: f64,
    pub release: f64,
    pub urgency: f64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            strategy: "due".into(),
            iterations: 128,
            seed: 42,
            budget_ms: 5000,
            weights: None,
            route_choices: BTreeMap::new(),
            mode_choices: BTreeMap::new(),
            conditional_choices: BTreeMap::new(),
            queue_policy: None,
            decision_prefix: vec![],
            decision_order: vec![],
            top_k: 1,
            xh: XhConfig::default(),
            xt: XtConfig::default(),
            improve: ImproveConfig::default(),
            xe: crate::xe::Config::default(),
        }
    }
}

#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct QueuePolicy {
    pub normalization: String,
    pub stages: BTreeMap<String, BTreeMap<String, f64>>,
    /// Zero evaluates the entire ready set; a positive value bounds the candidate window.
    pub candidate_limit: usize,
}
impl Default for QueuePolicy {
    fn default() -> Self {
        Self {
            normalization: "minmax".into(),
            stages: BTreeMap::new(),
            candidate_limit: 64,
        }
    }
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueDefinition {
    pub id: String,
    pub metric: String,
    pub attribute: String,
    #[serde(default = "one")]
    pub weight: f64,
    #[serde(default)]
    pub prefer_high: bool,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    pub task: String,
    pub mode: String,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct XhConfig {
    pub population_size: usize,
    pub generations: Option<usize>,
    pub max_evaluations: Option<usize>,
    pub workers: usize,
    pub mutation_rate: f64,
    pub crossover_rate: f64,
    pub selection: String,
    pub archive_limit: usize,
}
impl Default for XhConfig {
    fn default() -> Self {
        Self {
            population_size: 16,
            generations: None,
            max_evaluations: None,
            workers: 1,
            mutation_rate: 0.75,
            crossover_rate: 0.25,
            selection: "lexicographic".into(),
            archive_limit: 32,
        }
    }
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct XtConfig {
    pub branching: usize,
    pub depth: usize,
    pub workers: usize,
    pub exploration: f64,
}
impl Default for XtConfig {
    fn default() -> Self {
        Self {
            branching: 4,
            depth: 8,
            workers: 1,
            exploration: 1.4,
        }
    }
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
pub struct SearchCandidate {
    pub metrics: BTreeMap<String, f64>,
    pub score: Vec<f64>,
    pub objectives: Vec<f64>,
    pub options: Options,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImproveConfig {
    /// Fraction of the shared evaluation/time allowance assigned to XH.
    #[serde(alias = "trainer_share")]
    pub xh_share: f64,
    /// Reserve part of the same total budget for XE.
    #[serde(alias = "evolution_share")]
    pub xe_share: f64,
    /// Maximum number of distinct validated strategies handed to XT.
    pub policies: usize,
}
impl Default for ImproveConfig {
    fn default() -> Self {
        Self {
            xh_share: 0.4,
            // Equal-budget ablations did not justify diverting XT budget by default.
            xe_share: 0.0,
            policies: 3,
        }
    }
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
pub struct SearchProgress {
    pub evaluations: usize,
    pub elapsed_ms: f64,
    pub score: Vec<f64>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
pub struct SearchPhase {
    pub name: String,
    pub evaluations: usize,
    pub failed_evaluations: usize,
    pub elapsed_ms: f64,
    pub stop_reason: String,
    pub queue_policy: Option<QueuePolicy>,
    pub route_choices: BTreeMap<String, String>,
    pub best_score: Option<Vec<f64>>,
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
pub struct SearchReport {
    pub algorithm: String,
    pub generations: usize,
    pub evaluations: usize,
    pub failed_evaluations: usize,
    pub workers: usize,
    pub mutations: usize,
    pub crossovers: usize,
    pub stop_reason: String,
    pub nodes: usize,
    pub revisits: usize,
    pub archive: Vec<SearchCandidate>,
    pub unmapped_objectives: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phases: Vec<SearchPhase>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub progress: Vec<SearchProgress>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub operators: Vec<crate::xe::OperatorReport>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FreezeZone {
    pub resource: String,
    pub until: Time,
    pub dimensions: Vec<String>,
    pub tasks: Vec<String>,
    pub baseline_schedule: String,
}

#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActivityDependency {
    pub before: String,
    #[serde(default)]
    pub min_lag: Time,
    #[serde(default)]
    pub max_lag: Option<Time>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteChoice {
    pub id: String,
    pub alternatives: Vec<RouteAlternative>,
    #[serde(default)]
    pub selected: Option<String>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteAlternative {
    pub id: String,
    pub tasks: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub id: String,
    pub item: String,
    pub quantity: f64,
    #[serde(default)]
    pub due: Option<Time>,
    #[serde(default)]
    pub deadline: Option<Time>,
    #[serde(default = "one")]
    pub priority: f64,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Order {
    pub id: String,
    pub jobs: Vec<String>,
    #[serde(default)]
    pub due: Option<Time>,
    #[serde(default)]
    pub deadline: Option<Time>,
    #[serde(default = "one")]
    pub priority: f64,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CustomizationRef {
    pub id: String,
    pub version: String,
}
