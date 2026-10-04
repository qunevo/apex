//! Shared placement state for full decoding and transactional construction probes.
use crate::{
    calendar::{self, Book},
    compile::Compiled,
    material_ledger::Ledger,
    model::*,
    rules,
    xg::append_stage,
};

type Specs<'a> = (
    Vec<(String, &'a Conditional)>,
    Vec<(String, &'a Conditional)>,
);

pub(crate) struct State {
    pub(crate) assignments: Vec<Option<Assignment>>,
    book: Book,
    tails: Vec<Time>,
    materials: Ledger,
}

impl State {
    /// Earlier placements are stable only without neighbor-dependent decoration.
    pub(crate) fn incremental(problem: &Problem) -> bool {
        problem.transitions.is_empty()
            && !problem
                .rules
                .iter()
                .any(|rule| matches!(rule, Rule::SequencePattern { .. }))
            && problem.tasks.iter().all(|task| {
                task.pre.is_empty()
                    && task.post.is_empty()
                    && task.execution.as_ref().is_none_or(|e| e.restart.is_empty())
                    && task
                        .modes
                        .iter()
                        .all(|mode| mode.pre.is_empty() && mode.post.is_empty())
            })
    }
    pub(crate) fn new(c: &Compiled<'_>) -> Self {
        let p = c.problem;
        let mut book = Book::new(p.resources.len());
        for t in &p.tasks {
            if let Some(e) = &t.execution {
                book.reservations(c, &e.actual.reservations, 1.0);
            }
        }
        Self {
            assignments: vec![None; p.tasks.len()],
            book,
            tails: vec![0; p.resources.len()],
            materials: Ledger::new(p),
        }
    }
    pub(crate) fn preview(
        &mut self,
        c: &Compiled<'_>,
        i: usize,
        mi: usize,
        specs: &Specs<'_>,
    ) -> Result<Assignment, Diagnostic> {
        self.book.begin();
        let result = self.place(c, i, mi, specs);
        self.book.rollback();
        result
    }
    pub(crate) fn commit(&mut self, c: &Compiled<'_>, i: usize, assignment: Assignment) {
        let t = &c.problem.tasks[i];
        let mode = t.modes.iter().find(|m| m.id == assignment.mode).unwrap();
        for activity in assignment.activities.iter().filter(|a| a.role != "actual") {
            self.book.reservations(c, &activity.reservations, 1.0);
        }
        self.book.reservations(c, &assignment.retained, 1.0);
        self.tails[c.resources[mode.primary.as_str()]] = assignment
            .activities
            .iter()
            .flat_map(|a| &a.reservations)
            .filter(|r| r.resource == mode.primary)
            .map(|r| r.end)
            .max()
            .unwrap_or(assignment.end);
        let (consume, produce) = crate::domain::materials(t, mode);
        if t.execution.is_none() {
            for (item, need) in consume {
                self.materials.book(item, assignment.start, -*need);
            }
        }
        for (item, amount) in produce {
            self.materials.book(item, assignment.ready, *amount);
        }
        self.assignments[i] = Some(assignment);
    }
    fn place(
        &mut self,
        c: &Compiled<'_>,
        i: usize,
        mi: usize,
        specs: &Specs<'_>,
    ) -> Result<Assignment, Diagnostic> {
        let p = c.problem;
        let book = &mut self.book;
        let assignments = &self.assignments;
        let tails = &self.tails;
        let materials = &self.materials;
        let t = &p.tasks[i];
        let selected = &t.modes[mi];
        let (consume, _) = crate::domain::materials(t, selected);
        let r = c.resources[selected.primary.as_str()];
        let (release, deadline) = rules::bounds(p, t);
        let mut at = release.max(tails[r]);
        for &(pred, min, _) in &c.predecessors[i] {
            at = at.max(
                assignments[pred]
                    .as_ref()
                    .ok_or_else(|| Diagnostic::new("ORDER", &t.id, "Predecessor not scheduled"))?
                    .ready
                    + min,
            );
        }
        if t.execution.is_none() {
            at = materials.earliest(&t.id, consume, at)?;
        }
        let actual = t.execution.as_ref();
        if let Some(e) = actual {
            at = at.max(e.as_of);
        }
        let mut activities = Vec::new();
        if let Some(e) = actual {
            let mut a = e.actual.clone();
            a.role = "actual".into();
            activities.push(a);
        }
        let preparation = append_stage(c, book, t, &specs.0, "pre", at)?;
        at = preparation.iter().map(|a| a.end).max().unwrap_or(at);
        activities.extend(preparation);
        let fixed = p
            .locks
            .iter()
            .filter_map(|l| {
                if let Lock::Start { task, at } = l {
                    (task == &t.id).then_some(*at)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if actual.is_none()
            && let Some(&value) = fixed.first()
        {
            if at > value {
                return Err(Diagnostic::new(
                    "LOCK_CONFLICT",
                    &t.id,
                    "Earliest feasible start exceeds fixed start",
                ));
            }
            at = value;
        }
        let mut mode = selected.clone();
        if let Some(e) = actual {
            for ph in &mut mode.phases {
                ph.work = Some(e.remaining_work[&ph.id]);
            }
        }
        let main = calendar::place(c, book, &t.id, &format!("{}:main", t.id), "main", &mode, at)
            .ok_or_else(|| {
                Diagnostic::new(
                    "NO_SLOT",
                    &t.id,
                    "No feasible main slot found within horizon",
                )
            })?;
        let retained = if let Some(e) = actual {
            let req = selected.phases[0]
                .requirements
                .iter()
                .find(|r| r.resource == selected.primary)
                .unwrap();
            let mut points = vec![e.as_of, main.start];
            for a in &activities {
                for r in &a.reservations {
                    if r.resource == selected.primary {
                        points.extend([
                            r.start.max(e.as_of).min(main.start),
                            r.end.max(e.as_of).min(main.start),
                        ]);
                    }
                }
            }
            points.sort_unstable();
            points.dedup();
            let holds: Vec<_> = points
                .windows(2)
                .filter_map(|pair| {
                    let occupied: f64 = activities
                        .iter()
                        .flat_map(|a| &a.reservations)
                        .filter(|r| {
                            r.resource == selected.primary && r.start <= pair[0] && r.end > pair[0]
                        })
                        .map(|r| r.amount)
                        .sum();
                    let amount = (req.amount - occupied).max(0.0);
                    (amount > 0.0 && pair[1] > pair[0]).then(|| Reservation {
                        resource: selected.primary.clone(),
                        start: pair[0],
                        end: pair[1],
                        amount,
                    })
                })
                .collect();
            if !holds.iter().all(|r| calendar::fits(c, book, r)) {
                return Err(Diagnostic::new(
                    "RETAINED_EXECUTION_CONFLICT",
                    &t.id,
                    "Running resource cannot remain reserved until resumption",
                ));
            }
            book.reservations(c, &holds, 1.0);
            holds
        } else {
            vec![]
        };
        let start = actual.map_or(main.start, |e| e.actual.start);
        let end = main.end;
        if fixed.iter().any(|v| *v != start) {
            return Err(Diagnostic::new(
                "LOCK_CONFLICT",
                &t.id,
                "Fixed start cannot be respected",
            ));
        }
        for &(pred, _, max) in &c.predecessors[i] {
            if max.is_some_and(|m| main.start > assignments[pred].as_ref().unwrap().ready + m) {
                return Err(Diagnostic::new(
                    "MAX_LAG",
                    &t.id,
                    "Maximum lag exceeded by candidate",
                ));
            }
        }
        book.reservations(c, &main.reservations, 1.0);
        activities.push(main);
        let post = append_stage(c, book, t, &specs.1, "post", end)?;
        let mut ready = end;
        for (id, cond) in &specs.1 {
            if cond.releases_product
                && let Some(a) = post.iter().find(|a| a.id == format!("{}:{id}", t.id))
            {
                ready = ready.max(a.end);
            }
        }
        activities.extend(post);
        if ready > deadline {
            return Err(Diagnostic::new(
                "DEADLINE",
                &t.id,
                "Candidate exceeds hard completion bound",
            ));
        }
        Ok(Assignment {
            task: t.id.clone(),
            mode: selected.id.clone(),
            primary: selected.primary.clone(),
            start,
            end,
            ready,
            activities,
            retained,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{compile::compile, demo};

    #[test]
    fn successful_and_rejected_probes_restore_exact_booking_state() {
        let mut p = demo::problem(2);
        p.locks.push(Lock::Start {
            task: p.tasks[1].id.clone(),
            at: 0,
        });
        let c = compile(&p).unwrap();
        let mut state = State::new(&c);
        let before = state.book.profiles.clone();
        let first = state.preview(&c, 0, 0, &(vec![], vec![])).unwrap();
        assert_eq!(state.book.profiles, before);
        assert!(state.assignments.iter().all(Option::is_none));
        state.commit(&c, 0, first);
        let before = state.book.profiles.clone();
        let preparation = Conditional {
            id: "preparation".into(),
            modes: vec![demo::mode("M2", 10.0)],
            ..Default::default()
        };
        let specs = (vec![("pre:preparation".into(), &preparation)], vec![]);
        assert_eq!(
            state.preview(&c, 1, 0, &specs).unwrap_err().code,
            "LOCK_CONFLICT"
        );
        assert_eq!(state.book.profiles, before);
        assert!(state.assignments[1].is_none());
        assert!(state.assignments[0].is_some());
    }
}
