use crate::{compile, model::*, validate, xg};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

static COUNTER: AtomicU64 = AtomicU64::new(0);
pub type ToolResult = Result<Value, Value>;
fn error(code: &str, message: impl ToString) -> Value {
    json!({"code":code,"message":message.to_string()})
}
fn hash(value: &Value) -> String {
    Sha256::digest(serde_json::to_vec(value).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn import_hash_preserves_sha256_hex_encoding() {
    assert_eq!(
        hash(&Value::Null),
        "74234e98afe7498fb5daf1f36ac2d78acc339464f950703b8c019892f982b90b"
    );
}

fn id() -> String {
    format!(
        "a{:x}-{:x}-{:x}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )
}
fn field<'a>(v: &'a Value, k: &str) -> Result<&'a str, Value> {
    v[k].as_str()
        .ok_or_else(|| error("ARGUMENT", format!("Missing string {k}")))
}
fn decode<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, Value> {
    serde_json::from_value(v).map_err(|e| error("SCHEMA", e))
}
fn diagnostics(ds: Vec<Diagnostic>) -> Value {
    json!({"code":"INVALID_CANDIDATE_OR_INPUT","total":ds.len(),"diagnostics":ds.iter().take(100).collect::<Vec<_>>(),"truncated":ds.len()>100})
}

#[derive(Serialize, Deserialize)]
struct Scenario {
    revision: u64,
    problem: Problem,
    parent: Option<String>,
}
#[derive(Serialize, Deserialize)]
struct SavedSchedule {
    scenario_id: String,
    revision: u64,
    problem: Problem,
    schedule: Schedule,
}
#[derive(Serialize, Deserialize)]
struct Import {
    problem: Problem,
    expected_tasks: usize,
    chunks: std::collections::BTreeMap<String, String>,
    finalized: Option<String>,
}

pub struct Service {
    pub root: PathBuf,
    pub workspace: PathBuf,
    pub viewer_url: String,
}
impl Service {
    pub fn new(
        root: impl AsRef<Path>,
        workspace: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        fs::create_dir_all(root.as_ref())?;
        Ok(Self {
            root: root.as_ref().canonicalize()?,
            workspace: workspace.as_ref().canonicalize()?,
            viewer_url: "http://127.0.0.1:8765".into(),
        })
    }
    fn path(&self, key: &str) -> Result<PathBuf, Value> {
        if key.is_empty()
            || key.len() > 120
            || !key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(error("ID", "Invalid artifact ID"));
        }
        Ok(self.root.join(format!("{key}.json")))
    }
    fn read<T: serde::de::DeserializeOwned>(&self, key: &str) -> Result<T, Value> {
        let bytes = fs::read(self.path(key)?).map_err(|e| error("NOT_FOUND", e))?;
        serde_json::from_slice(&bytes).map_err(|e| error("ARTIFACT", e))
    }
    fn write<T: Serialize>(&self, key: &str, value: &T) -> Result<(), Value> {
        let target = self.path(key)?;
        let temp = self.root.join(format!("{}.tmp", id()));
        let mut f =
            BufWriter::with_capacity(65536, File::create(&temp).map_err(|e| error("STORE", e))?);
        serde_json::to_writer(&mut f, value).map_err(|e| error("STORE", e))?;
        f.flush().map_err(|e| error("STORE", e))?;
        f.get_ref().sync_all().map_err(|e| error("STORE", e))?;
        fs::rename(temp, target).map_err(|e| error("STORE", e))
    }
    fn save(&self, p: Problem, parent: Option<String>) -> ToolResult {
        let key = id();
        compile::compile(&p).map_err(diagnostics)?;
        let tasks = p.tasks.len();
        self.write(
            &key,
            &Scenario {
                revision: 1,
                problem: p,
                parent,
            },
        )?;
        Ok(json!({"scenario_id":key,"revision":1,"tasks":tasks}))
    }
    fn input_file(&self, path: &str) -> Result<Value, Value> {
        let path = self
            .workspace
            .join(path)
            .canonicalize()
            .map_err(|e| error("FILE", e))?;
        if !path.starts_with(&self.workspace) {
            return Err(error(
                "PATH",
                "Input must be inside the configured workspace",
            ));
        }
        let f = File::open(path).map_err(|e| error("FILE", e))?;
        if f.metadata().map_err(|e| error("FILE", e))?.len() > 256 * 1024 * 1024 {
            return Err(error("SIZE", "Use chunk import above 256 MiB"));
        }
        serde_json::from_reader(BufReader::with_capacity(65536, f)).map_err(|e| error("JSON", e))
    }
    pub fn call(&self, name: &str, args: &Value) -> ToolResult {
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join("store.lock"))
            .map_err(|e| error("STORE", e))?;
        fs2::FileExt::lock_exclusive(&lock).map_err(|e| error("STORE", e))?;
        self.dispatch(name, args)
    }

    /// Keep unusually large records or metadata out of the language-model context.
    pub fn agent_response(&self, result: ToolResult) -> (Value, bool) {
        let (value, failed) = match result {
            Ok(v) => (v, false),
            Err(v) => (v, true),
        };
        let bytes = serde_json::to_vec(&value).unwrap().len();
        if bytes <= 65_536 {
            return (value, failed);
        }
        let key = id();
        if let Err(e) = self.write(&key, &value) {
            return (e, true);
        }
        (
            json!({"artifact_id":key,"artifact_path":self.path(&key).unwrap(),"bytes":bytes,"total":value.get("total"),"message":"Response retained as a local artifact because it exceeds 64 KiB. Request a smaller page or read selected artifact fields using a local script."}),
            failed,
        )
    }
    fn dispatch(&self, name: &str, args: &Value) -> ToolResult {
        match name {
            "material.prepare" => {
                let source = field(args, "scenario_id")?;
                let scenario: Scenario = self.read(source)?;
                if args["expected_revision"].as_u64() != Some(scenario.revision) {
                    return Err(error(
                        "REVISION",
                        "Expected revision differs from the source scenario",
                    ));
                }
                let options = decode(args.get("options").cloned().unwrap_or(json!({})))?;
                let (problem, report) =
                    crate::material::prepare(&scenario.problem, &options).map_err(diagnostics)?;
                let report_id = id();
                self.write(&report_id, &report)?;
                let mut result = self.save(problem, Some(source.into()))?;
                result["report_id"] = json!(report_id);
                result["allocations"] = json!(report.allocations.len());
                result["added_dependencies"] = json!(report.added_dependencies);
                result["source_revision"] = json!(scenario.revision);
                result["allocation_preview"] = json!(report.preview);
                result["preview_diagnostics"] = json!(report.preview_diagnostics);
                result["contract"] = json!(
                    "Existing supply only. Free routes remain searchable; preview allocations are provisional and each schedule stores its actual allocation report. Page model.page/materials for a saved schedule. Fully selected routes use a materialized snapshot."
                );
                Ok(result)
            }
            "policy.inspect" => {
                let scenario: Scenario = self.read(field(args, "scenario_id")?)?;
                Ok(
                    json!({"revision":scenario.revision,"planning":scenario.problem.planning,
                    "language":"apex.planning.v1","semantics":"Constraints lower to the physical model. Ordered policies filter the complete eligible ready pool before Q normalization. Exceptions never relax physical constraints.",
                    "exact_time":"Uses calendar placement or full prefix decoding, including prospective previous-post activities. Productive time counts main/actual segments only; occupied time includes primary retention and conditionals.",
                    "limits":{"retained_steps":256,"examples_per_reason":8,"decoded_prefixes_per_construction":100000},
                    "native_hooks":["has_dispatch_policy","filter_candidates","dispatch_needs_placements","supports_prefix_decoration"],
                    "extension_workflow":["Select a supported template and explicit scope, units, fallback and exceptions","Fork and patch planning; compare actual goals under equal search budgets","New semantics require a compiled Rust hook, semantic tests and a rebuild"],
                    "guarantee":"A construction policy is not a theorem about every feasible schedule or an optimality guarantee."}),
                )
            }
            "schedule.explain_decision" => {
                let saved: SavedSchedule = self.read(field(args, "schedule_id")?)?;
                let Some(e) = &saved.schedule.dispatch else {
                    return Ok(
                        json!({"enabled":false,"message":"This schedule used no mandatory dispatch policy"}),
                    );
                };
                let position = if let Some(task) = args["task"].as_str() {
                    e.decisions
                        .iter()
                        .position(|d| d.task == task)
                        .ok_or_else(|| error("UNKNOWN_TASK", "Task is absent from this schedule"))?
                } else {
                    args["position"].as_u64().unwrap_or(0) as usize
                };
                let selected = e.decisions.get(position).ok_or_else(|| {
                    error("DECISION_POSITION", "Position is outside this schedule")
                })?;
                Ok(
                    json!({"enabled":true,"position":position,"selected":selected,"step":e.steps.iter().find(|v|v.position==position),"trace_retained":position<e.steps.len(),"model_hash":e.model_hash,"exact_probes":e.exact_probes,"direct_placements":e.direct_placements,"contract":"Reasons describe construction decisions. Full witness is retained for policy replay; detailed explanations are bounded to the first 256 decisions."}),
                )
            }
            "queues.inspect" => {
                let scenario: Scenario = self.read(field(args, "scenario_id")?)?;
                let lowered = crate::language::lower(&scenario.problem).map_err(diagnostics)?;
                let p = &lowered;
                let extension = p
                    .customization
                    .as_ref()
                    .map(crate::extensions::registered)
                    .transpose()
                    .map_err(diagnostics)?;
                let definitions = crate::queues::definitions(p, extension.as_deref());
                let policy = p
                    .queue_policy
                    .clone()
                    .unwrap_or_else(|| crate::queues::default_policy(p, &definitions));
                let native_objectives = extension
                    .as_ref()
                    .map(|e| e.objectives(p))
                    .unwrap_or_default();
                let mapping:Vec<_>=crate::rules::objectives(p,&native_objectives).iter().map(|o|json!({"objective":o,"standard_proxies":crate::queues::objective_queues(&o.metric),"custom_proxies":definitions.iter().filter(|d|d.metric==o.metric).collect::<Vec<_>>() })).collect();
                Ok(
                    json!({"standard_queues":crate::queues::STANDARD,"definitions":definitions,"policy":policy,"mapping":mapping,"unmapped_objectives":crate::queues::unmapped(p,&definitions),"contract":"Q scores are normalized construction proxies. Full validated metrics determine search fitness; no universal correlation guarantee."}),
                )
            }
            "artifact.read" => {
                let value: Value = self.read(field(args, "artifact_id")?)?;
                let pointer = args["pointer"].as_str().unwrap_or("");
                let selected = value
                    .pointer(pointer)
                    .ok_or_else(|| error("POINTER", "Unknown JSON pointer"))?;
                Ok(match selected {
                    Value::Array(rows) => page(rows, args),
                    Value::Object(object) => {
                        let fields:Vec<_>=object.iter().map(|(key,value)|json!({"key":key,"type":match value {Value::Array(_)=>"array",Value::Object(_)=>"object",Value::String(_)=>"string",_=>"scalar"},"length":match value {Value::Array(v)=>Some(v.len()),Value::Object(v)=>Some(v.len()),Value::String(v)=>Some(v.len()),_=>None},"value":if value.is_number() || value.is_boolean() || value.is_null(){value.clone()}else{Value::Null}})).collect();
                        page(&fields, args)
                    }
                    Value::String(text) => {
                        let chars: Vec<_> = text.chars().collect();
                        let start =
                            (args["offset"].as_u64().unwrap_or(0) as usize).min(chars.len());
                        let end = (start + 4096).min(chars.len());
                        json!({"text":chars[start..end].iter().collect::<String>(),"total":chars.len(),"next_offset":if end<chars.len(){Some(end)}else{None}})
                    }
                    _ => json!({"value":selected}),
                })
            }
            "production.import" => {
                let value = if let Some(path) = args["path"].as_str() {
                    self.input_file(path)?
                } else {
                    args["production"].clone()
                };
                let input = decode(value)?;
                let p = crate::production::expand(input).map_err(diagnostics)?;
                self.save(p, None)
            }
            "model.page" => {
                let saved: Option<SavedSchedule> = args["schedule_id"]
                    .as_str()
                    .map(|id| self.read(id))
                    .transpose()?;
                if let (Some(saved), Some(id)) = (&saved, args["scenario_id"].as_str())
                    && saved.scenario_id != id
                {
                    return Err(error("SNAPSHOT", "Schedule belongs to another scenario"));
                }
                let p = if let Some(saved) = &saved {
                    saved.problem.clone()
                } else {
                    self.read::<Scenario>(field(args, "scenario_id")?)?.problem
                };
                let section = field(args, "section")?;
                let mut result = match section {
                    "metrics" => {
                        let mut rows = crate::metrics::catalog(&p);
                        if let Some(s) = &saved {
                            for id in s.schedule.metrics.keys() {
                                if !rows.iter().any(|r| r["id"] == *id) {
                                    rows.push(json!({"id":id,"unit":"extension_defined","standard_proxies":[],"proxy_status":"inspect_queues"}));
                                }
                            }
                        }
                        for row in &mut rows {
                            row["value"] = json!(saved.as_ref().and_then(|s| {
                                s.schedule.metrics.get(row["id"].as_str().unwrap())
                            }));
                        }
                        page(&rows, args)
                    }
                    "materials" => {
                        let saved = saved.as_ref().ok_or_else(|| {
                            error("ARGUMENT", "Provide schedule_id for actual allocations")
                        })?;
                        page(
                            saved
                                .schedule
                                .material_report
                                .as_ref()
                                .map(|r| r.allocations.as_slice())
                                .unwrap_or(&[]),
                            args,
                        )
                    }
                    "routes" => page(&p.routes, args),
                    "jobs" => page(&p.jobs, args),
                    "orders" => page(&p.orders, args),
                    "dependencies" => page(&p.dependencies, args),
                    "rules" => page(&p.rules, args),
                    "policies" => page(&p.planning.policies, args),
                    "constraints" => page(&p.planning.constraints, args),
                    "planning_objectives" => page(&p.planning.objectives, args),
                    "locks" => page(&p.locks, args),
                    "freeze_zones" => page(&p.freeze_zones, args),
                    _ => {
                        return Err(error(
                            "SECTION",
                            "Use routes, jobs, orders, dependencies, rules, locks, metrics or materials",
                        ));
                    }
                };
                if let Some(saved) = saved {
                    if let Some(items) = result["items"].as_array_mut() {
                        for item in items {
                            let id = item["id"].as_str().unwrap_or("").to_owned();
                            match section {
                                "routes" => {
                                    item["selected_in_schedule"] =
                                        json!(saved.schedule.route_choices.get(&id))
                                }
                                "jobs" => {
                                    item["completion"] =
                                        json!(saved.schedule.job_completions.get(&id))
                                }
                                "orders" => {
                                    item["completion"] =
                                        json!(saved.schedule.order_completions.get(&id))
                                }
                                _ => {}
                            }
                        }
                    }
                    result["schedule_revision"] = json!(saved.revision);
                }
                Ok(result)
            }
            "task.inspect" => {
                let saved: SavedSchedule = self.read(field(args, "schedule_id")?)?;
                let id = field(args, "task")?;
                let active: std::collections::HashSet<_> = saved
                    .schedule
                    .assignments
                    .iter()
                    .map(|a| a.task.as_str())
                    .collect();
                let task = saved
                    .problem
                    .tasks
                    .iter()
                    .find(|t| t.id == id)
                    .ok_or_else(|| error("TASK", id))?;
                let (resolved, _) = crate::domain::resolve(
                    &saved.problem,
                    &crate::conditionals::validation_options(&saved.schedule),
                )
                .map_err(diagnostics)?;
                let urgency = crate::urgency::derive(&resolved);
                let effective = resolved
                    .tasks
                    .iter()
                    .position(|t| t.id == id)
                    .map(|i| urgency[i]);
                let conditional_alternatives = crate::conditionals::catalog(&resolved).remove(id);
                Ok(
                    json!({"task":task,"dispatch_urgency":effective,"conditional_alternatives":conditional_alternatives,"assignment":saved.schedule.assignments.iter().find(|a|a.task==id),"dependencies":resolved.dependencies.iter().filter(|d|(d.before==id || d.after==id) && active.contains(d.before.as_str()) && active.contains(d.after.as_str())).collect::<Vec<_>>(),"route_choices":saved.schedule.route_choices.iter().filter(|(key,_)|saved.problem.routes.iter().any(|r|&r.id==*key && r.alternatives.iter().any(|a|a.tasks.iter().any(|t|t==id)))).collect::<std::collections::BTreeMap<_,_>>(),"revision":saved.revision,"locks":locks_for(&saved.problem,id)}),
                )
            }
            "schema.get" => {
                let schema = if args["model"].as_str() == Some("options") {
                    serde_json::to_value(schemars::schema_for!(Options)).unwrap()
                } else if args["model"].as_str() == Some("production") {
                    serde_json::to_value(schemars::schema_for!(crate::production::ProductionInput))
                        .unwrap()
                } else {
                    serde_json::to_value(schemars::schema_for!(Problem)).unwrap()
                };
                let definitions = schema["$defs"].as_object().unwrap();
                if let Some(name) = args["definition"].as_str() {
                    return definitions
                        .get(name)
                        .cloned()
                        .ok_or_else(|| error("SCHEMA", "Unknown definition"));
                }
                let mut root = schema.clone();
                root.as_object_mut().unwrap().remove("$defs");
                Ok(
                    json!({"root":root,"definitions":definitions.keys().collect::<Vec<_>>(),"reference_help":"Resolve #/$defs/Name through schema.get with definition=Name. Times and work use seconds relative to the epoch."}),
                )
            }
            "capabilities" => Ok(
                json!({"version":env!("CARGO_PKG_VERSION"),"schema":"apex.v3.4","accepted_schemas":["apex.v3.1","apex.v3.2","apex.v3.3","apex.v3.4"],"transports":["stdio","streamable_http","http_json"],"mcp_url":format!("{}/mcp",self.viewer_url),"openapi_url":format!("{}/openapi.json",self.viewer_url),"customizations":[{"id":"dummy_customer","version":"1"}],"engine":"rust","algorithms":[{"id":crate::xg::ID,"tool":"schedule.create"},{"id":crate::xh::ID,"tool":"schedule.hypersearch"},{"id":crate::xt::ID,"tool":"schedule.treesearch"},{"id":crate::xe::ID,"tool":"schedule.evolve"}],"features":["combined_xh_xt_improvement","xe_direct_evolution","discounted_ucb_operators","applicability_aware_xe","deduplicated_xe_proposals","native_genetic_operators","main_mode_material_search","native_conditional_catalog","shared_search_budget","incumbent_progress","declarative_planning_v1","shared_dispatch_policy","exact_campaign_idle_urgency_policies","native_candidate_filters","policy_witness_replay","calendar_breaks","rates","retention","phase_resources","pre_post","neighbor_transitions","terminal_cleanup","mode_alternatives","dependencies","min_max_lag","material_balance","running_remainders","restart_activities","mode_resource_time_order_block_locks","freeze_from_baseline","attribute_objectives","window_rules","xh_hypersearch","parallel_evaluation","pareto_archive","xt_tree_search","stage_queue_policies","objective_directions_scales","durable_import","scenarios","independent_validation","whole_workplan_choices","mode_conditionals","conditional_dags","quantity_formulas","job_order_completion","sequence_penalties","sequence_context_rules","native_sequence_objective_validation_hooks","production_order_expansion","existing_supply_pegging","route_aware_material_allocation","upstream_dispatch_urgency","conditional_mode_search","group_resource_stage_kpis"],"limitations":["exact coupled dispatch uses prefix reconstruction; no universal speed parity guarantee", "complete ready-pool policies may cost more than a 64-candidate Q window", "constructive append decoder; no guarantee of finding every feasible schedule","repair uses full reconstruction, not incremental deltas","integer-second time and fixed resource identity within a phase","XH is instance-specific dispatch/weight search, not learned generalization","solver backend unavailable","general simultaneous batch formation, stochastic simulation and arbitrary plugin hot-loading unavailable"],"tools":tool_names()}),
            ),
            "demo.create" => {
                if args["profile"].as_str() == Some("production") {
                    let input =
                        serde_json::from_str(include_str!("../examples/production-orders.json"))
                            .map_err(|e| error("DEMO", e))?;
                    return self.save(crate::production::expand(input).map_err(diagnostics)?, None);
                }
                self.save(
                    crate::demo::problem(
                        args["tasks"].as_u64().unwrap_or(24).clamp(1, 100000) as usize
                    ),
                    None,
                )
            }
            "problem.import" => {
                let value = if let Some(path) = args["path"].as_str() {
                    self.input_file(path)?
                } else {
                    args["problem"].clone()
                };
                self.save(decode(value)?, None)
            }
            "import.begin" => {
                let p: Problem = decode(args["problem"].clone())?;
                if !p.tasks.is_empty() {
                    return Err(error("IMPORT", "Header must have an empty tasks array"));
                }
                let count = args["expected_tasks"]
                    .as_u64()
                    .ok_or_else(|| error("ARGUMENT", "expected_tasks is required"))?
                    as usize;
                if count > 1_000_000 {
                    return Err(error("LIMIT", "At most one million tasks per import"));
                }
                let key = id();
                self.write(
                    &key,
                    &Import {
                        problem: p,
                        expected_tasks: count,
                        chunks: Default::default(),
                        finalized: None,
                    },
                )?;
                Ok(
                    json!({"import_id":key,"expected_tasks":count,"max_chunk_tasks":5000,"max_chunk_bytes":4194304}),
                )
            }
            "import.append" => {
                let key = field(args, "import_id")?;
                let chunk = field(args, "chunk_id")?;
                let mut session: Import = self.read(key)?;
                if session.finalized.is_some() {
                    return Err(error("FINALIZED", "Import already finalized"));
                }
                if chunk.is_empty()
                    || chunk.len() > 100
                    || !chunk
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                {
                    return Err(error("CHUNK_ID", "Use an alphanumeric chunk ID"));
                }
                let records = if let Some(path) = args["path"].as_str() {
                    self.input_file(path)?
                } else {
                    args["tasks"].clone()
                };
                let bytes = serde_json::to_vec(&records).map_err(|e| error("JSON", e))?;
                if bytes.len() > 4 * 1024 * 1024 {
                    return Err(error("CHUNK_SIZE", "Chunk exceeds 4 MiB"));
                }
                let tasks: Vec<Task> = decode(records.clone())?;
                if tasks.len() > 5000 {
                    return Err(error("CHUNK_SIZE", "Chunk exceeds 5000 tasks"));
                }
                let fingerprint = hash(&records);
                if let Some(prior) = session.chunks.get(chunk) {
                    return if prior == &fingerprint {
                        Ok(json!({"replayed":true,"chunk_id":chunk,"hash":fingerprint}))
                    } else {
                        Err(error(
                            "CHUNK_CONFLICT",
                            "Chunk ID already contains different data",
                        ))
                    };
                }
                self.write(&format!("{key}_{chunk}"), &tasks)?;
                session.chunks.insert(chunk.into(), fingerprint.clone());
                self.write(key, &session)?;
                Ok(
                    json!({"chunk_id":chunk,"tasks":tasks.len(),"hash":fingerprint,"replayed":false}),
                )
            }
            "import.status" => {
                let key = field(args, "import_id")?;
                let s: Import = self.read(key)?;
                Ok(
                    json!({"import_id":key,"chunks":s.chunks.len(),"expected_tasks":s.expected_tasks,"scenario_id":s.finalized}),
                )
            }
            "import.finalize" => {
                let key = field(args, "import_id")?;
                let mut session: Import = self.read(key)?;
                if let Some(scenario) = session.finalized {
                    return Ok(json!({"scenario_id":scenario,"replayed":true}));
                }
                let mut tasks = Vec::new();
                for chunk in session.chunks.keys() {
                    let mut rows: Vec<Task> = self.read(&format!("{key}_{chunk}"))?;
                    tasks.append(&mut rows);
                }
                if tasks.len() != session.expected_tasks {
                    return Err(error(
                        "INCOMPLETE",
                        format!(
                            "Expected {}, received {} tasks",
                            session.expected_tasks,
                            tasks.len()
                        ),
                    ));
                }
                let mut p = session.problem.clone();
                p.tasks = tasks;
                if let Err(ds) = compile::compile(&p) {
                    self.write(&format!("{key}_diagnostics"), &ds)?;
                    let mut report = diagnostics(ds);
                    report["diagnostic_id"] = json!(format!("{key}_diagnostics"));
                    return Err(report);
                }
                let result = self.save(p, None)?;
                session.finalized = Some(field(&result, "scenario_id")?.into());
                self.write(key, &session)?;
                Ok(result)
            }
            "diagnostics.page" => {
                let ds: Vec<Diagnostic> = self.read(field(args, "diagnostic_id")?)?;
                Ok(page(&ds, args))
            }
            "scenario.get" => {
                let s: Scenario = self.read(field(args, "scenario_id")?)?;
                Ok(
                    json!({"revision":s.revision,"problem_id":s.problem.id,"tasks":s.problem.tasks.len(),"resources":s.problem.resources,"objectives":s.problem.objectives,"rules":s.problem.rules,"locks":s.problem.locks,"assumptions":s.problem.assumptions,"parent":s.parent,"schema":s.problem.schema_version,"routes":s.problem.routes.len(),"jobs":s.problem.jobs.len(),"orders":s.problem.orders.len(),"customization":s.problem.customization,"freeze_zones":s.problem.freeze_zones,"queue_policy":s.problem.queue_policy,"planning":s.problem.planning}),
                )
            }
            "tasks.page" => {
                let s: Scenario = self.read(field(args, "scenario_id")?)?;
                Ok(page(&s.problem.tasks, args))
            }
            "scenario.fork" => {
                let key = field(args, "scenario_id")?;
                let s: Scenario = self.read(key)?;
                self.save(s.problem, Some(key.into()))
            }
            "scenario.patch" => {
                let key = field(args, "scenario_id")?;
                let mut s: Scenario = self.read(key)?;
                if args["expected_revision"].as_u64() != Some(s.revision) {
                    return Err(error(
                        "REVISION_CONFLICT",
                        format!("Current revision is {}", s.revision),
                    ));
                }
                let patches = args["patches"]
                    .as_array()
                    .ok_or_else(|| error("ARGUMENT", "patches must be an array"))?;
                for patch in patches {
                    apply_patch(&mut s.problem, patch)?;
                }
                compile::compile(&s.problem).map_err(diagnostics)?;
                s.revision += 1;
                self.write(key, &s)?;
                Ok(json!({"scenario_id":key,"revision":s.revision}))
            }
            "schedule.create"
            | "schedule.repair"
            | "schedule.hypersearch"
            | "schedule.treesearch"
            | "schedule.improve"
            | "schedule.evolve" => {
                let key = field(args, "scenario_id")?;
                let scenario: Scenario = self.read(key)?;
                let mut options: Options =
                    decode(args.get("options").cloned().unwrap_or(json!({})))?;
                if args
                    .get("options")
                    .and_then(|v| v.get("strategy"))
                    .is_none()
                {
                    options.strategy = "queues".into();
                }
                let incumbent: Option<SavedSchedule> =
                    if matches!(name, "schedule.improve" | "schedule.evolve") {
                        args.get("schedule_id")
                            .map(|_| self.read(field(args, "schedule_id")?))
                            .transpose()?
                    } else {
                        None
                    };
                if incumbent
                    .as_ref()
                    .is_some_and(|s| s.scenario_id != key || s.revision != scenario.revision)
                {
                    return Err(error(
                        "REVISION",
                        "Incumbent must belong to this scenario and its current revision; replan after model changes",
                    ));
                }
                let result = if name == "schedule.improve" {
                    crate::improve::improve_from(
                        &scenario.problem,
                        &options,
                        None,
                        incumbent.as_ref().map(|s| &s.schedule),
                    )
                } else if name == "schedule.evolve" {
                    // XE: direct schedule evolution.
                    if let Some(s) = &incumbent {
                        crate::xe::evolve_from(&scenario.problem, &options, &s.schedule)
                    } else {
                        crate::xe::evolve(&scenario.problem, &options)
                    }
                } else if name == "schedule.create" {
                    // XG: greedy construction.
                    xg::create(&scenario.problem, &options)
                } else if name == "schedule.treesearch" {
                    // XT: tree search.
                    crate::xt::search(&scenario.problem, &options)
                } else {
                    // XH: hypersearch, also used to reconstruct repairs.
                    crate::xh::search(&scenario.problem, &options)
                };
                let schedule = result.map_err(diagnostics)?;
                let schedule_id = id();
                let response = json!({"schedule_id":schedule_id,"scenario_id":key,"revision":scenario.revision,"valid":true,"tasks":schedule.assignments.len(),"metrics":schedule.metrics,"score":schedule.score,"elapsed_ms":schedule.elapsed_ms,"evaluations":schedule.evaluations,"selected_strategy":schedule.strategy,"seed":schedule.seed,"dispatch_weights":schedule.dispatch_weights,"route_choices":schedule.route_choices,"search":schedule.search,"replay":schedule.replay,"viewer_url":format!("{}/?schedule={schedule_id}",self.viewer_url)});
                self.write(
                    &schedule_id,
                    &SavedSchedule {
                        scenario_id: key.into(),
                        revision: scenario.revision,
                        problem: scenario.problem,
                        schedule,
                    },
                )?;
                Ok(response)
            }
            "schedule.validate" => {
                let saved: SavedSchedule = self.read(field(args, "schedule_id")?)?;
                let result = validate::validate(&saved.problem, &saved.schedule);
                Ok(serde_json::to_value(result).unwrap())
            }
            "schedule.page" => {
                let saved: SavedSchedule = self.read(field(args, "schedule_id")?)?;
                let rows: Vec<_> = saved
                    .schedule
                    .assignments
                    .iter()
                    .filter(|a| {
                        args["resource"].as_str().is_none_or(|r| {
                            a.primary == r
                                || a.activities
                                    .iter()
                                    .flat_map(|x| &x.reservations)
                                    .any(|x| x.resource == r)
                        })
                    })
                    .collect();
                let mut result = page(&rows, args);
                result["metrics"] = json!(saved.schedule.metrics);
                result["revision"] = json!(saved.revision);
                result["scenario_id"] = json!(saved.scenario_id);
                result["epoch"] = json!(saved.problem.epoch);
                result["planned_count"] = json!(saved.schedule.assignments.len());
                result["elapsed_ms"] = json!(saved.schedule.elapsed_ms);
                result["strategy"] = json!(saved.schedule.strategy);
                result["dispatch_summary"] = json!(saved.schedule.dispatch.as_ref().map(|e| json!({"version":e.version,"steps":e.decisions.len(),"retained_steps":e.steps.len(),"exact_probes":e.exact_probes,"direct_placements":e.direct_placements})));
                result["planning"] = json!(saved.problem.planning);
                result["material_allocation_count"] = json!(
                    saved
                        .schedule
                        .material_report
                        .as_ref()
                        .map(|r| r.allocations.len())
                );
                result["search"] = json!(saved.schedule.search);
                result["replay"] = json!(saved.schedule.replay);
                result["freeze_zones"] = json!(saved.problem.freeze_zones);
                result["route_choices"] = json!(
                    saved
                        .schedule
                        .route_choices
                        .iter()
                        .take(200)
                        .collect::<std::collections::BTreeMap<_, _>>()
                );
                result["job_completions"] = json!(
                    saved
                        .schedule
                        .job_completions
                        .iter()
                        .take(200)
                        .collect::<std::collections::BTreeMap<_, _>>()
                );
                result["order_completions"] = json!(
                    saved
                        .schedule
                        .order_completions
                        .iter()
                        .take(200)
                        .collect::<std::collections::BTreeMap<_, _>>()
                );
                result["route_choices_total"] = json!(saved.schedule.route_choices.len());
                result["relationship_metadata_truncated"] = json!(
                    saved.schedule.route_choices.len() > 200
                        || saved.schedule.job_completions.len() > 200
                        || saved.schedule.order_completions.len() > 200
                );
                result["relationship_help"] =
                    json!("Use model.page with schedule_id to page all choices and completions");
                let inputs: std::collections::HashMap<_, _> =
                    saved.problem.tasks.iter().map(|t| (&t.id, t)).collect();
                if let Some(items) = result["items"].as_array_mut() {
                    for item in items {
                        if let Some(t) = item["task"]
                            .as_str()
                            .and_then(|id| inputs.get(&id.to_string()))
                        {
                            item["job"] = json!(t.job);
                            item["quantity"] = json!(t.quantity);
                            item["locks"] = json!(locks_for(&saved.problem, &t.id));
                            item["running"] = json!(t.execution.is_some());
                        }
                    }
                }
                Ok(result)
            }
            "scenario.compare" => {
                let a: SavedSchedule = self.read(field(args, "baseline_id")?)?;
                let b: SavedSchedule = self.read(field(args, "candidate_id")?)?;
                let by: std::collections::HashMap<_, _> = a
                    .schedule
                    .assignments
                    .iter()
                    .map(|a| (&a.task, a))
                    .collect();
                let current: std::collections::HashSet<_> = b
                    .schedule
                    .assignments
                    .iter()
                    .map(|a| a.task.as_str())
                    .collect();
                let removed: Vec<_> = a
                    .schedule
                    .assignments
                    .iter()
                    .filter(|a| !current.contains(a.task.as_str()))
                    .map(|a| a.task.clone())
                    .collect();
                let changed: Vec<_> = b
                    .schedule
                    .assignments
                    .iter()
                    .filter(|b| {
                        by.get(&b.task).is_none_or(|a| {
                            a.start != b.start || a.end != b.end || a.mode != b.mode
                        })
                    })
                    .map(|a| a.task.clone())
                    .collect();
                Ok(
                    json!({"baseline":a.schedule.metrics,"candidate":b.schedule.metrics,"changed_count":changed.len(),"changed_tasks":changed.iter().take(100).collect::<Vec<_>>(),"truncated":changed.len()>100 || removed.len()>100,"removed_count":removed.len(),"removed_tasks":removed.iter().take(100).collect::<Vec<_>>(),"baseline_routes":a.schedule.route_choices,"candidate_routes":b.schedule.route_choices}),
                )
            }
            "scenario.freeze" => {
                let key = field(args, "scenario_id")?;
                let mut s: Scenario = self.read(key)?;
                if args["expected_revision"].as_u64() != Some(s.revision) {
                    return Err(error("REVISION_CONFLICT", "Refresh scenario revision"));
                }
                let baseline: SavedSchedule = self.read(field(args, "schedule_id")?)?;
                let until = args["until"]
                    .as_i64()
                    .ok_or_else(|| error("ARGUMENT", "until must be integer seconds"))?;
                if baseline.problem.id != s.problem.id {
                    return Err(error(
                        "BASELINE",
                        "Freeze baseline belongs to a different problem",
                    ));
                }
                let overrides: std::collections::BTreeMap<String, Time> =
                    decode(args.get("until_by_resource").cloned().unwrap_or(json!({})))?;
                if until < 0
                    || until > s.problem.horizon
                    || overrides.iter().any(|(id, time)| {
                        *time < 0
                            || *time > s.problem.horizon
                            || !s.problem.resources.iter().any(|r| &r.id == id)
                    })
                {
                    return Err(error(
                        "FREEZE_BOUNDARY",
                        "Invalid global or resource-specific boundary",
                    ));
                }
                let dimensions = args["dimensions"]
                    .as_array()
                    .ok_or_else(|| error("ARGUMENT", "dimensions must be an array"))?;
                if dimensions
                    .iter()
                    .any(|x| !matches!(x.as_str(), Some("mode" | "resource" | "start" | "order")))
                {
                    return Err(error("ARGUMENT", "Unknown freeze dimension"));
                }
                let mut selected: Vec<_> = baseline
                    .schedule
                    .assignments
                    .iter()
                    .filter(|a| {
                        a.start < *overrides.get(&a.primary).unwrap_or(&until)
                            && args["resource"].as_str().is_none_or(|r| r == a.primary)
                    })
                    .collect();
                selected.sort_by_key(|a| a.start);
                for a in &selected {
                    for dimension in dimensions {
                        match dimension.as_str().unwrap() {
                            "mode" => s.problem.locks.push(Lock::Mode {
                                task: a.task.clone(),
                                mode: a.mode.clone(),
                            }),
                            "resource" => s.problem.locks.push(Lock::Resource {
                                task: a.task.clone(),
                                resource: a.primary.clone(),
                            }),
                            "start" => s.problem.locks.push(Lock::Start {
                                task: a.task.clone(),
                                at: a.start,
                            }),
                            _ => {}
                        }
                    }
                }
                if dimensions.contains(&json!("order")) {
                    for r in &s.problem.resources {
                        let tasks: Vec<_> = selected
                            .iter()
                            .filter(|a| a.primary == r.id)
                            .map(|a| a.task.clone())
                            .collect();
                        if tasks.len() > 1 {
                            s.problem.locks.push(Lock::Order {
                                resource: r.id.clone(),
                                tasks,
                                consecutive: false,
                            });
                        }
                    }
                }
                for r in &s.problem.resources {
                    if args["resource"].as_str().is_none_or(|id| id == r.id) {
                        s.problem.freeze_zones.push(FreezeZone {
                            resource: r.id.clone(),
                            until: *overrides.get(&r.id).unwrap_or(&until),
                            dimensions: dimensions
                                .iter()
                                .map(|d| d.as_str().unwrap().to_owned())
                                .collect(),
                            tasks: selected
                                .iter()
                                .filter(|a| a.primary == r.id)
                                .map(|a| a.task.clone())
                                .collect(),
                            baseline_schedule: field(args, "schedule_id")?.into(),
                        });
                    }
                }
                compile::compile(&s.problem).map_err(diagnostics)?;
                s.problem.assumptions.push(format!(
                    "Freeze membership pinned to schedule {} revision {}: baseline start < {until}",
                    field(args, "schedule_id")?,
                    baseline.revision
                ));
                s.revision += 1;
                self.write(key, &s)?;
                Ok(json!({"revision":s.revision,"frozen_tasks":selected.len()}))
            }
            "solver.solve" => Err(error(
                "UNSUPPORTED_BACKEND",
                "No solver is installed. Use schedule.create or schedule.hypersearch.",
            )),
            _ => Err(error("UNKNOWN_TOOL", name)),
        }
    }
}
fn locks_for(p: &Problem, id: &str) -> Vec<Lock> {
    let mut result: Vec<_> = p
        .locks
        .iter()
        .filter(|l| match l {
            Lock::Mode { task, .. } | Lock::Resource { task, .. } | Lock::Start { task, .. } => {
                task == id
            }
            Lock::Order { tasks, .. } => tasks.iter().any(|task| task == id),
        })
        .cloned()
        .collect();
    for constraint in &p.planning.constraints {
        match constraint {
            crate::language::Constraint::Sequence {
                resource,
                tasks,
                consecutive,
                ..
            } if tasks.iter().any(|t| t == id) => result.push(Lock::Order {
                resource: resource.clone(),
                tasks: tasks.clone(),
                consecutive: *consecutive,
            }),
            crate::language::Constraint::EligibleResources {
                select, resources, ..
            } if resources.len() == 1
                && p.tasks.iter().any(|t| t.id == id && select.matches(t)) =>
            {
                result.push(Lock::Resource {
                    task: id.into(),
                    resource: resources[0].clone(),
                })
            }
            _ => (),
        }
    }
    result
}
fn page<T: Serialize>(rows: &[T], args: &Value) -> Value {
    let offset = (args["offset"].as_u64().unwrap_or(0) as usize).min(rows.len());
    let limit = (args["limit"].as_u64().unwrap_or(50) as usize).clamp(1, 200);
    let end = (offset + limit).min(rows.len());
    json!({"total":rows.len(),"offset":offset,"items":&rows[offset..end],"next_offset":if end<rows.len(){Some(end)}else{None}})
}
fn apply_patch(p: &mut Problem, v: &Value) -> Result<(), Value> {
    match field(v, "kind")? {
        "due" | "priority" => {
            let kind = field(v, "kind")?;
            let id = field(v, "task")?;
            let t = p
                .tasks
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| error("TASK", id))?;
            if kind == "due" {
                t.due = Some(
                    v["value"]
                        .as_i64()
                        .ok_or_else(|| error("ARGUMENT", "Integer due required"))?,
                );
            } else {
                t.priority = v["value"]
                    .as_f64()
                    .ok_or_else(|| error("ARGUMENT", "Numeric priority required"))?;
            }
        }
        "conditional_modes" => {
            let id = field(v, "task")?;
            let t = p
                .tasks
                .iter_mut()
                .find(|t| t.id == id)
                .ok_or_else(|| error("TASK", id))?;
            t.conditional_modes = decode(v["choices"].clone())?;
        }
        "route_choice" => {
            let id = field(v, "route")?;
            let route = p
                .routes
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or_else(|| error("ROUTE", id))?;
            route.selected = decode(v["alternative"].clone())?;
        }
        "customization" => p.customization = decode(v["customization"].clone())?,
        "job" => {
            let id = field(v, "job")?;
            let job = p
                .jobs
                .iter_mut()
                .find(|j| j.id == id)
                .ok_or_else(|| error("JOB", id))?;
            if v.get("due").is_some() {
                job.due = decode(v["due"].clone())?;
            }
            if v.get("deadline").is_some() {
                job.deadline = decode(v["deadline"].clone())?;
            }
            if v.get("priority").is_some() {
                job.priority = decode(v["priority"].clone())?;
            }
        }
        "lock" => p.locks.push(decode(v["lock"].clone())?),
        "queue_policy" => p.queue_policy = decode(v["queue_policy"].clone())?,
        "queue_definitions" => p.queue_definitions = decode(v["queue_definitions"].clone())?,
        "objectives" => p.objectives = decode(v["objectives"].clone())?,
        "rules" => p.rules = decode(v["rules"].clone())?,
        "planning" => p.planning = decode(v["planning"].clone())?,
        "downtime" => {
            let id = field(v, "resource")?;
            let start = v["start"]
                .as_i64()
                .ok_or_else(|| error("ARGUMENT", "start required"))?;
            let end = v["end"]
                .as_i64()
                .ok_or_else(|| error("ARGUMENT", "end required"))?;
            if start < 0 || end <= start || end > p.horizon {
                return Err(error("INTERVAL", "Invalid downtime interval"));
            }
            let r = p
                .resources
                .iter_mut()
                .find(|r| r.id == id)
                .ok_or_else(|| error("RESOURCE", id))?;
            let cut = |windows: &[Window]| {
                windows
                    .iter()
                    .flat_map(|w| {
                        let mut result = vec![];
                        if w.start < start {
                            let mut a = w.clone();
                            a.end = a.end.min(start);
                            if a.end > a.start {
                                result.push(a);
                            }
                        }
                        if w.end > end {
                            let mut b = w.clone();
                            b.start = b.start.max(end);
                            if b.end > b.start {
                                result.push(b);
                            }
                        }
                        result
                    })
                    .collect()
            };
            r.calendar = cut(&r.calendar);
            if v["forbid_retention"].as_bool().unwrap_or(false) {
                if r.retention_calendar.is_empty() {
                    r.retention_calendar = vec![Window {
                        start: 0,
                        end: p.horizon,
                        capacity: r.capacity,
                        rate: 1.0,
                    }];
                }
                r.retention_calendar = cut(&r.retention_calendar);
            }
        }
        _ => return Err(error("PATCH", "Unsupported patch kind")),
    }
    Ok(())
}
pub fn tool_names() -> Vec<&'static str> {
    vec![
        "capabilities",
        "schema.get",
        "queues.inspect",
        "policy.inspect",
        "schedule.explain_decision",
        "demo.create",
        "problem.import",
        "production.import",
        "material.prepare",
        "artifact.read",
        "model.page",
        "task.inspect",
        "import.begin",
        "import.append",
        "import.status",
        "import.finalize",
        "diagnostics.page",
        "scenario.get",
        "tasks.page",
        "scenario.fork",
        "scenario.patch",
        "scenario.freeze",
        "schedule.create",
        "schedule.improve",
        "schedule.evolve",
        "schedule.treesearch",
        "schedule.repair",
        "schedule.hypersearch",
        "schedule.validate",
        "schedule.page",
        "scenario.compare",
        "solver.solve",
    ]
}
