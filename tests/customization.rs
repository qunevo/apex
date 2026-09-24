use apex::{demo, engine, model::*, rules::Customization, validate::validate};

struct SyntheticShiftPolicy;
impl Customization for SyntheticShiftPolicy {
    fn id(&self) -> &str {
        "synthetic-shift-policy"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, input: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        let mut p = input.clone();
        p.rules.push(Rule::TaskWindow {
            id: "release-after-handover".into(),
            task: p.tasks[0].id.clone(),
            earliest: 300,
            latest: p.horizon,
        });
        p.rules.push(Rule::AttributeObjective {
            id: "urgency_completion".into(),
            attribute: "urgency".into(),
            weight: 1.0,
            priority: 0,
        });
        Ok(p)
    }
}
#[test]
fn compiled_decorator_is_enforced_by_planner_trainer_and_validator() {
    let input = demo::problem(10);
    let (compiled, s) =
        engine::create_customized(&input, &Options::default(), &SyntheticShiftPolicy).unwrap();
    assert!(s.assignments[0].start >= 300);
    assert!(s.metrics.contains_key("urgency_completion"));
    assert!(input.rules.is_empty());
    let trained = engine::train(&compiled, &Options::default()).unwrap();
    assert!(trained.score <= s.score);
    let mut corrupt = trained;
    corrupt.assignments[0].start = 0;
    assert!(!validate(&compiled, &corrupt).valid);
}

struct SequenceInspectionPolicy {
    reject: bool,
}
impl Customization for SequenceInspectionPolicy {
    fn id(&self) -> &str {
        "synthetic-inspection"
    }
    fn version(&self) -> &str {
        "1"
    }
    fn compile(&self, input: &Problem) -> Result<Problem, Vec<Diagnostic>> {
        Ok(input.clone())
    }
    fn sequence(
        &self,
        context: &apex::extensions::SequenceContext<'_>,
    ) -> Result<Vec<apex::extensions::TaskDecoration>, Vec<Diagnostic>> {
        Ok(context
            .tasks
            .windows(2)
            .map(|pair| apex::extensions::TaskDecoration {
                task: pair[1].id.clone(),
                pre: vec![Conditional {
                    id: "context-inspection".into(),
                    modes: vec![demo::mode("M1", 15.0)],
                    owner_resource: Some("M0".into()),
                    ..Default::default()
                }],
                penalty: 7.0,
                ..Default::default()
            })
            .collect())
    }
    fn validate(&self, p: &Problem, _s: &Schedule) -> Vec<Diagnostic> {
        if self.reject {
            vec![Diagnostic::new(
                "CUSTOM_POLICY",
                &p.id,
                "Synthetic policy rejects this candidate",
            )]
        } else {
            vec![]
        }
    }
}

#[test]
fn native_sequence_activities_are_recreated_by_independent_validation() {
    let mut p = demo::problem(3);
    for t in &mut p.tasks {
        t.modes = vec![demo::mode("M0", 10.0)];
    }
    let policy = SequenceInspectionPolicy { reject: false };
    let (_, s) = engine::create_customized(&p, &Options::default(), &policy).unwrap();
    assert_eq!(s.metrics["setup_penalty"], 14.0);
    assert_eq!(
        s.assignments
            .iter()
            .map(|a| a.activities.len())
            .sum::<usize>(),
        5
    );
    assert!(apex::validate::validate_customized(&p, &s, &policy).valid);
    let trained = engine::train_customized(&p, &Options::default(), &policy).unwrap();
    assert!(apex::validate::validate_customized(&p, &trained, &policy).valid);
    let mut corrupt = trained;
    let assignment = corrupt
        .assignments
        .iter_mut()
        .find(|a| a.activities.len() > 1)
        .unwrap();
    assignment.activities.remove(0);
    assert!(!apex::validate::validate_customized(&p, &corrupt, &policy).valid);
}

#[test]
fn native_hard_validation_rejects_both_planning_modes_and_saved_results() {
    let p = demo::problem(1);
    let (_, s) = engine::create_customized(
        &p,
        &Options::default(),
        &SequenceInspectionPolicy { reject: false },
    )
    .unwrap();
    let policy = SequenceInspectionPolicy { reject: true };
    assert!(engine::create_customized(&p, &Options::default(), &policy).is_err());
    assert!(engine::train_customized(&p, &Options::default(), &policy).is_err());
    assert!(!apex::validate::validate_customized(&p, &s, &policy).valid);
}
