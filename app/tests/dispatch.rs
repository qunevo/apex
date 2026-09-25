use apex::{demo, model::*, policy, rules::Customization, validate, xg};
use serde_json::{Value, json};
fn problem(n: usize) -> Problem {
    let mut p = demo::problem(n);
    p.schema_version = "apex.v3.4".into();
    for (i, t) in p.tasks.iter_mut().enumerate() {
        t.id = format!("T{i}");
        t.family = "A".into();
        t.stage = "0".into();
        t.due = Some(100 + i as i64);
        t.priority = 1.0;
        t.modes = vec![demo::mode("M0", 10.0)];
    }
    p
}
fn campaign(p: &mut Problem, basis: &str, minimum: f64) {
    p.planning.policies=serde_json::from_value(json!([{"kind":"campaign","id":"campaign","resource":"M0","minimum":minimum,"basis":basis,"initial":{"family":"A","credit":0},"on_no_match":"allow_switch"}])).unwrap();
}
fn seq(s: &Schedule) -> Vec<String> {
    s.dispatch
        .as_ref()
        .unwrap()
        .decisions
        .iter()
        .map(|d| d.task.clone())
        .collect()
}
fn set(p: &mut Problem, key: &str, value: Value) {
    let mut v = serde_json::to_value(&p.planning.policies).unwrap();
    v[0][key] = value;
    p.planning.policies = serde_json::from_value(v).unwrap();
}
fn o() -> Options {
    Options {
        strategy: "queues".into(),
        iterations: 12,
        xh: XhConfig {
            population_size: 4,
            ..Default::default()
        },
        budget_ms: 0,
        ..Default::default()
    }
}
#[test]
fn candidate_beyond_window_is_found_and_prefix_cannot_bypass_policy() {
    let mut p = problem(66);
    for t in &mut p.tasks[..65] {
        t.family = "B".into();
    }
    campaign(&mut p, "work", 10.0);
    let s = xg::create(&p, &o()).unwrap();
    assert_eq!(seq(&s)[0], "T65");
    assert_eq!(s.dispatch.as_ref().unwrap().steps[0].considered, 66);
    let mut options = o();
    options.decision_prefix = vec![Decision {
        task: "T0".into(),
        mode: "on-M0".into(),
    }];
    assert_eq!(
        xg::create(&p, &options).unwrap_err()[0].code,
        "DISPATCH_PREFIX"
    );
    assert!(validate::validate(&p, &s).valid);
}
#[test]
fn singleton_fallback_is_explicit_and_fixed_start_does_not_silently_override_policy() {
    let mut p = problem(1);
    p.tasks[0].family = "B".into();
    campaign(&mut p, "work", 60.0);
    assert!(xg::create(&p, &o()).is_ok());
    set(&mut p, "on_no_match", json!("fail"));
    assert_eq!(
        xg::create(&p, &o()).unwrap_err()[0].code,
        "DISPATCH_CONFLICT"
    );
    p.locks.push(Lock::Start {
        task: "T0".into(),
        at: 0,
    });
    set(&mut p, "exempt_committed", json!(true));
    assert!(xg::create(&p, &o()).is_ok());
}
#[test]
fn productive_time_excludes_breaks_and_differs_from_nominal_work_at_changed_rates() {
    let mut p = problem(3);
    p.tasks[1].family = "B".into();
    p.tasks[0].due = Some(0);
    p.resources[0].calendar = vec![
        Window {
            start: 0,
            end: 4,
            capacity: 1.0,
            rate: 2.0,
        },
        Window {
            start: 20,
            end: p.horizon,
            capacity: 1.0,
            rate: 1.0,
        },
    ];
    campaign(&mut p, "productive_time", 10.0);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(seq(&s), vec!["T0", "T2", "T1"]);
    campaign(&mut p, "elapsed_time", 10.0);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(seq(&s), vec!["T0", "T1", "T2"]);
    campaign(&mut p, "work", 10.0);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(seq(&s), vec!["T0", "T1", "T2"]);
}
#[test]
fn previous_post_is_included_in_prospective_occupied_campaign_and_idle_gap() {
    let mut p = problem(3);
    p.tasks[1].family = "B".into();
    p.tasks[0].due = Some(0);
    p.rules=serde_json::from_value(json!([{"kind":"sequence_pattern","id":"clean","resource":"M0","pattern":["A","B"],"previous_post":[{"id":"wash","modes":[demo::mode("M0",20.0)]}]}])).unwrap();
    campaign(&mut p, "occupied_time", 25.0);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(seq(&s), vec!["T0", "T1", "T2"]);
    assert_eq!(s.assignments[1].start, 30);
    campaign(&mut p, "productive_time", 25.0);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(seq(&s), vec!["T0", "T2", "T1"]);
}
#[test]
fn material_availability_is_considered_before_campaign_filtering() {
    let mut p = problem(2);
    p.tasks[0].family = "B".into();
    p.tasks[1].consume.insert("raw".into(), 1.0);
    p.receipts.push(Receipt {
        item: "raw".into(),
        at: 200,
        amount: 1.0,
    });
    campaign(&mut p, "work", 20.0);
    p.planning.policies.insert(0,serde_json::from_value(json!({"kind":"idle_gap","id":"gap","resource":"M0","maximum":20,"fallback_to_smallest":true})).unwrap());
    let s = xg::create(&p, &o()).unwrap();
    assert_eq!(seq(&s), vec!["T0", "T1"]);
    assert_eq!(s.assignments[1].start, 200);
}
#[test]
fn urgent_work_has_only_the_explicit_campaign_exception() {
    let mut p = problem(2);
    p.tasks[0].family = "B".into();
    p.tasks[0].due = Some(5);
    campaign(&mut p, "work", 20.0);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(seq(&s)[0], "T1");
    set(&mut p, "urgent_slack", json!(0));
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(seq(&s)[0], "T0");
}
#[test]
fn every_strategy_and_both_searches_share_enforcement_and_parallel_replay() {
    let mut p = problem(5);
    p.tasks[0].family = "B".into();
    campaign(&mut p, "productive_time", 20.0);
    for strategy in [
        "due",
        "shortest",
        "priority",
        "release",
        "objective",
        "random",
        "setup",
        "queues",
    ] {
        let s = xg::create(
            &p,
            &Options {
                strategy: strategy.into(),
                ..o()
            },
        )
        .unwrap();
        assert_ne!(seq(&s)[0], "T0");
        assert!(validate::validate(&p, &s).valid);
    }
    let mut options = o();
    options.xh.workers = 2;
    options.xt.workers = 2;
    let a = apex::xh::search_with(&p, &options, None).unwrap();
    assert!(a.search.as_ref().unwrap().generations > 0);
    assert!(a.search.as_ref().unwrap().mutations > 0);
    let mut serial = options.clone();
    serial.xh.workers = 1;
    let b = apex::xh::search_with(&p, &serial, None).unwrap();
    assert_eq!(a.score, b.score);
    assert_eq!(seq(&a), seq(&b));
    let s = apex::xt::search(&p, &options).unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert_ne!(seq(&s)[0], "T0");
}
#[test]
fn missing_corrupt_and_foreign_witnesses_are_rejected() {
    let mut p = problem(3);
    p.tasks[0].family = "B".into();
    campaign(&mut p, "work", 20.0);
    let s = xg::create(&p, &o()).unwrap();
    let mut bad = s.clone();
    bad.dispatch = None;
    assert!(!validate::validate(&p, &bad).valid);
    let mut bad = s.clone();
    bad.dispatch.as_mut().unwrap().decisions.swap(0, 2);
    assert!(!validate::validate(&p, &bad).valid);
    set(&mut p, "minimum", json!(30));
    assert!(!validate::validate(&p, &s).valid);
}
#[test]
fn declarative_constraints_and_completion_objective_equal_explicit_model() {
    let mut p = problem(4);
    p.tasks[0].modes.push(demo::mode("M1", 2.0));
    p.planning=serde_json::from_value(json!({"constraints":[{"kind":"eligible_resources","id":"machine","select":{"tasks":["T0"]},"resources":["M0"]},{"kind":"window","id":"window","select":{"tasks":["T1"]},"earliest":30,"latest":1000},{"kind":"sequence","id":"order","resource":"M0","tasks":["T0","T1"],"consecutive":true}],"objectives":[{"id":"selected_completion","select":{"families":["A"]},"attribute":"urgency","weight":2}]})).unwrap();
    let lowered = apex::language::lower(&p).unwrap();
    let a = xg::create(&p, &o()).unwrap();
    let b = xg::create(&lowered, &o()).unwrap();
    assert_eq!(a.metrics, b.metrics);
    assert_eq!(
        serde_json::to_value(a.assignments).unwrap(),
        serde_json::to_value(b.assignments).unwrap()
    );
    let defs = apex::queues::definitions(&lowered, None);
    assert!(defs.iter().any(|q| q.id == "attribute:selected_completion"));
    let s = apex::xh::search_with(&p, &o(), None).unwrap();
    assert!(validate::validate(&p, &s).valid);
    p.tasks[0].attributes.clear();
    assert!(apex::compile::compile(&p).is_err());
}
struct RejectFirst;
impl Customization for RejectFirst {
    fn id(&self) -> &str {
        "synthetic-filter"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, p: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        Ok(p.clone())
    }
    fn has_dispatch_policy(&self) -> bool {
        true
    }
    fn filter_candidates(
        &self,
        c: &policy::Context<'_>,
    ) -> Result<Vec<policy::Rejection>, Diagnostic> {
        Ok(c.candidates
            .iter()
            .filter(|v| c.prefix.order.is_empty() && v.task == 0)
            .map(|v| policy::Rejection {
                task: v.task,
                mode: v.mode,
                reason: "Synthetic rule excludes the first input task at the first decision".into(),
            })
            .collect())
    }
}
#[test]
fn native_filter_gets_full_pool_and_cannot_be_bypassed_by_forced_decisions() {
    let p = problem(3);
    let (_, s) = xg::create_customized(&p, &o(), &RejectFirst).unwrap();
    assert_ne!(seq(&s)[0], "T0");
    assert!(apex::validate::validate_customized(&p, &s, &RejectFirst).valid);
    let mut options = o();
    options.decision_prefix = vec![Decision {
        task: "T0".into(),
        mode: "on-M0".into(),
    }];
    assert!(xg::create_customized(&p, &options, &RejectFirst).is_err());
    assert!(apex::xh::search_customized(&p, &o(), &RejectFirst).is_ok());
}
#[test]
fn exact_rules_reject_extensions_without_a_prefix_decoration_contract() {
    let mut p = problem(2);
    campaign(&mut p, "work", 20.0);
    let errors = xg::create_customized(&p, &o(), &RejectFirst).unwrap_err();
    assert_eq!(errors[0].code, "DISPATCH_EXTENSION");
}

