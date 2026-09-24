use apex::{demo, engine, improve, model::*, rules::Customization, validate};
use serde_json::json;

fn options() -> Options {
    Options {
        strategy: "queues".into(),
        iterations: 40,
        budget_ms: 0,
        improve: ImproveConfig {
            evolution_share: 0.3,
            ..Default::default()
        },
        trainer: TrainerConfig {
            population_size: 4,
            mutation_rate: 1.0,
            crossover_rate: 1.0,
            ..Default::default()
        },
        ..Default::default()
    }
}
#[test]
fn portfolio_shares_budget_hands_off_distinct_policies_and_preserves_incumbents() {
    let p = demo::problem(16);
    let o = options();
    let initial = engine::create(&p, &o).unwrap();
    let result = improve::improve(&p, &o).unwrap();
    assert!(result.score <= initial.score);
    assert!(validate::validate(&p, &result).valid);
    let r = result.search.as_ref().unwrap();
    assert_eq!(r.algorithm, "trainer_plus_ga");
    assert_eq!(r.evaluations, 40);
    assert_eq!(r.phases.iter().map(|s| s.evaluations).sum::<usize>(), 40);
    assert_eq!(r.phases[0].evaluations, 16);
    assert!(r.generations > 0 && r.mutations > 0 && r.crossovers > 0);
    assert!(r.phases.len() >= 3, "expected multiple policies: {r:?}");
    assert!(
        r.phases[1..r.phases.len() - 1]
            .iter()
            .all(|s| s.queue_policy.is_some())
    );
    let defaults = apex::queues::default_policy(&p, &apex::queues::definitions(&p, None));
    assert_eq!(
        r.phases[1].queue_policy.as_ref().unwrap().stages["*"],
        defaults.stages["*"]
    );
    for (i, a) in r.phases[1..r.phases.len() - 1].iter().enumerate() {
        for b in &r.phases[i + 2..r.phases.len() - 1] {
            assert!(a.queue_policy != b.queue_policy || a.route_choices != b.route_choices);
        }
    }
    for pair in r.progress.windows(2) {
        assert!(pair[1].score < pair[0].score);
        assert!(pair[1].evaluations > pair[0].evaluations);
        assert!(pair[1].elapsed_ms >= pair[0].elapsed_ms);
    }
    assert_eq!(r.progress.last().unwrap().score, result.score);
    let replay = engine::create(&p, result.replay.as_deref().unwrap()).unwrap();
    assert_eq!(replay.score, result.score);
    assert_eq!(
        serde_json::to_value(replay.assignments).unwrap(),
        serde_json::to_value(&result.assignments).unwrap()
    );
    let again = improve::improve(&p, &o).unwrap();
    assert_eq!(result.score, again.score);
    assert_eq!(
        serde_json::to_value(&result.assignments).unwrap(),
        serde_json::to_value(again.assignments).unwrap()
    );
}
#[test]
fn small_global_budgets_never_duplicate_or_exceed_evaluations() {
    let p = demo::problem(4);
    for n in 1..12 {
        let mut o = options();
        o.iterations = n;
        let s = improve::improve(&p, &o).unwrap();
        assert_eq!(s.evaluations, n);
        assert!(validate::validate(&p, &s).valid);
    }
}
#[test]
fn supplied_incumbent_survives_and_corruption_or_conflicting_options_fail() {
    let p = demo::problem(8);
    let mut o = options();
    let s = improve::improve(&p, &o).unwrap();
    o.iterations = 1;
    let improved = improve::improve_from(&p, &o, None, Some(&s)).unwrap();
    assert!(improved.score <= s.score);
    assert_eq!(improved.search.as_ref().unwrap().progress[0].evaluations, 0);
    let mut corrupt = s.clone();
    corrupt.score[0] -= 10000.0;
    assert!(improve::improve_from(&p, &o, None, Some(&corrupt)).is_err());
    o.mode_choices
        .insert(p.tasks[0].id.clone(), "not-the-mode".into());
    assert!(improve::improve_from(&p, &o, None, Some(&s)).is_err());
}
#[test]
fn campaign_and_fixed_start_hold_through_every_phase_and_parallel_workers() {
    let p: Problem =
        serde_json::from_str(include_str!("../examples/dispatch-campaign.json")).unwrap();
    let mut o = options();
    o.trainer.workers = 2;
    o.plus.workers = 2;
    o.trainer.selection = "pareto".into();
    let s = improve::improve(&p, &o).unwrap();
    assert_eq!(s.evaluations, o.iterations);
    assert!(validate::validate(&p, &s).valid);
    assert!(s.dispatch.is_some());
    for candidate in &s.search.as_ref().unwrap().archive {
        let replay = engine::create(&p, &candidate.options).unwrap();
        assert!(validate::validate(&p, &replay).valid);
        assert_eq!(candidate.score, replay.score);
    }
}
#[test]
fn invalid_combined_limits_and_configurations_fail_explicitly() {
    let p = demo::problem(3);
    for value in [0.0, 1.0, -1.0, f64::NAN] {
        let mut o = options();
        o.improve.trainer_share = value;
        assert!(improve::improve(&p, &o).is_err());
    }
    let mut o = options();
    o.iterations = 0;
    o.trainer.generations = Some(1);
    assert!(improve::improve(&p, &o).is_err());
    o.iterations = 10;
    o.improve.policies = 0;
    assert!(improve::improve(&p, &o).is_err());
}
#[test]
fn declarative_goal_and_route_choices_are_replayable() {
    let input = serde_json::from_str(include_str!("../examples/production-orders.json")).unwrap();
    let mut p = apex::production::expand(input).unwrap();
    p.planning =
        serde_json::from_value(json!({"objectives":[{"id":"completion","weight":1,"priority":0}]}))
            .unwrap();
    let mut o = options();
    let route = &p.routes[0];
    o.route_choices
        .insert(route.id.clone(), route.alternatives[0].id.clone());
    let s = improve::improve(&p, &o).unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert_eq!(s.route_choices[&route.id], route.alternatives[0].id);
    let replay = engine::create(&p, s.replay.as_deref().unwrap()).unwrap();
    assert_eq!(replay.score, s.score);
}
struct Slow;
impl Customization for Slow {
    fn id(&self) -> &str {
        "slow-test"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, p: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        std::thread::sleep(std::time::Duration::from_millis(5));
        Ok(p.clone())
    }
}
#[test]
fn soft_global_deadline_does_not_restart_a_fresh_budget_for_plus() {
    let p = demo::problem(3);
    let mut o = options();
    o.budget_ms = 1;
    o.iterations = 0;
    let s = match improve::improve_from(&p, &o, Some(&Slow), None) {
        Ok(s) => s,
        // Under load the deadline can expire during setup, before evaluation 1.
        Err(errors) => {
            assert_eq!(errors[0].code, "IMPROVE_BUDGET");
            return;
        }
    };
    assert_eq!(s.evaluations, 1);
    assert_eq!(s.search.as_ref().unwrap().phases.len(), 1);
    assert_eq!(s.search.as_ref().unwrap().stop_reason, "time_limit");
}
#[test]
fn native_customization_and_mandatory_filters_remain_active() {
    let mut p = demo::problem(8);
    for (i, t) in p.tasks.iter_mut().enumerate() {
        t.family = if i % 2 == 0 { "A" } else { "B" }.into();
    }
    p.customization = Some(CustomizationRef {
        id: "dummy_customer".into(),
        version: "1".into(),
    });
    p.planning = serde_json::from_value(json!({"policies":[{"kind":"campaign","id":"a","resource":"M0","basis":"work","minimum":120,"on_no_match":"allow_switch"}]})).unwrap();
    let s = improve::improve(&p, &options()).unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert!(s.dispatch.is_some());
}

