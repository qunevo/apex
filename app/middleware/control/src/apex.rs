//! `EngineAdapter` for the APEX scheduling engine. Facts are a scheduling
//! problem; run options select a method and carry engine `Options`.
use crate::{
    ComponentRef, Diagnostic, ScenarioContent, ScheduleView, ValidationReport, ViewOperation,
    ViewResource,
    engine::{CancelToken, EngineAdapter, EngineDescriptor, EngineOutput, Prepared},
};
use apex_engine::{compile, improve, model, validate, xe, xg, xh, xt};
use serde::Deserialize;
use serde_json::Value;

pub const ENGINE_ID: &str = "apex";
const METHODS: [&str; 5] = ["create", "hypersearch", "treesearch", "evolve", "improve"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RunOptions {
    #[serde(default = "default_method")]
    method: String,
    #[serde(default)]
    options: Option<Value>,
}
fn default_method() -> String {
    "create".into()
}

fn diagnostic(d: &model::Diagnostic) -> Diagnostic {
    Diagnostic::new(&d.code, d.message.clone()).at(d.entity.clone())
}
fn diagnostics(ds: Vec<model::Diagnostic>) -> Vec<Diagnostic> {
    ds.iter().map(diagnostic).collect()
}
fn schema(code: &str, e: impl ToString) -> Vec<Diagnostic> {
    vec![Diagnostic::new(code, e.to_string())]
}

fn problem(facts: &Value) -> Result<model::Problem, Vec<Diagnostic>> {
    serde_json::from_value(facts.clone()).map_err(|e| schema("FACTS_SCHEMA", e))
}

fn run_options(value: &Value) -> Result<(String, model::Options), Vec<Diagnostic>> {
    let raw = if value.is_null() {
        RunOptions {
            method: default_method(),
            options: None,
        }
    } else {
        serde_json::from_value::<RunOptions>(value.clone())
            .map_err(|e| schema("OPTIONS_SCHEMA", e))?
    };
    if !METHODS.contains(&raw.method.as_str()) {
        return Err(schema(
            "OPTIONS_METHOD",
            format!("Unknown method {}; use one of {METHODS:?}", raw.method),
        ));
    }
    let explicit_strategy = raw
        .options
        .as_ref()
        .is_some_and(|o| o.get("strategy").is_some());
    let mut options: model::Options = match raw.options {
        Some(o) => serde_json::from_value(o).map_err(|e| schema("OPTIONS_SCHEMA", e))?,
        None => model::Options::default(),
    };
    // Match the agent tools: queue policies unless a strategy is requested.
    if !explicit_strategy {
        options.strategy = "queues".into();
    }
    Ok((raw.method, options))
}

fn view(problem: &model::Problem, schedule: &model::Schedule) -> ScheduleView {
    ScheduleView {
        epoch: problem.epoch.clone(),
        resources: problem
            .resources
            .iter()
            .map(|r| ViewResource { id: r.id.clone() })
            .collect(),
        operations: schedule
            .assignments
            .iter()
            .map(|a| ViewOperation {
                id: a.task.clone(),
                resource: a.primary.clone(),
                start: a.start,
                end: a.end,
                label: Some(a.mode.clone()),
            })
            .collect(),
    }
}

/// Stateless adapter; the engine itself is deterministic for a given seed.
#[derive(Clone, Copy, Debug, Default)]
pub struct ApexEngine;

impl EngineAdapter for ApexEngine {
    fn descriptor(&self) -> EngineDescriptor {
        EngineDescriptor {
            engine: ComponentRef {
                id: ENGINE_ID.into(),
                version: apex_engine::VERSION.into(),
            },
            model: model::PROBLEM_MODEL.into(),
            // Planning intent is not compiled yet; commitments live in the facts.
            declarations: Vec::new(),
            methods: METHODS.iter().map(|m| m.to_string()).collect(),
            extensions: vec![ComponentRef {
                id: "demo".into(),
                version: "1".into(),
            }],
        }
    }

    fn check_options(&self, options: &Value) -> Result<(), Vec<Diagnostic>> {
        run_options(options).map(|_| ())
    }

    fn check_facts(&self, facts: &Value) -> Result<(), Vec<Diagnostic>> {
        let problem = problem(facts)?;
        compile::compile(&problem).map(|_| ()).map_err(diagnostics)
    }

    fn prepare(&self, content: &ScenarioContent) -> Result<Prepared, Vec<Diagnostic>> {
        let problem = problem(&content.facts)?;
        compile::compile(&problem).map_err(diagnostics)?;
        Ok(Prepared {
            extensions: problem
                .customization
                .iter()
                .map(|c| ComponentRef {
                    id: c.id.clone(),
                    version: c.version.clone(),
                })
                .collect(),
            model: content.facts.clone(),
            warnings: Vec::new(),
        })
    }

    fn run(
        &self,
        prepared: &Prepared,
        options: &Value,
        _cancel: &CancelToken,
    ) -> Result<EngineOutput, Vec<Diagnostic>> {
        // The engine has no cooperative cancellation yet; `budget_ms` bounds search time.
        let problem = problem(&prepared.model)?;
        let (method, options) = run_options(options)?;
        let schedule = match method.as_str() {
            "create" => xg::create(&problem, &options),
            "hypersearch" => xh::search(&problem, &options),
            "treesearch" => xt::search(&problem, &options),
            "evolve" => xe::evolve(&problem, &options),
            _ => improve::improve(&problem, &options),
        }
        .map_err(diagnostics)?;
        Ok(EngineOutput {
            view: view(&problem, &schedule),
            metrics: schedule.metrics.clone(),
            seed: Some(schedule.seed),
            schedule: serde_json::to_value(&schedule).map_err(|e| schema("SCHEDULE", e))?,
        })
    }

    fn validate(&self, prepared: &Prepared, schedule: &Value) -> ValidationReport {
        let checked = problem(&prepared.model).and_then(|p| {
            let s: model::Schedule =
                serde_json::from_value(schedule.clone()).map_err(|e| schema("SCHEDULE", e))?;
            Ok(validate::validate(&p, &s))
        });
        let (valid, diagnostics) = match checked {
            Ok(v) => (v.valid, v.diagnostics.iter().map(diagnostic).collect()),
            Err(d) => (false, d),
        };
        ValidationReport {
            valid,
            validator: "apex.validate".into(),
            diagnostics,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Control, EngineRegistry, Role, RunState, Store, Tenant, TenantId,
        memory::MemoryStore,
        ops::{CreateScenario, StartRun},
        testing::actor,
        worker::Worker,
    };
    use serde_json::json;
    use std::sync::Arc;

    fn content(facts: Value) -> ScenarioContent {
        ScenarioContent {
            customization_package: None,
            facts,
            intent: Default::default(),
        }
    }

    #[tokio::test]
    async fn plans_validates_and_records_provenance() {
        let store: Arc<dyn Store> = Arc::new(MemoryStore::new());
        let engines = EngineRegistry::default().with(Arc::new(ApexEngine));
        let control = Control::new(store.clone(), engines.clone());
        let tenant = TenantId::new();
        store
            .ensure_tenant(&Tenant {
                id: tenant,
                name: "Synthetic".into(),
            })
            .await
            .unwrap();
        let planner = actor(tenant, &[Role::Planner]);
        let facts = serde_json::to_value(apex_engine::demo::problem(12)).unwrap();
        let (scenario, _) = control
            .create_scenario(
                &planner,
                CreateScenario {
                    customization: None,
                    name: "Demo".into(),
                    engine: ENGINE_ID.into(),
                    content: content(facts),
                    note: None,
                },
            )
            .await
            .unwrap();
        for method in ["create", "hypersearch"] {
            let options = json!({"method": method, "options": {"seed": 3, "iterations": 4}});
            control
                .start_run(
                    &planner,
                    scenario.id,
                    StartRun {
                        revision: None,
                        options,
                    },
                )
                .await
                .unwrap();
            let run = Worker::new(store.clone(), engines.clone(), "w")
                .run_once()
                .await
                .unwrap()
                .unwrap();
            assert_eq!(run.state, RunState::Succeeded, "{:?}", run.diagnostics);
            let result = control
                .get_result(&planner, run.result.unwrap())
                .await
                .unwrap();
            assert!(result.validation.valid);
            assert_eq!(result.validation.validator, "apex.validate");
            assert_eq!(result.view.operations.len(), 12);
            assert_eq!(result.provenance.engine.id, ENGINE_ID);
            assert_eq!(result.provenance.seed, Some(3));
        }
    }

    #[test]
    fn rejects_invalid_facts_options_and_corrupted_schedules() {
        let engine = ApexEngine;
        assert!(engine.check_facts(&json!({"id": "x"})).is_err());
        assert!(engine.check_options(&json!({"method": "magic"})).is_err());
        assert!(
            engine
                .check_options(&json!({"options": {"seed": "x"}}))
                .is_err()
        );
        assert!(engine.check_options(&Value::Null).is_ok());

        let facts = serde_json::to_value(apex_engine::demo::problem(6)).unwrap();
        let prepared = engine.prepare(&content(facts)).unwrap();
        let output = engine
            .run(&prepared, &Value::Null, &CancelToken::default())
            .unwrap();
        assert!(engine.validate(&prepared, &output.schedule).valid);
        let mut corrupted = output.schedule.clone();
        corrupted["assignments"][0]["end"] = json!(-5);
        assert!(!engine.validate(&prepared, &corrupted).valid);
        assert!(!engine.validate(&prepared, &json!({"bogus": true})).valid);
    }
}