struct PrefixSafe;
impl Customization for PrefixSafe {
    fn id(&self) -> &str {
        "synthetic-prefix-safe"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, p: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        Ok(p.clone())
    }
    fn supports_prefix_decoration(&self) -> bool {
        true
    }
}
#[test]
fn direct_calendar_probe_matches_full_decoder_with_breaks_rates_locks_and_modes() {
    for seed in 0..12 {
        let mut p = problem(9);
        for (i, t) in p.tasks.iter_mut().enumerate() {
            t.family = if i % 3 == 0 { "B" } else { "A" }.into();
            t.release = ((i * 7 + seed * 13) % 21) as i64;
            t.modes = vec![
                demo::mode("M0", (3 + (i * 11 + seed) % 23) as f64),
                demo::mode("M1", (5 + (i * 7 + seed) % 29) as f64),
            ];
        }
        p.resources[0].calendar = vec![
            Window {
                start: 0,
                end: 25,
                capacity: 1.0,
                rate: 2.0,
            },
            Window {
                start: 40,
                end: p.horizon,
                capacity: 1.0,
                rate: 0.7,
            },
        ];
        campaign(&mut p, "occupied_time", 50.0);
        let a = xg::create(&p, &o()).unwrap();
        let (_, b) = xg::create_customized(&p, &o(), &PrefixSafe).unwrap();
        assert_eq!(seq(&a), seq(&b));
        assert_eq!(
            serde_json::to_value(&a.assignments).unwrap(),
            serde_json::to_value(&b.assignments).unwrap()
        );
        assert_eq!(a.dispatch.as_ref().unwrap().exact_probes, 0);
        assert!(b.dispatch.as_ref().unwrap().exact_probes > 0);
    }
}
#[test]
fn custom_filter_is_enforced_in_xt_and_corrupted_explanations_are_rejected() {
    let p = problem(3);
    let s = apex::xt::search_customized(&p, &o(), Some(&RejectFirst)).unwrap();
    assert_ne!(seq(&s)[0], "T0");
    assert!(validate::validate_customized(&p, &s, &RejectFirst).valid);
    let mut bad = s.clone();
    bad.dispatch.as_mut().unwrap().steps[0].reasons[0].message = "Forged explanation".into();
    assert!(!validate::validate_customized(&p, &bad, &RejectFirst).valid);
    let mut bad = s;
    bad.dispatch.as_mut().unwrap().options.strategy = "weighted".into();
    assert!(!validate::validate_customized(&p, &bad, &RejectFirst).valid);
}
#[test]
fn xt_can_expand_a_valid_prefix_even_when_its_greedy_suffix_fails() {
    let mut p = problem(3);
    p.tasks[0].priority = 100.0;
    p.tasks[0].modes[0].phases[0].work = Some(100.0);
    p.tasks[1].deadline = Some(10);
    p.tasks[2].family = "B".into();
    campaign(&mut p, "work", 20.0);
    let mut options = o();
    options.iterations = 32;
    options.queue_policy = Some(
        serde_json::from_value(json!({"stages":{"*":{"priority":1}},"candidate_limit":64}))
            .unwrap(),
    );
    assert!(xg::create(&p, &options).is_err());
    let s = apex::xt::search(&p, &options).unwrap();
    assert_eq!(seq(&s)[0], "T1");
    assert!(validate::validate(&p, &s).valid);
}
#[test]
fn policy_counts_observed_and_remaining_running_work() {
    let mut p = problem(3);
    p.tasks[1].family = "B".into();
    p.tasks[0].due = Some(0);
    campaign(&mut p, "productive_time", 15.0);
    let original = xg::create(&p, &Options::default()).unwrap();
    let mut actual = original.assignments[0].activities[0].clone();
    actual.id = "actual".into();
    p.tasks[0].execution = Some(Execution {
        mode: "on-M0".into(),
        as_of: 10,
        actual,
        remaining_work: std::collections::BTreeMap::from([("run".into(), 5.0)]),
        restart: vec![],
    });
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(seq(&s), vec!["T0", "T1", "T2"]);
    assert_eq!(s.assignments[0].end, 15);
    assert!(validate::validate(&p, &s).valid);
}
#[test]
fn planning_versions_unknown_selectors_and_reserved_metric_names_are_rejected() {
    let mut p = problem(1);
    p.planning.version = "future".into();
    assert!(apex::compile::compile(&p).is_err());
    p.planning.version = "apex.planning.v1".into();
    p.planning.constraints=serde_json::from_value(json!([{"kind":"window","id":"typo","select":{"tasks":["missing"]},"earliest":0,"latest":100}])).unwrap();
    assert!(apex::compile::compile(&p).is_err());
    p.planning.constraints.clear();
    p.planning.objectives = serde_json::from_value(json!([{"id":"makespan","weight":1}])).unwrap();
    assert!(apex::compile::compile(&p).is_err());
    assert!(
        serde_json::from_value::<apex::language::PlanningModel>(
            json!({"policies":[{"kind":"eval","code":"arbitrary"}]})
        )
        .is_err()
    );
}

