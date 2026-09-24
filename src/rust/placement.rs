//! Exact dispatch probes. Reuse calendar placement for independent primary-only work;
//! fall back to prefix decoding when earlier activities may be changed by a neighbor.
use crate::{
    calendar::{self, Book},
    compile::Compiled,
    engine::Decisions,
    model::*,
    policy::Facts,
    queues::Candidate,
};
use std::collections::HashMap;

pub struct Oracle {
    direct: bool,
    book: Book,
    committed: usize,
    states: Vec<Facts>,
    pending: HashMap<(usize, usize), Facts>,
    pub direct_placements: usize,
    pub decoded_prefixes: usize,
}
impl Oracle {
    pub fn new(c: &Compiled<'_>, native: bool) -> Self {
        let p = c.problem;
        let direct = !native
            && p.transitions.is_empty()
            && p.dependencies.is_empty()
            && !p
                .rules
                .iter()
                .any(|r| matches!(r, Rule::SequencePattern { .. }))
            && p.tasks.iter().all(|t| {
                t.execution.is_none()
                    && t.pre.is_empty()
                    && t.post.is_empty()
                    && t.consume.is_empty()
                    && t.produce.is_empty()
                    && t.modes.iter().all(|m| {
                        m.pre.is_empty()
                            && m.post.is_empty()
                            && m.consume.as_ref().is_none_or(|v| v.is_empty())
                            && m.produce.as_ref().is_none_or(|v| v.is_empty())
                            && m.phases.len() == 1
                            && m.phases.iter().all(|ph| {
                                ph.requirements.len() == 1
                                    && ph.requirements[0].resource == m.primary
                                    && ph.requirements[0].amount == 1.0
                            })
                            && p.resources[c.resources[m.primary.as_str()]].capacity == 1.0
                    })
            });
        Self {
            direct,
            book: Book::new(p.resources.len()),
            committed: 0,
            states: vec![
                Facts {
                    reaches_initial: true,
                    ..Default::default()
                };
                p.resources.len()
            ],
            pending: HashMap::new(),
            direct_placements: 0,
            decoded_prefixes: 0,
        }
    }
    pub fn prepare(&mut self, c: &Compiled<'_>, d: &Decisions) {
        if !self.direct {
            return;
        }
        if let Some(&i) = d.order.last() {
            // The caller only appends one decision between invocations.
            assert_eq!(d.order.len(), self.committed + 1);
            let state = self
                .pending
                .remove(&(i, d.modes[i]))
                .expect("selected candidate was probed");
            self.states[c.resources[c.problem.tasks[i].modes[d.modes[i]].primary.as_str()]] = state;
            self.committed += 1;
        }
        self.pending.clear();
    }
    pub fn direct(
        &mut self,
        c: &Compiled<'_>,
        candidate: &Candidate,
    ) -> Option<Result<Facts, Diagnostic>> {
        if !self.direct {
            return None;
        }
        self.direct_placements += 1;
        let p = c.problem;
        let t = &p.tasks[candidate.task];
        let m = &t.modes[candidate.mode];
        let previous = &self.states[c.resources[m.primary.as_str()]];
        let (release, deadline) = crate::rules::bounds(p, t);
        let mut at = previous.previous_end.max(release);
        let fixed = p.locks.iter().find_map(|l| match l {
            Lock::Start { task, at } if task == &t.id => Some(*at),
            _ => None,
        });
        if let Some(fixed) = fixed {
            if at > fixed {
                return Some(Err(Diagnostic::new(
                    "LOCK_CONFLICT",
                    &t.id,
                    "Earliest start exceeds commitment",
                )));
            }
            at = fixed;
        }
        self.book.profiles[c.resources[m.primary.as_str()]].clear();
        let Some(a) = calendar::place(
            c,
            &mut self.book,
            &t.id,
            &format!("{}:main", t.id),
            "main",
            m,
            at,
        ) else {
            return Some(Err(Diagnostic::new(
                "NO_SLOT",
                &t.id,
                "No exact primary-resource slot",
            )));
        };
        if fixed.is_some_and(|v| v != a.start) {
            return Some(Err(Diagnostic::new(
                "LOCK_CONFLICT",
                &t.id,
                "Calendar cannot honor fixed start",
            )));
        }
        if a.end > deadline {
            return Some(Err(Diagnostic::new(
                "DEADLINE",
                &t.id,
                "Exact placement exceeds deadline",
            )));
        }
        let f = Facts {
            start: a.start,
            end: a.end,
            ready: a.end,
            ..previous.clone()
        };
        let same_family = previous.family.as_ref().is_none_or(|v| v == &t.family);
        let mut next = if same_family {
            previous.clone()
        } else {
            Facts::default()
        };
        // 'start' on committed state is the first main start of its current campaign.
        if previous.family.is_none() || !same_family {
            next.start = a.start;
        }
        next.family = Some(t.family.clone());
        next.previous_end = a.end;
        next.campaign_work += a.segments.iter().map(|s| s.work).sum::<f64>();
        next.campaign_productive += a
            .segments
            .iter()
            .map(|s| (s.end - s.start) as f64)
            .sum::<f64>();
        next.campaign_occupied += a
            .reservations
            .iter()
            .map(|r| (r.end - r.start) as f64)
            .sum::<f64>();
        next.campaign_elapsed = (a.end - next.start) as f64;
        self.pending.insert((candidate.task, candidate.mode), next);
        Some(Ok(f))
    }
}
