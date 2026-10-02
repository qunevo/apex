//! A deterministic fake engine and store conformance checks. Every `Store`
//! implementation must pass [`conformance`].
use crate::{
    Control,
    auth::{Actor, Role},
    engine::*,
    error::Error,
    model::*,
    ops::{CreateScenario, DecideResult, ReviseScenario, StartRun},
    store::Store,
    worker::Worker,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

/// Schedules `facts.operations` (`[{id, resource, duration}]`) back to back per resource.
pub struct FakeEngine;

impl EngineAdapter for FakeEngine {
    fn descriptor(&self) -> EngineDescriptor {
        EngineDescriptor {
            engine: ComponentRef {
                id: "fake".into(),
                version: "1".into(),
            },
            model: "fake".into(),
            declarations: vec!["pin".into()],
            methods: vec!["sequence".into()],
            extensions: Vec::new(),
        }
    }
    fn check_options(&self, options: &Value) -> Result<(), Vec<Diagnostic>> {
        match options.get("fail") {
            Some(Value::Bool(_)) | None => Ok(()),
            Some(_) => Err(vec![Diagnostic::new("OPTION", "fail must be a boolean")]),
        }
    }
    fn check_facts(&self, facts: &Value) -> Result<(), Vec<Diagnostic>> {
        if facts["operations"].is_array() {
            Ok(())
        } else {
            Err(vec![Diagnostic::new(
                "FACTS",
                "operations must be an array",
            )])
        }
    }
    fn prepare(&self, content: &ScenarioContent) -> Result<Prepared, Vec<Diagnostic>> {
        self.check_facts(&content.facts)?;
        Ok(Prepared {
            model: content.facts.clone(),
            extensions: Vec::new(),
            warnings: Vec::new(),
        })
    }
    fn run(
        &self,
        prepared: &Prepared,
        options: &Value,
        _cancel: &CancelToken,
    ) -> Result<EngineOutput, Vec<Diagnostic>> {
        if options["fail"] == json!(true) {
            return Err(vec![Diagnostic::new("ENGINE", "requested failure")]);
        }
        let mut clock: BTreeMap<String, i64> = BTreeMap::new();
        let mut operations = Vec::new();
        for op in prepared.model["operations"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let resource = op["resource"].as_str().unwrap_or("default").to_string();
            let start = *clock.get(&resource).unwrap_or(&0);
            let end = start + op["duration"].as_i64().unwrap_or(1);
            clock.insert(resource.clone(), end);
            operations.push(ViewOperation {
                id: op["id"].as_str().unwrap_or_default().into(),
                resource,
                start,
                end,
                label: None,
            });
        }
        let makespan = clock.values().copied().max().unwrap_or(0) as f64;
        let view = ScheduleView {
            epoch: None,
            resources: clock
                .keys()
                .map(|id| ViewResource { id: id.clone() })
                .collect(),
            operations,
        };
        Ok(EngineOutput {
            schedule: serde_json::to_value(&view).unwrap(),
            view,
            metrics: BTreeMap::from([("makespan".into(), makespan)]),
            seed: Some(7),
        })
    }
    fn validate(&self, _prepared: &Prepared, schedule: &Value) -> ValidationReport {
        let ok = serde_json::from_value::<ScheduleView>(schedule.clone())
            .is_ok_and(|v| v.operations.iter().all(|o| o.end >= o.start));
        ValidationReport {
            valid: ok,
            validator: "fake-validator".into(),
            diagnostics: Vec::new(),
        }
    }
}

pub fn actor(tenant: TenantId, roles: &[Role]) -> Actor {
    Actor {
        tenant,
        id: format!("{:?}", roles).to_lowercase(),
        roles: roles.to_vec(),
    }
}

pub fn content(durations: &[i64]) -> ScenarioContent {
    let operations: Vec<_> = durations
        .iter()
        .enumerate()
        .map(|(i, d)| json!({"id": format!("op{i}"), "resource": "m1", "duration": d}))
        .collect();
    ScenarioContent {
        customization_package: None,
        facts: json!({ "operations": operations }),
        intent: PlanningIntent::default(),
    }
}

fn conflict<T: std::fmt::Debug>(r: crate::Result<T>) -> bool {
    matches!(r, Err(Error::Conflict(_)))
}

/// End-to-end workflow and isolation checks against a fresh, empty store.
pub async fn conformance(store: Arc<dyn Store>) {
    let engines = EngineRegistry::default().with(Arc::new(FakeEngine));
    let control = Control::new(store.clone(), engines.clone());
    let worker = Worker::new(store.clone(), engines, "worker-1");
    let (a, b) = (TenantId::new(), TenantId::new());
    for (id, name) in [(a, "A"), (b, "B")] {
        store
            .ensure_tenant(&Tenant {
                id,
                name: name.into(),
            })
            .await
            .unwrap();
    }
    let planner = actor(a, &[Role::Planner]);
    let approver = actor(a, &[Role::Approver]);
    let outsider = actor(b, &[Role::Admin]);

    // Scenario creation checks facts and refuses unsupported declarations.
    let mut unsupported = content(&[1]);
    unsupported.intent.declarations.push(Declaration {
        id: "d1".into(),
        kind: "freeze".into(),
        target: Value::Null,
        parameters: Value::Null,
        note: None,
    });
    let request = |content| CreateScenario {
        customization: None,
        name: "Week 40".into(),
        engine: "fake".into(),
        content,
        note: None,
    };
    assert!(matches!(
        control.create_scenario(&planner, request(unsupported)).await,
        Err(Error::Invalid { diagnostics, .. }) if diagnostics[0].code == "UNSUPPORTED_DECLARATION"
    ));
    assert!(matches!(
        control
            .create_scenario(&approver, request(content(&[1])))
            .await,
        Err(Error::Forbidden(_))
    ));
    let (scenario, first) = control
        .create_scenario(&planner, request(content(&[3, 4])))
        .await
        .unwrap();
    assert_eq!((scenario.current_revision, first.number), (1, 1));

    // Tenant isolation.
    assert!(matches!(
        control.get_scenario(&outsider, scenario.id).await,
        Err(Error::NotFound(_))
    ));
    assert!(control.list_scenarios(&outsider).await.unwrap().is_empty());

    // A run keeps its revision even when the scenario changes meanwhile.
    let run = control
        .start_run(&planner, scenario.id, StartRun::default())
        .await
        .unwrap();
    let revise = |expected, durations: &[i64]| ReviseScenario {
        expected_revision: expected,
        content: content(durations),
        note: Some("rush order".into()),
    };
    let second = control
        .revise_scenario(&planner, scenario.id, revise(1, &[3, 4, 5]))
        .await
        .unwrap();
    assert_eq!(second.number, 2);
    assert!(conflict(
        control
            .revise_scenario(&planner, scenario.id, revise(1, &[9]))
            .await
    ));
    assert!(matches!(
        control.get_run(&outsider, run.id).await,
        Err(Error::NotFound(_))
    ));

    let finished = worker.run_once().await.unwrap().unwrap();
    assert_eq!((finished.id, finished.state), (run.id, RunState::Succeeded));
    let old = control
        .get_result(&planner, finished.result.unwrap())
        .await
        .unwrap();
    assert_eq!(old.revision, 1);
    assert_eq!(old.metrics["makespan"], 7.0);
    assert_eq!(old.provenance.content_hash, first.content_hash);
    assert_eq!(old.provenance.engine.id, "fake");
    assert_eq!(old.provenance.seed, Some(7));
    assert!(control.get_result(&outsider, old.id).await.is_err());

    // Approval is separate from planning; stale results cannot be published.
    assert!(matches!(
        control
            .approve_result(&planner, old.id, DecideResult::default())
            .await,
        Err(Error::Forbidden(_))
    ));
    assert!(conflict(
        control
            .publish_result(&approver, old.id, DecideResult::default())
            .await
    ));
    control
        .approve_result(&approver, old.id, DecideResult::default())
        .await
        .unwrap();
    assert!(conflict(
        control
            .publish_result(&approver, old.id, DecideResult::default())
            .await
    ));

    // The current revision's result can be published and supersedes the previous one.
    let publish_current = async || {
        control
            .start_run(&planner, scenario.id, StartRun::default())
            .await
            .unwrap();
        let run = worker.run_once().await.unwrap().unwrap();
        let id = run.result.unwrap();
        control
            .approve_result(&approver, id, DecideResult::default())
            .await
            .unwrap();
        control
            .publish_result(&approver, id, DecideResult::default())
            .await
            .unwrap()
    };
    let first_published = publish_current().await;
    assert_eq!(
        (first_published.revision, first_published.status),
        (2, ResultStatus::Published)
    );
    let second_published = publish_current().await;
    let scenario_now = control.get_scenario(&planner, scenario.id).await.unwrap();
    assert_eq!(scenario_now.published_result, Some(second_published.id));
    let superseded = control
        .get_result(&planner, first_published.id)
        .await
        .unwrap();
    assert_eq!(superseded.status, ResultStatus::Superseded);
    assert_eq!(
        control
            .list_results(&planner, scenario.id)
            .await
            .unwrap()
            .len(),
        3
    );

    // Engine failures fail the run with diagnostics and produce no result.
    let failing = StartRun {
        revision: None,
        options: json!({"fail": true}),
    };
    control
        .start_run(&planner, scenario.id, failing)
        .await
        .unwrap();
    let failed = worker.run_once().await.unwrap().unwrap();
    assert_eq!(failed.state, RunState::Failed);
    assert_eq!(failed.diagnostics[0].code, "ENGINE");
    assert!(failed.result.is_none());
    assert!(matches!(
        control
            .start_run(
                &planner,
                scenario.id,
                StartRun {
                    revision: None,
                    options: json!({"fail": 1})
                }
            )
            .await,
        Err(Error::Invalid { .. })
    ));

    // Queued runs cancel immediately; finished runs cannot be cancelled.
    let queued = control
        .start_run(&planner, scenario.id, StartRun::default())
        .await
        .unwrap();
    let cancelled = control.cancel_run(&planner, queued.id).await.unwrap();
    assert_eq!(cancelled.state, RunState::Cancelled);
    assert!(conflict(control.cancel_run(&planner, queued.id).await));
    assert!(worker.run_once().await.unwrap().is_none());

    // A run abandoned by its worker is reclaimed after the lease expires.
    let abandoned = control
        .start_run(&planner, scenario.id, StartRun::default())
        .await
        .unwrap();
    let mut crashed = Worker::new(
        store.clone(),
        EngineRegistry::default().with(Arc::new(FakeEngine)),
        "crashed",
    );
    crashed.lease = chrono::Duration::milliseconds(-1);
    let lost = crashed.claim().await.unwrap().unwrap();
    assert_eq!(lost.run.id, abandoned.id);
    let recovered = worker.run_once().await.unwrap().unwrap();
    assert_eq!(
        (recovered.id, recovered.state, recovered.attempts),
        (abandoned.id, RunState::Succeeded, 2)
    );
    assert!(
        crashed
            .finish(&lost, crate::store::Completion::Cancelled)
            .await
            .is_err()
    );
}

#[cfg(test)]
#[tokio::test]
async fn memory_store_conforms() {
    conformance(Arc::new(crate::memory::MemoryStore::new())).await;
}