#[test]
fn future_terminal_cleanup_does_not_satisfy_a_current_campaign_minimum() {
    let mut p = problem(3);
    p.resources[0].initial_state = "A".into();
    p.tasks[1].family = "B".into();
    p.tasks[0].due = Some(0);
    for from in ["A", "B"] {
        for to in ["A", "B", "__end__"] {
            p.transitions.push(serde_json::from_value(json!({"id":format!("{from}-{to}"),"resource":"M0","from":from,"to":to,"previous_post":if to=="__end__" {json!([{"id":"terminal-wash","modes":[demo::mode("M0",1000.0)]}])} else {json!([])}})).unwrap());
        }
    }
    campaign(&mut p, "occupied_time", 100.0);
    let s = xg::create(&p, &Options::default()).unwrap();
    assert_eq!(seq(&s), vec!["T0", "T2", "T1"]);
    assert!(
        s.assignments[1]
            .activities
            .iter()
            .any(|a| a.id.contains("terminal-wash"))
    );
    assert!(validate::validate(&p, &s).valid);
}
#[test]
fn policies_resolve_routes_and_preserve_min_max_lag_and_sequence_commitments() {
    let mut p = problem(4);
    p.tasks[0].family = "B".into();
    p.tasks[1].family = "B".into();
    p.routes=serde_json::from_value(json!([{"id":"route","selected":"short","alternatives":[{"id":"short","tasks":["T0"]},{"id":"long","tasks":["T1"]}]}])).unwrap();
    p.dependencies =
        serde_json::from_value(json!([{"before":"T2","after":"T3","min_lag":5,"max_lag":10}]))
            .unwrap();
    p.locks.push(Lock::Order {
        resource: "M0".into(),
        tasks: vec!["T2".into(), "T3".into()],
        consecutive: true,
    });
    campaign(&mut p, "work", 20.0);
    let s = xg::create(&p, &o()).unwrap();
    assert_eq!(seq(&s), vec!["T2", "T3", "T0"]);
    assert_eq!(
        s.assignments.iter().find(|a| a.task == "T3").unwrap().start,
        15
    );
    assert!(validate::validate(&p, &s).valid);
}

