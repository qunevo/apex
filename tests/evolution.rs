use apex::{
    demo, engine, evolution, improve, material, model::*, rules::Customization, search, validate,
};
use serde_json::json;

fn options() -> Options {
    Options {
        strategy: "queues".into(),
        iterations: 128,
        budget_ms: 0,
        improve: ImproveConfig {
            evolution_share: 0.3,
            ..Default::default()
        },
        trainer: TrainerConfig {
            population_size: 8,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn problem(n: usize) -> Problem {
    let mut p = demo::problem(n);
    p.dependencies.clear();
    for t in &mut p.tasks {
        t.modes = vec![demo::mode("M0", 10.0)];
        t.due = Some(100);
        t.release = 0;
    }
    p
}
fn same(a: &Schedule, b: &Schedule) {
    assert_eq!(a.score, b.score);
    assert_eq!(
        serde_json::to_value(&a.assignments).unwrap(),
        serde_json::to_value(&b.assignments).unwrap()
    );
}

#[test]
fn direct_priorities_cross_stages_but_repair_precedence_and_reject_unknown_genes() {
    let mut p = problem(3);
    p.tasks[0].stage = "0".into();
    p.tasks[1].stage = "2".into();
    p.tasks[2].stage = "1".into();
    p.dependencies =
        serde_json::from_value(json!([{"before":"T000000","after":"T000002"}])).unwrap();
    let mut o = options();
    o.decision_order = vec!["T000002".into(), "T000001".into(), "T000000".into()];
    let s = engine::create(&p, &o).unwrap();
    assert_eq!(
        s.construction
            .iter()
            .map(|d| d.task.as_str())
            .collect::<Vec<_>>(),
        vec!["T000001", "T000000", "T000002"]
    );
    assert!(validate::validate(&p, &s).valid);
    o.decision_prefix = vec![Decision {
        task: "T000002".into(),
        mode: "on-M0".into(),
    }];
    assert!(engine::create(&p, &o).is_err());
    o.decision_prefix.clear();
    o.decision_order.push("missing".into());
    assert!(engine::create(&p, &o).is_err());
    o.decision_order = vec!["T000000".into(); 2];
    assert!(engine::create(&p, &o).is_err());
    let mut bad = s;
    bad.construction[0].mode = "missing".into();
    assert!(!validate::validate(&p, &bad).valid);
}

#[test]
fn running_actuals_restart_choices_and_input_locks_remain_immutable() {
    let mut p = problem(2);
    p.tasks[0].execution = Some(Execution {
        mode: "on-M0".into(),
        as_of: 5,
        actual: Activity {
            id: "actual".into(),
            task: "T000000".into(),
            role: "actual".into(),
            mode: "on-M0".into(),
            start: 0,
            end: 5,
            segments: vec![Segment {
                phase: "run".into(),
                start: 0,
                end: 5,
                work: 5.0,
            }],
            reservations: vec![Reservation {
                resource: "M0".into(),
                start: 0,
                end: 5,
                amount: 1.0,
            }],
        },
        remaining_work: [("run".into(), 5.0)].into(),
        restart: vec![Conditional {
            id: "restart".into(),
            modes: vec![demo::mode("M1", 2.0), demo::mode("M2", 3.0)],
            ..Default::default()
        }],
    });
    p.tasks[0]
        .conditional_modes
        .insert("restart:restart".into(), "on-M2".into());
    p.tasks[1].modes.push(demo::mode("M1", 1.0));
    let mut o = options();
    o.mode_choices.insert("T000001".into(), "on-M0".into());
    let s = evolution::evolve(&p, &o).unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert_eq!(
        serde_json::to_value(&s.assignments[0].activities[0]).unwrap(),
        serde_json::to_value(&p.tasks[0].execution.as_ref().unwrap().actual).unwrap()
    );
    assert!(
        s.assignments[0]
            .activities
            .iter()
            .any(|a| a.id.ends_with("restart:restart") && a.mode == "on-M2")
    );
    let mut corrupt = s.clone();
    corrupt
        .replay
        .as_mut()
        .unwrap()
        .mode_choices
        .insert("T000001".into(), "on-M1".into());
    assert!(!validate::validate(&p, &corrupt).valid);
    let refined = evolution::evolve_from(&p, &o, &s).unwrap();
    assert!(refined.score <= s.score);
    o.mode_choices.insert("T000001".into(), "on-M1".into());
    assert!(evolution::evolve_from(&p, &o, &s).is_err());
}
#[test]
fn ga_replays_is_parallel_deterministic_and_never_loses_its_initial_plan() {
    let p = demo::problem(18);
    let mut o = options();
    let initial = engine::create(&p, &o).unwrap();
    let a = evolution::evolve(&p, &o).unwrap();
    assert!(a.score <= initial.score);
    assert!(validate::validate(&p, &a).valid);
    same(&a, &engine::create(&p, a.replay.as_ref().unwrap()).unwrap());
    o.trainer.workers = 4;
    let b = evolution::evolve(&p, &o).unwrap();
    same(&a, &b);
    let ra = a.search.unwrap();
    let rb = b.search.unwrap();
    assert_eq!(ra.evaluations, 128);
    assert!(ra.mutations > 0 && ra.crossovers > 0);
    assert_eq!(
        ra.operators.iter().map(|r| r.attempts).collect::<Vec<_>>(),
        rb.operators.iter().map(|r| r.attempts).collect::<Vec<_>>()
    );
    assert_eq!(
        ra.operators.iter().map(|r| r.reward).collect::<Vec<_>>(),
        rb.operators.iter().map(|r| r.reward).collect::<Vec<_>>()
    );
    assert_eq!(
        ra.operators.iter().map(|r| r.skipped).collect::<Vec<_>>(),
        rb.operators.iter().map(|r| r.skipped).collect::<Vec<_>>()
    );
    assert_eq!(ra.operators.iter().map(|r| r.attempts).sum::<usize>(), 120);
}
#[test]
fn crossover_transfers_parent_orders_and_choices_without_duplicate_operations() {
    let p = problem(12);
    let a = evolution::Genome {
        order: p.tasks.iter().map(|t| t.id.clone()).collect(),
        modes: [("T000001".into(), "first".into())].into(),
        ..Default::default()
    };
    let mut b = a.clone();
    b.order.reverse();
    b.modes.insert("T000001".into(), "second".into());
    let mut recombinant = false;
    let mut donor_mode = false;
    for seed in 0..30 {
        let c = evolution::crossover(&p, &a, &b, &mut search::Random(seed));
        let mut ids = c.order.clone();
        ids.sort();
        assert_eq!(ids, a.order);
        recombinant |= c.order != a.order && c.order != b.order;
        donor_mode |= c.modes == b.modes;
    }
    assert!(recombinant && donor_mode);
}
#[test]
fn fixed_budgets_generation_bounds_and_pareto_archive_hold() {
    let p = demo::problem(8);
    let mut o = options();
    o.trainer.selection = "pareto".into();
    for n in [1, 2, 7, 17, 33] {
        o.iterations = n;
        let s = evolution::evolve(&p, &o).unwrap();
        assert_eq!(s.evaluations, n);
        assert!(validate::validate(&p, &s).valid);
    }
    o.iterations = 0;
    o.trainer.generations = Some(2);
    let s = evolution::evolve(&p, &o).unwrap();
    let r = s.search.unwrap();
    assert_eq!(r.generations, 2);
    assert_eq!(r.evaluations, 16);
    for a in &r.archive {
        for b in &r.archive {
            assert!(!search::dominates(&a.objectives, &b.objectives));
        }
        same(
            &engine::create(&p, &a.options).unwrap(),
            &engine::create(&p, &a.options).unwrap(),
        );
    }
    o.trainer.generations = None;
    assert!(evolution::evolve(&p, &o).is_err());
    o.iterations = 10;
    o.evolution.discount = 0.0;
    assert!(evolution::evolve(&p, &o).is_err());
}
#[test]
fn combined_search_reserves_ga_inside_shared_budget_and_keeps_external_incumbent() {
    assert_eq!(Options::default().improve.evolution_share, 0.0);
    let p = demo::problem(12);
    let o = options();
    let initial = engine::create(&p, &o).unwrap();
    let s = improve::improve_from(&p, &o, None, Some(&initial)).unwrap();
    let r = s.search.as_ref().unwrap();
    assert_eq!(r.algorithm, "trainer_plus_ga");
    assert_eq!(r.evaluations, 128);
    assert_eq!(r.phases.iter().map(|p| p.evaluations).sum::<usize>(), 128);
    assert_eq!(r.phases.last().unwrap().name, "direct_ga");
    assert!(r.phases.last().unwrap().evaluations >= 38);
    assert!(s.score <= initial.score);
    assert!(!r.operators.is_empty());
    let mut disabled = o.clone();
    disabled.improve.evolution_share = 0.0;
    let old = improve::improve(&p, &disabled).unwrap();
    assert_eq!(old.search.unwrap().algorithm, "trainer_plus_portfolio");
}
#[test]
fn main_material_modes_are_reallocated_and_all_searches_can_recover_from_shortage() {
    let mut p = problem(1);
    p.inventory.insert("available".into(), 1.0);
    p.tasks[0].modes[0].consume = Some([("absent".into(), 1.0)].into());
    let mut other = demo::mode("M1", 20.0);
    other.consume = Some([("available".into(), 1.0)].into());
    p.tasks[0].modes.push(other);
    let (prepared, preview) = material::prepare(&p, &Default::default()).unwrap();
    assert!(preview.preview);
    assert!(!preview.preview_diagnostics.is_empty());
    assert_eq!(prepared.tasks[0].modes.len(), 2);
    let o = options();
    assert!(engine::create(&prepared, &o).is_err());
    for s in [
        evolution::evolve(&prepared, &o).unwrap(),
        search::train(&prepared, &o, None).unwrap(),
        search::plus(&prepared, &o).unwrap(),
        improve::improve(&prepared, &o).unwrap(),
    ] {
        assert_eq!(s.assignments[0].mode, "on-M1");
        assert!(validate::validate(&prepared, &s).valid);
        same(
            &s,
            &engine::create(&prepared, s.replay.as_ref().unwrap()).unwrap(),
        );
        let mut bad = s.clone();
        bad.material_report.as_mut().unwrap().allocations[0].quantity = 999.0;
        assert!(!validate::validate(&prepared, &bad).valid);
    }
    let mut pinned = o.clone();
    pinned.mode_choices.insert("T000000".into(), "on-M0".into());
    assert!(evolution::evolve(&prepared, &pinned).is_err());
}
struct Native {
    bad: bool,
    omit: bool,
}
impl Customization for Native {
    fn id(&self) -> &str {
        "synthetic-evolution"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, p: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        Ok(p.clone())
    }
    fn evolution_operators(&self) -> Vec<evolution::OperatorDefinition> {
        vec![evolution::OperatorDefinition {
            id: "reverse".into(),
            kind: evolution::OperatorKind::Mutation,
        }]
    }
    fn evolve(
        &self,
        c: &evolution::OperatorContext<'_>,
    ) -> Result<Option<evolution::Genome>, Diagnostic> {
        let mut g = c.parent.clone();
        g.order.reverse();
        if self.bad {
            g.order.push("unknown".into());
        }
        Ok(Some(g))
    }
    fn conditional_modes(&self, _: &Problem) -> apex::conditionals::Catalog {
        [(
            "T000000".into(),
            [(
                "pre:inspection".into(),
                vec!["on-M1".into(), "on-M2".into()],
            )]
            .into(),
        )]
        .into()
    }
    fn sequence(
        &self,
        c: &apex::extensions::SequenceContext<'_>,
    ) -> Result<Vec<apex::extensions::TaskDecoration>, Vec<Diagnostic>> {
        if self.omit || !c.tasks.iter().any(|t| t.id == "T000000") {
            return Ok(vec![]);
        }
        Ok(vec![apex::extensions::TaskDecoration {
            task: "T000000".into(),
            pre: vec![Conditional {
                id: "inspection".into(),
                modes: vec![demo::mode("M1", 10.0), demo::mode("M2", 12.0)],
                ..Default::default()
            }],
            ..Default::default()
        }])
    }
    fn supports_prefix_decoration(&self) -> bool {
        true
    }
}
#[test]
fn sequence_generated_alternatives_are_searchable_and_missing_or_corrupt_activities_fail() {
    let mut p = problem(2);
    p.tasks[0].modes = vec![demo::mode("M0", 1.0)];
    p.tasks[1].modes = vec![demo::mode("M1", 100.0)];
    p.objectives = vec![Objective {
        metric: "makespan".into(),
        ..Default::default()
    }];
    let native = Native {
        bad: false,
        omit: false,
    };
    let mut o = options();
    o.decision_prefix = vec![Decision {
        task: "T000000".into(),
        mode: "on-M0".into(),
    }];
    for s in [
        evolution::evolve_customized(&p, &o, &native).unwrap(),
        search::train(&p, &o, Some(&native)).unwrap(),
        search::plus_customized(&p, &o, Some(&native)).unwrap(),
    ] {
        assert_eq!(s.metrics["makespan"], 100.0);
        assert!(validate::validate_customized(&p, &s, &native).valid);
        assert_eq!(s.assignments[0].activities[0].mode, "on-M2");
        let mut corrupt = s;
        corrupt.assignments[0].activities.remove(0);
        assert!(!validate::validate_customized(&p, &corrupt, &native).valid);
    }
    o.conditional_choices.insert(
        "T000000".into(),
        [("pre:inspection".into(), "on-M2".into())].into(),
    );
    assert!(
        engine::create_customized(
            &p,
            &o,
            &Native {
                bad: false,
                omit: true
            }
        )
        .is_err()
    );
}
#[test]
fn invalid_native_operator_is_counted_and_cannot_discard_incumbents() {
    let p = problem(4);
    let o = options();
    let native = Native {
        bad: true,
        omit: false,
    };
    let s = evolution::evolve_customized(&p, &o, &native).unwrap();
    assert!(validate::validate_customized(&p, &s, &native).valid);
    let arm = s
        .search
        .unwrap()
        .operators
        .into_iter()
        .find(|r| r.id == "synthetic-evolution:reverse@1")
        .unwrap();
    assert!(arm.attempts > 0);
    assert_eq!(arm.attempts, arm.rejected);
    assert_eq!(arm.reward, 0.0);
}
#[test]
fn commitments_breaks_max_lag_and_consecutive_sequences_survive_direct_variation() {
    let mut p = problem(5);
    p.resources[0].calendar = vec![
        Window {
            start: 0,
            end: 20,
            capacity: 1.0,
            rate: 1.0,
        },
        Window {
            start: 40,
            end: p.horizon,
            capacity: 1.0,
            rate: 1.0,
        },
    ];
    for t in &mut p.tasks {
        t.modes[0].phases[0].interruption = Interrupt::CalendarResumable;
    }
    p.locks=serde_json::from_value(json!([{"kind":"start","task":"T000000","at":0},{"kind":"order","resource":"M0","tasks":["T000001","T000002"],"consecutive":true}])).unwrap();
    p.dependencies = serde_json::from_value(
        json!([{"before":"T000001","after":"T000002","min_lag":0,"max_lag":40}]),
    )
    .unwrap();
    let mut o = options();
    o.mode_choices.insert("T000003".into(), "on-M0".into());
    let s = evolution::evolve(&p, &o).unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert_eq!(s.assignments[0].start, 0);
    assert_eq!(s.assignments[3].mode, "on-M0");
    let seq: Vec<_> = s.construction.iter().map(|d| d.task.as_str()).collect();
    let at = seq.iter().position(|&id| id == "T000001").unwrap();
    assert_eq!(seq[at + 1], "T000002");
}
#[test]
fn mandatory_policy_sees_candidate_beyond_window_under_direct_priorities() {
    let mut p = problem(70);
    for t in &mut p.tasks {
        t.family = "B".into();
    }
    p.tasks[69].family = "A".into();
    p.tasks[0].family = "A".into();
    p.resources[0].initial_state = "A".into();
    p.planning=serde_json::from_value(json!({"policies":[{"kind":"campaign","id":"campaign","resource":"M0","basis":"work","minimum":20,"on_no_match":"allow_switch"}]})).unwrap();
    let mut o = options();
    o.decision_prefix = vec![Decision {
        task: "T000000".into(),
        mode: "on-M0".into(),
    }];
    o.decision_order = p.tasks.iter().map(|t| t.id.clone()).collect();
    let s = engine::create(&p, &o).unwrap();
    assert_eq!(s.construction[1].task, "T000069");
    assert!(validate::validate(&p, &s).valid);
    o.decision_order.clear();
    o.iterations = 24;
    let s = evolution::evolve(&p, &o).unwrap();
    assert_eq!(s.construction[1].task, "T000069");
    assert!(validate::validate(&p, &s).valid);
}

#[test]
fn ga_checks_rich_synthetic_shift_route_material_and_conditional_fixtures() {
    let mut models: Vec<Problem> = vec![
        serde_json::from_str(include_str!("../examples/shift-factory.json")).unwrap(),
        serde_json::from_str(include_str!("../examples/chain-routing.json")).unwrap(),
        serde_json::from_str(include_str!("../examples/dispatch-campaign.json")).unwrap(),
        apex::production::expand(
            serde_json::from_str(include_str!("../examples/production-orders.json")).unwrap(),
        )
        .unwrap(),
    ];
    let (prepared, _) = material::prepare(&models[1], &Default::default()).unwrap();
    models.push(prepared);
    let mut o = options();
    o.iterations = 48;
    for p in models {
        let s = evolution::evolve(&p, &o).unwrap_or_else(|e| panic!("{}: {e:?}", p.id));
        assert!(validate::validate(&p, &s).valid, "{}", p.id);
        same(&s, &engine::create(&p, s.replay.as_ref().unwrap()).unwrap());
    }
}

#[test]
fn irrelevant_operators_are_never_proposed_and_skips_do_not_spend_evaluations() {
    let mut p = problem(12);
    for t in &mut p.tasks {
        t.due = None;
    }
    let s = evolution::evolve(&p, &options()).unwrap();
    assert!(validate::validate(&p, &s).valid);
    let r = s.search.unwrap();
    assert_eq!(r.evaluations, 128);
    assert!(r.operators.iter().map(|o| o.skipped).sum::<usize>() > 0);
    for name in [
        "tardy_insert",
        "main_mode",
        "route",
        "conditional_mode",
        "conditional_reset",
    ] {
        let op = r
            .operators
            .iter()
            .find(|o| o.id == format!("apex:{name}@2"))
            .unwrap();
        assert_eq!(op.attempts + op.skipped, 0, "{name}");
    }
    assert!(r.operators.iter().all(|o| o.unchanged == 0));
    assert_eq!(r.operators.iter().map(|o| o.attempts).sum::<usize>(), 120);
}

struct NoProposal;
impl Customization for NoProposal {
    fn id(&self) -> &str {
        "synthetic-no-proposal"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, p: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        Ok(p.clone())
    }
    fn evolution_operators(&self) -> Vec<evolution::OperatorDefinition> {
        vec![evolution::OperatorDefinition {
            id: "none".into(),
            kind: evolution::OperatorKind::Mutation,
        }]
    }
    fn evolve(
        &self,
        _: &evolution::OperatorContext<'_>,
    ) -> Result<Option<evolution::Genome>, Diagnostic> {
        Ok(None)
    }
}

#[test]
fn exhausted_spaces_and_noop_hooks_terminate_without_discarding_incumbent() {
    let p = problem(1);
    let o = options();
    for native in [false, true] {
        let s = if native {
            evolution::evolve_customized(&p, &o, &NoProposal)
        } else {
            evolution::evolve(&p, &o)
        }
        .unwrap();
        assert!(validate::validate(&p, &s).valid);
        same(&s, &engine::create(&p, s.replay.as_ref().unwrap()).unwrap());
        let r = s.search.unwrap();
        assert_eq!(r.evaluations, 8);
        assert_eq!(r.stop_reason, "no_new_candidates");
        assert!(r.operators.iter().all(|o| o.attempts == 0));
        if native {
            assert_eq!(r.operators.last().unwrap().skipped, 8 * 32);
        }
    }
}

#[test]
fn caller_pinned_modes_are_not_offered_as_mutations() {
    let mut p = problem(1);
    p.tasks[0].modes.push(demo::mode("M1", 1.0));
    let mut o = options();
    o.mode_choices.insert(p.tasks[0].id.clone(), "on-M0".into());
    let s = evolution::evolve(&p, &o).unwrap();
    assert!(validate::validate(&p, &s).valid);
    assert_eq!(s.assignments[0].mode, "on-M0");
    let r = s.search.unwrap();
    let op = r
        .operators
        .iter()
        .find(|v| v.id == "apex:main_mode@2")
        .unwrap();
    assert_eq!(op.attempts + op.skipped, 0);
}

#[test]
fn independent_population_override_and_invalid_new_settings_are_checked() {
    let p = demo::problem(18);
    let mut o = options();
    o.evolution.population_size = Some(32);
    o.iterations = 33;
    let s = evolution::evolve(&p, &o).unwrap();
    assert_eq!(s.evaluations, 33);
    assert_eq!(
        s.search
            .unwrap()
            .operators
            .iter()
            .map(|v| v.attempts)
            .sum::<usize>(),
        1
    );
    assert_eq!(o.trainer.population_size, 8);
    for n in [0, 1, 257] {
        o.evolution.population_size = Some(n);
        assert!(evolution::evolve(&p, &o).is_err());
    }
    o.evolution.population_size = None;
    for value in [f64::NAN, -0.1, 1.1] {
        o.evolution.crossover_share = Some(value);
        assert!(evolution::evolve(&p, &o).is_err());
    }
    o.evolution.crossover_share = None;
    o.evolution.parent_selection = "unsupported".into();
    assert!(evolution::evolve(&p, &o).is_err());
}