#[test]
fn plus_recovers_when_the_entire_training_phase_is_infeasible() {
    let mut p = demo::problem(3);
    for t in &mut p.tasks {
        t.modes = vec![demo::mode("M0", 5.0)];
        t.stage = "0".into();
    }
    p.tasks[0].priority = 100.0;
    p.tasks[0].modes[0].phases[0].work = Some(100.0);
    p.tasks[1].deadline = Some(10);
    let mut o = options();
    o.improve.trainer_share = 0.05;
    o.queue_policy = Some(serde_json::from_value(json!({"stages":{"*":{"priority":1}}})).unwrap());
    assert!(engine::create(&p, &o).is_err());
    let s = improve::improve(&p, &o).unwrap();
    let r = s.search.as_ref().unwrap();
    assert_eq!(r.phases[0].failed_evaluations, 2);
    assert!(r.evaluations <= o.iterations);
    assert_eq!(
        r.evaluations,
        r.phases.iter().map(|p| p.evaluations).sum::<usize>()
    );
    // The three-task GA space can run out of new chromosomes before its cap.
    assert_eq!(r.phases.last().unwrap().stop_reason, "no_new_candidates");
    assert!(
        r.phases
            .iter()
            .any(|p| p.name.starts_with("plus") && p.best_score.is_some())
    );
    assert!(validate::validate(&p, &s).valid);
}