#[test]
fn authored_queue_policy_can_name_a_generated_completion_proxy_before_lowering() {
    let mut p = problem(2);
    p.planning.objectives = serde_json::from_value(json!([{"id":"service","weight":1}])).unwrap();
    p.queue_policy =
        Some(serde_json::from_value(json!({"stages":{"*":{"attribute:service":1}}})).unwrap());
    assert!(apex::compile::compile(&p).is_ok());
    assert!(xg::create(&p, &o()).is_ok());
}

struct InvalidFilter;
impl Customization for InvalidFilter {
    fn id(&self) -> &str {
        "invalid-synthetic-filter"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, p: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        Ok(p.clone())
    }
    fn has_dispatch_policy(&self) -> bool {
        true
    }
    fn filter_candidates(
        &self,
        _: &policy::Context<'_>,
    ) -> Result<Vec<policy::Rejection>, Diagnostic> {
        Ok(vec![policy::Rejection {
            task: usize::MAX,
            mode: 0,
            reason: "Outside the supplied pool".into(),
        }])
    }
}
#[test]
fn native_hook_cannot_reject_an_unknown_pair() {
    let error = xg::create_customized(&problem(2), &o(), &InvalidFilter).unwrap_err();
    assert_eq!(error[0].code, "DISPATCH_HOOK");
}

