//! Mandatory dispatch policies, evaluated before Q normalization in every search strategy.
use crate::{
    compile::Compiled, engine::Decisions, language::Selector, model::*, queues::Candidate,
    rules::Customization,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

#[derive(JsonSchema, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    Work,
    ProductiveTime,
    OccupiedTime,
    ElapsedTime,
}
#[derive(JsonSchema, Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoMatch {
    AllowSwitch,
    Fail,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InitialCampaign {
    pub family: String,
    pub credit: f64,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Policy {
    Campaign {
        id: String,
        resource: String,
        #[serde(default)]
        select: Selector,
        minimum: f64,
        basis: Basis,
        #[serde(default)]
        initial: Option<InitialCampaign>,
        on_no_match: NoMatch,
        #[serde(default)]
        urgent_slack: Option<Time>,
        #[serde(default)]
        continue_after_minimum: bool,
        #[serde(default)]
        exempt_committed: bool,
    },
    IdleGap {
        id: String,
        resource: String,
        maximum: Time,
        /// Explicit fallback to the smallest feasible gap when all exceed the limit.
        fallback_to_smallest: bool,
    },
    Urgency {
        id: String,
        resource: String,
        slack: Time,
    },
}
impl Policy {
    pub fn id(&self) -> &str {
        match self {
            Self::Campaign { id, .. } | Self::IdleGap { id, .. } | Self::Urgency { id, .. } => id,
        }
    }
    pub fn resource(&self) -> &str {
        match self {
            Self::Campaign { resource, .. }
            | Self::IdleGap { resource, .. }
            | Self::Urgency { resource, .. } => resource,
        }
    }
}
pub fn check(p: &Problem, errors: &mut Vec<Diagnostic>) {
    let mut ids = HashSet::new();
    for r in &p.planning.policies {
        let bad = match r {
            Policy::Campaign {
                minimum,
                initial,
                urgent_slack,
                ..
            } => {
                !minimum.is_finite()
                    || *minimum < 0.0
                    || initial.as_ref().is_some_and(|v| {
                        v.family.is_empty() || !v.credit.is_finite() || v.credit < 0.0
                    })
                    || urgent_slack.is_some_and(|v| v < 0)
            }
            Policy::IdleGap { maximum, .. } => *maximum < 0,
            Policy::Urgency { slack, .. } => *slack < 0,
        };
        if bad
            || r.id().is_empty()
            || !ids.insert(r.id())
            || !p.resources.iter().any(|v| v.id == r.resource())
        {
            errors.push(Diagnostic::new(
                "DISPATCH_POLICY",
                r.id(),
                "Policy needs a unique ID, existing resource and finite nonnegative parameters",
            ));
        }
        if let Policy::Campaign { select, .. } = r
            && p.tasks.iter().any(|t| {
                select.matches(t)
                    && t.modes.iter().any(|m| m.primary == r.resource())
                    && t.family.is_empty()
            })
        {
            errors.push(Diagnostic::new(
                "DISPATCH_FAMILY",
                r.id(),
                "Campaign tasks need explicit family values",
            ));
        }
    }
}
pub fn enabled(p: &Problem, extension: Option<&dyn Customization>) -> bool {
    !p.planning.policies.is_empty() || extension.is_some_and(|e| e.has_dispatch_policy())
}
pub fn fingerprint(p: &Problem, extension: Option<&dyn Customization>) -> String {
    let mut model = serde_json::to_value(p).expect("validated model");
    model.as_object_mut().unwrap().remove("assumptions");
    Sha256::digest(serde_json::to_vec(&(model, extension.map(|e| (e.id(), e.version())))).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Reason {
    pub rule: String,
    pub message: String,
    pub excluded: usize,
    pub examples: Vec<Decision>,
}
#[derive(JsonSchema, Clone, Debug, Default, Serialize, Deserialize)]
pub struct Step {
    pub position: usize,
    pub selected: Option<Decision>,
    pub considered: usize,
    pub allowed: usize,
    pub reasons: Vec<Reason>,
    pub omitted_reasons: usize,
}
#[derive(JsonSchema, Clone, Debug, Serialize, Deserialize)]
pub struct Evidence {
    pub version: String,
    pub model_hash: String,
    pub decisions: Vec<Decision>,
    pub options: Box<Options>,
    pub steps: Vec<Step>,
    pub omitted_steps: usize,
    pub exact_probes: usize,
    pub direct_placements: usize,
}
#[derive(Clone, Debug, Default)]
pub struct Facts {
    pub start: Time,
    pub end: Time,
    pub ready: Time,
    pub previous_end: Time,
    pub family: Option<String>,
    pub campaign_work: f64,
    pub campaign_productive: f64,
    pub campaign_occupied: f64,
    pub campaign_elapsed: f64,
    pub reaches_initial: bool,
}
/// The complete eligible ready pool, not a Q window. A hook may only reject existing pairs.
pub struct Context<'a> {
    pub problem: &'a Problem,
    pub prefix: &'a Decisions,
    pub candidates: &'a [Candidate],
    pub placements: &'a [Option<Facts>],
}
pub struct Rejection {
    pub task: usize,
    pub mode: usize,
    pub reason: String,
}

fn decision(p: &Problem, c: &Candidate) -> Decision {
    Decision {
        task: p.tasks[c.task].id.clone(),
        mode: p.tasks[c.task].modes[c.mode].id.clone(),
    }
}
fn occupied(a: &Assignment, resource: &str) -> f64 {
    let mut intervals: Vec<_> = a
        .activities
        .iter()
        .flat_map(|v| &v.reservations)
        .chain(&a.retained)
        .filter(|v| v.resource == resource)
        .map(|v| (v.start, v.end))
        .collect();
    intervals.sort_unstable();
    let (mut end, mut total) = (0, 0);
    for (start, next) in intervals {
        total += (next - start.max(end)).max(0);
        end = end.max(next);
    }
    total as f64
}
fn facts(c: &Compiled<'_>, d: &Decisions, candidate: &Candidate, s: &Schedule) -> Facts {
    let p = c.problem;
    let task = &p.tasks[candidate.task];
    let resource = &task.modes[candidate.mode].primary;
    let rows: std::collections::HashMap<_, _> =
        s.assignments.iter().map(|a| (a.task.as_str(), a)).collect();
    let a = rows[task.id.as_str()];
    let sequence: Vec<_> = d
        .order
        .iter()
        .copied()
        .filter(|&i| p.tasks[i].modes[d.modes[i]].primary == *resource)
        .collect();
    let mut f = Facts {
        start: a.start,
        end: a.end,
        ready: a.ready,
        reaches_initial: true,
        ..Default::default()
    };
    let mut first = None;
    let mut last = 0;
    for &i in sequence.iter().rev() {
        let t = &p.tasks[i];
        let a = rows[t.id.as_str()];
        if f.family.as_ref().is_some_and(|family| family != &t.family) {
            f.reaches_initial = false;
            break;
        }
        if f.family.is_none() {
            f.family = Some(t.family.clone());
            f.previous_end = a
                .activities
                .iter()
                .flat_map(|v| &v.reservations)
                .chain(&a.retained)
                .filter(|v| v.resource == *resource)
                .map(|v| v.end)
                .max()
                .unwrap_or(a.end);
            last = a.end;
        }
        first = Some(a.start);
        for seg in a
            .activities
            .iter()
            .filter(|v| matches!(v.role.as_str(), "main" | "actual"))
            .flat_map(|v| &v.segments)
        {
            f.campaign_work += seg.work;
            f.campaign_productive += (seg.end - seg.start) as f64;
        }
        f.campaign_occupied += occupied(a, resource);
    }
    f.campaign_elapsed = first.map_or(0.0, |start| (last - start) as f64);
    f
}
fn probe(
    c: &Compiled<'_>,
    d: &Decisions,
    candidate: &Candidate,
    extension: Option<&dyn Customization>,
) -> Result<Facts, Diagnostic> {
    let mut next = Decisions {
        order: d.order.clone(),
        modes: d.modes.clone(),
        next_choices: vec![],
        evidence: None,
    };
    next.order.push(candidate.task);
    next.modes[candidate.task] = candidate.mode;
    let s = if let Some(extension) = extension {
        let decorated =
            crate::extensions::decorate(c.problem, &next, extension).map_err(|e| e[0].clone())?;
        crate::engine::decode_prefix(
            &crate::compile::compile(&decorated).map_err(|e| e[0].clone())?,
            &next,
        )?
    } else {
        crate::engine::decode_prefix(c, &next)?
    };
    Ok(facts(c, d, candidate, &s))
}
pub fn filter(
    c: &Compiled<'_>,
    d: &Decisions,
    candidates: &mut Vec<Candidate>,
    extension: Option<&dyn Customization>,
    oracle: &mut crate::placement::Oracle,
) -> Result<Step, Diagnostic> {
    let p = c.problem;
    let exact =
        !p.planning.policies.is_empty() || extension.is_some_and(|e| e.dispatch_needs_placements());
    if exact && extension.is_some_and(|e| !e.supports_prefix_decoration()) {
        return Err(Diagnostic::new(
            "DISPATCH_EXTENSION",
            &p.id,
            "Exact dispatch requires the native extension to declare prefix-safe sequence decoration",
        ));
    }
    let mut step = Step {
        position: d.order.len(),
        considered: candidates.len(),
        ..Default::default()
    };
    oracle.prepare(c, d);
    let mut placements = Vec::with_capacity(candidates.len());
    let mut keep = vec![true; candidates.len()];
    let mut record = |rule: &str, message: &str, rejected: Vec<usize>| {
        if rejected.is_empty() {
            return;
        }
        let message: String = message.chars().take(512).collect();
        if let Some(r) = step
            .reasons
            .iter_mut()
            .find(|r| r.rule == rule && r.message == message)
        {
            r.excluded += rejected.len();
            let available = 8_usize.saturating_sub(r.examples.len());
            r.examples.extend(
                rejected
                    .iter()
                    .take(available)
                    .map(|&i| decision(p, &candidates[i])),
            );
        } else if step.reasons.len() < 32 {
            step.reasons.push(Reason {
                rule: rule.into(),
                message,
                excluded: rejected.len(),
                examples: rejected
                    .iter()
                    .take(8)
                    .map(|&i| decision(p, &candidates[i]))
                    .collect(),
            });
        } else {
            step.omitted_reasons += 1;
        }
    };
    let mut infeasible = vec![];
    for (i, candidate) in candidates.iter().enumerate() {
        if exact {
            if oracle.decoded_prefixes >= 100_000 {
                return Err(Diagnostic::new(
                    "DISPATCH_PROBE_BUDGET",
                    &p.id,
                    "Exact construction exceeded 100000 probes; split the horizon or reduce the ready pool. This is not an infeasibility proof.",
                ));
            }
            let result = oracle.direct(c, candidate).unwrap_or_else(|| {
                oracle.decoded_prefixes += 1;
                probe(c, d, candidate, extension)
            });
            match result {
                Ok(f) => placements.push(Some(f)),
                Err(e)
                    if [
                        "NO_SLOT",
                        "MATERIAL_SHORTAGE",
                        "LOCK_CONFLICT",
                        "MAX_LAG",
                        "DEADLINE",
                        "RETAINED_EXECUTION_CONFLICT",
                    ]
                    .contains(&e.code.as_str()) =>
                {
                    keep[i] = false;
                    infeasible.push(i);
                    placements.push(None);
                }
                Err(e) => return Err(e),
            }
        } else {
            placements.push(None);
        }
    }
    record(
        "placement",
        "No feasible exact prefix placement",
        infeasible,
    );
    for rule in &p.planning.policies {
        let indexes: Vec<_> = candidates
            .iter()
            .enumerate()
            .filter(|(i, v)| keep[*i] && p.tasks[v.task].modes[v.mode].primary == rule.resource())
            .map(|(i, _)| i)
            .collect();
        let mut rejected = vec![];
        match rule {
            Policy::IdleGap {
                maximum,
                fallback_to_smallest,
                ..
            } => {
                let gap = |i: usize| {
                    let f = placements[i].as_ref().unwrap();
                    (f.start - f.previous_end).max(0)
                };
                let threshold = if *fallback_to_smallest {
                    (*maximum).max(indexes.iter().map(|&i| gap(i)).min().unwrap_or(0))
                } else {
                    *maximum
                };
                rejected.extend(indexes.into_iter().filter(|&i| gap(i) > threshold));
            }
            Policy::Urgency { slack, .. } => {
                let urgent = |i: usize| {
                    c.urgency[candidates[i].task]
                        .due
                        .is_some_and(|due| due - placements[i].as_ref().unwrap().end <= *slack)
                };
                if indexes.iter().any(|&i| urgent(i)) {
                    rejected.extend(indexes.into_iter().filter(|&i| !urgent(i)));
                }
            }
            Policy::Campaign {
                select,
                minimum,
                basis,
                initial,
                on_no_match,
                urgent_slack,
                continue_after_minimum,
                exempt_committed,
                ..
            } => {
                let scoped: Vec<_> = indexes
                    .into_iter()
                    .filter(|&i| select.matches(&p.tasks[candidates[i].task]))
                    .collect();
                let family = |i: usize| {
                    placements[i]
                        .as_ref()
                        .unwrap()
                        .family
                        .as_deref()
                        .or(initial.as_ref().map(|v| v.family.as_str()))
                };
                let continuation = scoped
                    .iter()
                    .any(|&i| family(i).is_some_and(|f| f == p.tasks[candidates[i].task].family));
                for i in scoped {
                    let t = &p.tasks[candidates[i].task];
                    let f = placements[i].as_ref().unwrap();
                    let committed = t.execution.is_some()
                        || p.locks
                            .iter()
                            .any(|l| matches!(l, Lock::Start { task, .. } if task == &t.id));
                    if *exempt_committed && committed
                        || urgent_slack.is_some_and(|slack| {
                            c.urgency[candidates[i].task]
                                .due
                                .is_some_and(|due| due - f.end <= slack)
                        })
                    {
                        continue;
                    }
                    let credit = match basis {
                        Basis::Work => f.campaign_work,
                        Basis::ProductiveTime => f.campaign_productive,
                        Basis::OccupiedTime => f.campaign_occupied,
                        Basis::ElapsedTime => f.campaign_elapsed,
                    } + initial
                        .as_ref()
                        .filter(|v| f.reaches_initial && family(i) == Some(v.family.as_str()))
                        .map_or(0.0, |v| v.credit);
                    if family(i).is_some_and(|previous| previous != t.family)
                        && (*continue_after_minimum || credit < *minimum)
                        && (continuation || matches!(on_no_match, NoMatch::Fail))
                    {
                        rejected.push(i);
                    }
                }
            }
        }
        for &i in &rejected {
            keep[i] = false;
        }
        let message = match rule {
            Policy::Campaign { minimum, basis, .. } => {
                let unit = match basis {
                    Basis::Work => "work units",
                    Basis::ProductiveTime => "productive seconds",
                    Basis::OccupiedTime => "occupied seconds",
                    Basis::ElapsedTime => "elapsed seconds",
                };
                format!(
                    "Family switch excluded by the campaign policy (minimum {minimum} {unit}, with declared exceptions)"
                )
            }
            Policy::IdleGap { maximum, .. } => {
                format!("Idle gap exceeds {maximum} seconds and the declared fallback threshold")
            }
            Policy::Urgency { slack, .. } => format!(
                "An urgent candidate with due-date slack at most {slack} seconds is available"
            ),
        };
        record(rule.id(), &message, rejected);
    }
    if let Some(extension) = extension.filter(|e| e.has_dispatch_policy()) {
        let allowed: Vec<_> = candidates
            .iter()
            .enumerate()
            .filter(|(i, _)| keep[*i])
            .map(|(_, c)| c.clone())
            .collect();
        let facts: Vec<_> = placements
            .iter()
            .enumerate()
            .filter(|(i, _)| keep[*i])
            .map(|(_, f)| f.clone())
            .collect();
        for rejection in extension.filter_candidates(&Context {
            problem: p,
            prefix: d,
            candidates: &allowed,
            placements: &facts,
        })? {
            let Some(i) = candidates
                .iter()
                .enumerate()
                .find(|(i, c)| keep[*i] && c.task == rejection.task && c.mode == rejection.mode)
                .map(|(i, _)| i)
            else {
                return Err(Diagnostic::new(
                    "DISPATCH_HOOK",
                    extension.id(),
                    "Hook rejected a pair outside its supplied candidate pool or repeated a rejection",
                ));
            };
            keep[i] = false;
            record(extension.id(), &rejection.reason, vec![i]);
        }
    }
    let mut i = 0;
    candidates.retain(|_| {
        let value = keep[i];
        i += 1;
        value
    });
    step.allowed = candidates.len();
    if candidates.is_empty() {
        return Err(Diagnostic::new(
            "DISPATCH_CONFLICT",
            &p.id,
            serde_json::to_string(&step).unwrap(),
        ));
    }
    Ok(step)
}

pub fn verify(
    p: &Problem,
    s: &Schedule,
    extension: Option<&dyn Customization>,
) -> Result<(), Vec<Diagnostic>> {
    if !enabled(p, extension) {
        return Ok(());
    }
    let fail = |message| vec![Diagnostic::new("DISPATCH_WITNESS", &p.id, message)];
    let e = s
        .dispatch
        .as_ref()
        .ok_or_else(|| fail("Policy schedules require a replayable dispatch witness"))?;
    if e.version != "apex.dispatch.v1"
        || e.model_hash != fingerprint(p, extension)
        || e.decisions.len() != p.tasks.len()
    {
        return Err(fail(
            "Dispatch witness version, model hash or coverage differs",
        ));
    }
    let mut options = (*e.options).clone();
    if options.strategy == "weighted" && options.weights.is_none() {
        return Err(fail("Weighted replay needs explicit dispatch weights"));
    }
    options.decision_prefix = e.decisions.clone();
    let c = crate::compile::compile(p)?;
    let d = crate::dispatch::decisions_with(&c, &options, extension).map_err(|e| vec![e])?;
    if let Some(actual) = &d.evidence
        && (serde_json::to_value(&actual.steps).unwrap() != serde_json::to_value(&e.steps).unwrap()
            || actual.omitted_steps != e.omitted_steps
            || actual.exact_probes != e.exact_probes
            || actual.direct_placements != e.direct_placements)
    {
        return Err(fail(
            "Dispatch explanations or counters differ from policy replay",
        ));
    }
    let expected = if let Some(extension) = extension {
        let decorated = crate::extensions::decorate(p, &d, extension)?;
        crate::engine::decode(&crate::compile::compile(&decorated)?, &d)
    } else {
        crate::engine::decode(&c, &d)
    }
    .map_err(|e| vec![e])?;
    if serde_json::to_value(&expected.assignments).unwrap()
        != serde_json::to_value(&s.assignments).unwrap()
    {
        return Err(fail(
            "Assignments differ from independently replayed policy decisions",
        ));
    }
    Ok(())
}