#[test]
fn registered_native_metrics_and_sequence_decorations_compose_with_exact_policies() {
    let mut p = problem(3);
    p.tasks[1].family = "B".into();
    p.dependencies = vec![Dependency {
        before: "T1".into(),
        after: "T2".into(),
        min_lag: 0,
        max_lag: None,
    }];
    p.customization = Some(CustomizationRef {
        id: "dummy_customer".into(),
        version: "1".into(),
    });
    campaign(&mut p, "productive_time", 20.0);
    let s = xg::create(&p, &o()).unwrap();
    assert_eq!(seq(&s), vec!["T0", "T1", "T2"]);
    assert_eq!(s.metrics["setup_penalty"], 100.0);
    assert!(s.metrics.contains_key("dummy_priority_completion"));
    assert!(validate::validate(&p, &s).valid);
}
struct TimedFilter;
impl Customization for TimedFilter {
    fn id(&self) -> &str {
        "synthetic-timed-filter"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, p: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        Ok(p.clone())
    }
    fn has_dispatch_policy(&self) -> bool {
        true
    }
    fn dispatch_needs_placements(&self) -> bool {
        true
    }
    fn supports_prefix_decoration(&self) -> bool {
        true
    }
    fn filter_candidates(
        &self,
        c: &policy::Context<'_>,
    ) -> Result<Vec<policy::Rejection>, Diagnostic> {
        assert!(c.placements.iter().all(Option::is_some));
        Ok(c.candidates
            .iter()
            .zip(c.placements)
            .filter(|(_, f)| c.prefix.order.is_empty() && f.as_ref().unwrap().start > 20)
            .map(|(candidate, _)| policy::Rejection {
                task: candidate.task,
                mode: candidate.mode,
                reason: "First operation must be ready within 20 seconds including preparation"
                    .into(),
            })
            .collect())
    }
}
#[test]
fn native_hook_can_request_exact_preparation_facts() {
    let mut p = problem(2);
    p.tasks[0].pre = vec![Conditional {
        id: "prepare".into(),
        modes: vec![demo::mode("M1", 30.0)],
        ..Default::default()
    }];
    let (_, s) = xg::create_customized(&p, &o(), &TimedFilter).unwrap();
    assert_eq!(seq(&s), vec!["T1", "T0"]);
    assert!(validate::validate_customized(&p, &s, &TimedFilter).valid);
}
