//! Checks supplied assignments independently of the placement algorithm and its caches.
use crate::{
    calendar::{boundary, hold_capacity, window_at},
    compile::compile,
    model::*,
    rules,
    xg::{Decisions, conditional_specs},
};
use std::collections::{BTreeMap, HashMap, HashSet};

pub fn validate(p: &Problem, s: &Schedule) -> Validation {
    let mut report = validate_model(p, s);
    if report.valid && p.customization.is_none() && crate::policy::enabled(p, None) {
        let checked = crate::domain::resolve(p, &crate::conditionals::validation_options(s))
            .and_then(|(active, _)| crate::policy::verify(&active, s, None));
        if let Err(errors) = checked {
            report.valid = false;
            report.diagnostics.extend(errors);
        }
    }
    report
}
fn validate_model(p: &Problem, s: &Schedule) -> Validation {
    if let Some(reference) = &p.customization {
        return match crate::extensions::registered(reference) {
            Ok(extension) => validate_customized(p, s, extension.as_ref()),
            Err(diagnostics) => Validation {
                valid: false,
                diagnostics,
            },
        };
    }
    if !crate::domain::requires_lowering(p)
        && s.route_choices.is_empty()
        && crate::conditionals::validation_options(s)
            .conditional_choices
            .is_empty()
    {
        return validate_active(p, s);
    }
    if let Err(diagnostics) = compile(p) {
        return Validation {
            valid: false,
            diagnostics,
        };
    }
    if s.route_choices.len() != p.routes.len()
        || p.routes
            .iter()
            .any(|r| !s.route_choices.contains_key(&r.id))
    {
        return Validation {
            valid: false,
            diagnostics: vec![Diagnostic::new(
                "ROUTE_COVERAGE",
                &p.id,
                "Schedule must declare one alternative for every route choice",
            )],
        };
    }
    match crate::domain::resolve(p, &crate::conditionals::validation_options(s)) {
        Ok((resolved, _)) => {
            let mut report = validate_active(&resolved, s);
            if resolved.material_report != s.material_report {
                report.diagnostics.push(Diagnostic::new(
                    "MATERIAL_REPORT",
                    &p.id,
                    "Material allocation report differs from the selected route's allocation",
                ));
                report.valid = false;
            }
            report
        }
        Err(diagnostics) => Validation {
            valid: false,
            diagnostics,
        },
    }
}
pub(crate) fn validate_active(p: &Problem, s: &Schedule) -> Validation {
    let c = match compile(p) {
        Ok(c) => c,
        Err(diagnostics) => {
            return Validation {
                valid: false,
                diagnostics,
            };
        }
    };
    let mut errors = crate::conditionals::violations(p, s);
    if !s.construction.is_empty() {
        let decisions: HashMap<_, _> = s.construction.iter().map(|d| (&d.task, &d.mode)).collect();
        if decisions.len() != s.construction.len()
            || decisions.len() != s.assignments.len()
            || s.assignments
                .iter()
                .any(|a| decisions.get(&a.task) != Some(&&a.mode))
        {
            errors.push(Diagnostic::new(
                "CONSTRUCTION",
                &p.id,
                "Decoder decisions must uniquely cover the actual assignments and modes",
            ));
        }
        if s.replay
            .as_deref()
            .is_some_and(|o| !s.construction.starts_with(&o.decision_prefix))
        {
            errors.push(Diagnostic::new(
                "CONSTRUCTION_PREFIX",
                &p.id,
                "Actual construction differs from the committed prefix",
            ));
        }
    }
    if let Some(o) = s.replay.as_deref() {
        let modes: HashMap<_, _> = s.assignments.iter().map(|a| (&a.task, &a.mode)).collect();
        if o.mode_choices
            .iter()
            .any(|(id, mode)| modes.get(id) != Some(&mode))
        {
            errors.push(Diagnostic::new(
                "MODE_SELECTION",
                &p.id,
                "Assignments differ from replay mode commitments",
            ));
        }
    }
    let mut emit = |code: &str, id: &str, msg: &str| errors.push(Diagnostic::new(code, id, msg));
    if s.problem_id != p.id {
        emit(
            "PROBLEM_ID",
            &p.id,
            "Schedule belongs to a different problem",
        );
    }
    let mut map = HashMap::new();
    let mut modes = vec![0; p.tasks.len()];
    for a in &s.assignments {
        if map.insert(a.task.as_str(), a).is_some() {
            emit("DUPLICATE_TASK", &a.task, "Repeated assignment");
        }
        if let Some(&i) = c.tasks.get(a.task.as_str()) {
            if let Some(mi) = p.tasks[i].modes.iter().position(|m| m.id == a.mode) {
                modes[i] = mi;
            } else {
                emit("MODE", &a.task, "Unknown selected mode");
            }
        } else {
            emit("UNKNOWN_TASK", &a.task, "Unexpected assignment");
        }
    }
    for t in &p.tasks {
        if !map.contains_key(t.id.as_str()) {
            emit("MISSING_TASK", &t.id, "Required task has no assignment");
        }
    }
    if !errors.is_empty() {
        return Validation {
            valid: false,
            diagnostics: errors,
        };
    }
    let mut order: Vec<_> = (0..p.tasks.len()).collect();
    order.sort_by_key(|&i| {
        let a = map[p.tasks[i].id.as_str()];
        (
            a.activities
                .iter()
                .map(|x| x.start)
                .min()
                .unwrap_or(a.start),
            a.start,
            i,
        )
    });
    // Transition neighbors follow main-operation order, independently of array ordering.
    order.sort_by_key(|&i| {
        let a = map[p.tasks[i].id.as_str()];
        (a.start, i)
    });
    let specs = match conditional_specs(
        &c,
        &Decisions {
            next_choices: vec![],
            evidence: None,
            order,
            modes: modes.clone(),
        },
    ) {
        Ok(v) => v,
        Err(e) => {
            return Validation {
                valid: false,
                diagnostics: vec![e],
            };
        }
    };
    let mut occupancy: Vec<BTreeMap<Time, f64>> = vec![BTreeMap::new(); p.resources.len()];
    let mut material: BTreeMap<&str, BTreeMap<Time, f64>> = BTreeMap::new();
    for (item, amount) in &p.inventory {
        *material.entry(item).or_default().entry(0).or_default() += amount;
    }
    for receipt in &p.receipts {
        *material
            .entry(&receipt.item)
            .or_default()
            .entry(receipt.at)
            .or_default() += receipt.amount;
    }
    for (i, t) in p.tasks.iter().enumerate() {
        let a = map[t.id.as_str()];
        let m = &t.modes[modes[i]];
        let (consume, produce) = crate::domain::materials(t, m);
        if t.execution.as_ref().is_some_and(|e| e.mode != a.mode) {
            errors.push(Diagnostic::new(
                "EXECUTION_MODE",
                &t.id,
                "Running work changed its fixed mode",
            ));
        }
        let (release, deadline) = rules::bounds(p, t);
        if a.primary != m.primary
            || a.start < release && t.execution.is_none()
            || a.end < a.start
            || a.ready < a.end
            || a.ready > deadline
        {
            errors.push(Diagnostic::new(
                "TASK_BOUNDS",
                &t.id,
                "Invalid assignment bounds, resource or deadline",
            ));
        }
        let main_id = format!("{}:main", t.id);
        let mut expected: HashMap<String, (&[Mode], &str)> = HashMap::new();
        expected.insert(main_id.clone(), (std::slice::from_ref(m), "main"));
        for (id, cond) in &specs[i].0 {
            expected.insert(format!("{}:{id}", t.id), (&cond.modes, "pre"));
        }
        for (id, cond) in &specs[i].1 {
            expected.insert(format!("{}:{id}", t.id), (&cond.modes, "post"));
        }
        let mut seen = HashSet::new();
        let mut actual_count = 0;
        for activity in &a.activities {
            if activity.role == "actual" {
                actual_count += 1;
                let correct = t.execution.as_ref().is_some_and(|e| {
                    let mut original = e.actual.clone();
                    original.role = "actual".into();
                    original == *activity
                });
                if !correct {
                    errors.push(Diagnostic::new(
                        "EXECUTED_HISTORY",
                        &t.id,
                        "Actual history was modified",
                    ));
                }
            } else if let Some((alternatives, role)) = expected.get(&activity.id) {
                if !seen.insert(&activity.id) {
                    errors.push(Diagnostic::new(
                        "DUPLICATE_ACTIVITY",
                        &t.id,
                        "Repeated activity",
                    ));
                }
                if let Some(mode) = alternatives.iter().find(|m| m.id == activity.mode) {
                    let mut mode = crate::domain::scaled_mode(mode, t.quantity);
                    if activity.id == main_id
                        && let Some(e) = &t.execution
                    {
                        for ph in &mut mode.phases {
                            ph.work = Some(e.remaining_work[&ph.id]);
                        }
                        if activity.start < e.as_of {
                            errors.push(Diagnostic::new(
                                "EXECUTION_TIME",
                                &t.id,
                                "Remaining work precedes observation time",
                            ));
                        }
                    }
                    check_activity(p, activity, &mode, role, &t.id, &mut errors);
                } else {
                    errors.push(Diagnostic::new(
                        "ACTIVITY_MODE",
                        &t.id,
                        "Unknown activity mode",
                    ));
                }
            } else {
                errors.push(Diagnostic::new(
                    "UNEXPECTED_ACTIVITY",
                    &t.id,
                    format!("Unexpected {}", activity.id),
                ));
            }
            for r in &activity.reservations {
                if let Some(&ri) = c.resources.get(r.resource.as_str()) {
                    if r.start < 0
                        || r.end > p.horizon
                        || r.start >= r.end
                        || !r.amount.is_finite()
                        || r.amount <= 0.0
                    {
                        errors.push(Diagnostic::new(
                            "RESERVATION",
                            &activity.id,
                            "Invalid reservation",
                        ));
                        continue;
                    }
                    *occupancy[ri].entry(r.start).or_default() += r.amount;
                    *occupancy[ri].entry(r.end).or_default() -= r.amount;
                } else {
                    errors.push(Diagnostic::new(
                        "RESOURCE",
                        &activity.id,
                        "Unknown reservation resource",
                    ));
                }
            }
        }
        for r in &a.retained {
            if let Some(&ri) = c.resources.get(r.resource.as_str()) {
                if r.resource != a.primary
                    || r.start >= r.end
                    || r.amount <= 0.0
                    || !r.amount.is_finite()
                {
                    errors.push(Diagnostic::new(
                        "RETAINED_EXECUTION",
                        &t.id,
                        "Invalid carried reservation",
                    ));
                }
                *occupancy[ri].entry(r.start).or_default() += r.amount;
                *occupancy[ri].entry(r.end).or_default() -= r.amount;
            } else {
                errors.push(Diagnostic::new(
                    "RETAINED_EXECUTION",
                    &t.id,
                    "Unknown carried resource",
                ));
            }
        }
        if t.execution.is_none() && !a.retained.is_empty() {
            errors.push(Diagnostic::new(
                "RETAINED_EXECUTION",
                &t.id,
                "Only running work may carry execution reservations",
            ));
        }
        if let Some(e) = &t.execution
            && let Some(main) = a.activities.iter().find(|a| a.id == main_id)
        {
            let required = m.phases[0]
                .requirements
                .iter()
                .find(|r| r.resource == m.primary)
                .unwrap()
                .amount;
            let mut changes = BTreeMap::<Time, f64>::from([(e.as_of, 0.0), (main.start, 0.0)]);
            for r in a
                .activities
                .iter()
                .flat_map(|a| &a.reservations)
                .chain(&a.retained)
                .filter(|r| r.resource == m.primary)
            {
                let start = r.start.max(e.as_of);
                let end = r.end.min(main.start);
                if start < end {
                    *changes.entry(start).or_default() += r.amount;
                    *changes.entry(end).or_default() -= r.amount;
                }
            }
            let mut used = 0.0;
            for (time, delta) in changes {
                used += delta;
                if time < main.start && (used - required).abs() > 1e-7 {
                    errors.push(Diagnostic::new(
                        "RETAINED_EXECUTION",
                        &t.id,
                        "Running primary occupancy is not preserved until resumption",
                    ));
                }
            }
            if a.retained
                .iter()
                .any(|r| r.start < e.as_of || r.end > main.start)
            {
                errors.push(Diagnostic::new(
                    "RETAINED_EXECUTION",
                    &t.id,
                    "Carry outside the resumption interval",
                ));
            }
        }
        if seen.len() != expected.len() || actual_count != usize::from(t.execution.is_some()) {
            errors.push(Diagnostic::new(
                "ACTIVITY_COVERAGE",
                &t.id,
                "Missing required main, conditional or actual activity",
            ));
        }
        if let Some(main) = a.activities.iter().find(|x| x.id == main_id) {
            let expected_start = t.execution.as_ref().map_or(main.start, |e| e.actual.start);
            if a.start != expected_start || a.end != main.end {
                errors.push(Diagnostic::new(
                    "MAIN_ENVELOPE",
                    &t.id,
                    "Assignment differs from main activity",
                ));
            }
            let base = t.execution.as_ref().map_or(release, |e| e.as_of);
            let mut ready = main.end;
            for (stage, is_pre) in [(&specs[i].0, true), (&specs[i].1, false)] {
                let (_, edges) = crate::activities::graph(stage, p.horizon).unwrap();
                for (index, (id, conditional)) in stage.iter().enumerate() {
                    let Some(activity) = a
                        .activities
                        .iter()
                        .find(|x| x.id == format!("{}:{id}", t.id))
                    else {
                        continue;
                    };
                    if activity.start < if is_pre { base } else { main.end }
                        || is_pre && activity.end > main.start
                    {
                        errors.push(Diagnostic::new(
                            "PRECEDENCE",
                            &activity.id,
                            "Conditional crosses its stage boundary",
                        ));
                    }
                    for (before, min, max) in &edges[index] {
                        if let Some(previous) = a
                            .activities
                            .iter()
                            .find(|x| x.id == format!("{}:{}", t.id, stage[*before].0))
                            && (activity.start < previous.end + min
                                || max.is_some_and(|v| activity.start > previous.end + v))
                        {
                            errors.push(Diagnostic::new(
                                "CONDITIONAL_LAG",
                                &activity.id,
                                "Conditional dependency violated",
                            ));
                        }
                    }
                    if !is_pre && conditional.releases_product {
                        ready = ready.max(activity.end);
                    }
                }
            }
            if a.ready != ready {
                errors.push(Diagnostic::new(
                    "PRODUCT_RELEASE",
                    &t.id,
                    "Product readiness differs from required post work",
                ));
            }
        }
        for &(pred, min, max) in &c.predecessors[i] {
            let pa = map[p.tasks[pred].id.as_str()];
            if a.start < pa.ready + min || max.is_some_and(|m| a.start > pa.ready + m) {
                errors.push(Diagnostic::new(
                    "DEPENDENCY",
                    &t.id,
                    "Temporal dependency violated",
                ));
            }
        }
        if t.execution.is_none() {
            for (item, amount) in consume {
                *material
                    .entry(item)
                    .or_default()
                    .entry(a.start)
                    .or_default() -= amount;
            }
        }
        for (item, amount) in produce {
            *material
                .entry(item)
                .or_default()
                .entry(a.ready)
                .or_default() += amount;
        }
    }
    for (ri, events) in occupancy.iter_mut().enumerate() {
        let r = &p.resources[ri];
        for w in &r.retention_calendar {
            events.entry(w.start).or_default();
            events.entry(w.end).or_default();
        }
        let mut used = 0.0;
        for (&at, &delta) in events.iter() {
            used += delta;
            if used > hold_capacity(r, at) + 1e-7 && at < p.horizon {
                errors.push(Diagnostic::new(
                    "CAPACITY",
                    &r.id,
                    format!("Used {used} at {at}"),
                ));
            }
        }
    }
    for (item, events) in material {
        let mut stock = 0.0;
        for (at, delta) in events {
            stock += delta;
            if stock < -1e-7 {
                errors.push(Diagnostic::new(
                    "MATERIAL_BALANCE",
                    item,
                    format!("Negative stock at {at}"),
                ));
            }
        }
    }
    for l in &p.locks {
        let valid = match l {
            Lock::Mode { task, mode } => map[task.as_str()].mode == *mode,
            Lock::Resource { task, resource } => map[task.as_str()].primary == *resource,
            Lock::Start { task, at } => map[task.as_str()].start == *at,
            Lock::Order {
                resource,
                tasks,
                consecutive,
            } => {
                let mut seq: Vec<_> = s
                    .assignments
                    .iter()
                    .filter(|a| a.primary == *resource)
                    .collect();
                seq.sort_by_key(|a| (a.start, c.tasks[a.task.as_str()]));
                let pos: Vec<_> = tasks
                    .iter()
                    .filter_map(|id| seq.iter().position(|a| &a.task == id))
                    .collect();
                pos.len() == tasks.len()
                    && pos
                        .windows(2)
                        .all(|p| p[0] < p[1] && (!consecutive || p[1] == p[0] + 1))
            }
        };
        if !valid {
            errors.push(Diagnostic::new(
                "LOCK_VIOLATION",
                &p.id,
                "A fixed decision was changed",
            ));
        }
    }
    // Productive capacity is checked separately from physical retention capacity.
    let mut productive: Vec<BTreeMap<Time, f64>> = vec![BTreeMap::new(); p.resources.len()];
    for a in &s.assignments {
        for activity in &a.activities {
            for segment in &activity.segments {
                for reservation in &activity.reservations {
                    let start = segment.start.max(reservation.start);
                    let end = segment.end.min(reservation.end);
                    if start < end
                        && let Some(&ri) = c.resources.get(reservation.resource.as_str())
                    {
                        *productive[ri].entry(start).or_default() += reservation.amount;
                        *productive[ri].entry(end).or_default() -= reservation.amount;
                    }
                }
            }
        }
    }
    for (ri, events) in productive.iter_mut().enumerate() {
        let r = &p.resources[ri];
        for w in &r.calendar {
            events.entry(w.start).or_default();
            events.entry(w.end).or_default();
        }
        let mut used = 0.0;
        for (&at, &delta) in events.iter() {
            used += delta;
            if used > window_at(&r.calendar, at).map_or(0.0, |w| w.capacity) + 1e-7
                && at < p.horizon
            {
                errors.push(Diagnostic::new(
                    "PROCESSING_CAPACITY",
                    &r.id,
                    format!("Productive demand exceeds capacity at {at}"),
                ));
            }
        }
    }
    let (job_completions, order_completions) = crate::domain::completions(p, s);
    if s.job_completions != job_completions || s.order_completions != order_completions {
        errors.push(Diagnostic::new(
            "COMPLETION_METRICS",
            &p.id,
            "Job or order completion differs from assignments",
        ));
    }
    for job in &p.jobs {
        if job_completions
            .get(&job.id)
            .is_none_or(|end| job.deadline.is_some_and(|d| *end > d))
        {
            errors.push(Diagnostic::new(
                "JOB_COMPLETION",
                &job.id,
                "Job has no active work or violates its deadline",
            ));
        }
    }
    for order in &p.orders {
        if order_completions
            .get(&order.id)
            .is_none_or(|end| order.deadline.is_some_and(|d| *end > d))
        {
            errors.push(Diagnostic::new(
                "ORDER_COMPLETION",
                &order.id,
                "Order is incomplete or violates its deadline",
            ));
        }
    }
    if s.metric_version != 0 || !s.metrics.is_empty() {
        let expected = rules::metrics(p, s);
        if s.metric_version > 1
            || expected
                .iter()
                .filter(|(k, _)| {
                    s.metric_version == 1
                        || crate::metrics::ORIGINAL.contains(&k.as_str())
                        || s.metrics.contains_key(*k)
                        || rules::objectives(p, &[]).iter().any(|o| &o.metric == *k)
                })
                .any(|(k, v)| {
                    s.metrics
                        .get(k)
                        .is_none_or(|got| !got.is_finite() || (got - v).abs() > 1e-6)
                })
            || s.score != rules::score(p, &expected)
            || (!s.objective_values.is_empty()
                && s.objective_values != rules::objective_values(p, &expected, &[]))
        {
            errors.push(Diagnostic::new(
                "METRICS",
                &p.id,
                "Metrics or objective score differ from recomputation",
            ));
        }
    }
    Validation {
        valid: errors.is_empty(),
        diagnostics: errors,
    }
}

fn check_activity(
    p: &Problem,
    a: &Activity,
    m: &Mode,
    role: &str,
    task: &str,
    errors: &mut Vec<Diagnostic>,
) {
    let mut fail = |code: &str, msg: &str| errors.push(Diagnostic::new(code, &a.id, msg));
    if a.task != task || a.role != role || a.start < 0 || a.end < a.start || a.end > p.horizon {
        fail("ACTIVITY", "Invalid activity identity or envelope");
    }
    let resource: HashMap<_, _> = p.resources.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut expected = Vec::new();
    let mut cursor = a.start;
    let mut total_segments = 0;
    for (phase_index, ph) in m.phases.iter().enumerate() {
        let ss: Vec<_> = a.segments.iter().filter(|s| s.phase == ph.id).collect();
        total_segments += ss.len();
        if m.contiguous && phase_index > 0 && ss.first().is_some_and(|s| s.start != cursor) {
            fail("PHASE_CONTINUITY", "Contiguous phases have a waiting gap");
        }
        let mut work = 0.0;
        for (i, s) in ss.iter().enumerate() {
            if s.start < cursor
                || s.end <= s.start
                || s.start < a.start
                || s.end > a.end
                || !s.work.is_finite()
                || s.work < 0.0
            {
                fail("SEGMENT", "Invalid or unordered execution segment");
                continue;
            }
            if i > 0 && s.start > cursor {
                if ph.interruption == Interrupt::NonInterruptible {
                    fail("INTERRUPTION", "Non-interruptible phase contains a pause");
                } else {
                    let mut t = cursor;
                    while t < s.start {
                        let closed = ph.requirements.iter().any(|req| {
                            window_at(&resource[req.resource.as_str()].calendar, t).is_none_or(
                                |w| {
                                    w.capacity + 1e-8 < req.amount
                                        || ph.rate_resource.as_ref() == Some(&req.resource)
                                            && w.rate == 0.0
                                },
                            )
                        });
                        if !closed {
                            fail(
                                "UNDECLARED_PAUSE",
                                "Pause occurs while processing calendars allow work",
                            );
                            break;
                        }
                        t = ph
                            .requirements
                            .iter()
                            .map(|req| {
                                boundary(&resource[req.resource.as_str()].calendar, t, s.start)
                            })
                            .min()
                            .unwrap_or(s.start);
                    }
                }
            }
            let mut t = s.start;
            let mut possible = 0.0;
            let mut last_rate = 1.0;
            while t < s.end {
                let mut next = s.end;
                let mut rate = 1.0;
                for req in &ph.requirements {
                    let r = resource[req.resource.as_str()];
                    if let Some(w) = window_at(&r.calendar, t) {
                        if w.capacity + 1e-8 < req.amount {
                            fail("CALENDAR", "Demand exceeds calendar capacity");
                        }
                        if ph.rate_resource.as_ref() == Some(&req.resource) {
                            rate = w.rate;
                        }
                    } else {
                        fail("CALENDAR", "Work occurs outside processing availability");
                        rate = 0.0;
                    }
                    next = next.min(boundary(&r.calendar, t, s.end));
                }
                possible += (next - t) as f64 * rate;
                last_rate = rate;
                t = next;
            }
            if s.work > possible + 1e-7 || possible - s.work >= last_rate + 1e-7 {
                fail(
                    "WORK_RATE",
                    "Reported progress differs from processing rate and one-tick rounding",
                );
            }
            work += s.work;
            cursor = s.end;
        }
        if (work - ph.work.unwrap_or(0.0)).abs() > 1e-6 {
            fail("WORK_AMOUNT", "Required work is missing or counted twice");
        }
        if let (Some(first), Some(last)) = (ss.first(), ss.last()) {
            for req in &ph.requirements {
                if req.resource == m.primary {
                    continue;
                }
                let intervals = if req.retain {
                    vec![(first.start, last.end)]
                } else {
                    ss.iter().map(|s| (s.start, s.end)).collect()
                };
                for (start, end) in intervals {
                    expected.push(Reservation {
                        resource: req.resource.clone(),
                        start,
                        end,
                        amount: req.amount,
                    });
                }
            }
        }
    }
    if total_segments != a.segments.len() {
        fail("PHASE", "Unexpected phase segments");
    }
    if a.segments.first().is_some_and(|s| s.start != a.start)
        || a.segments.last().is_some_and(|s| s.end != a.end)
        || a.segments.is_empty() && a.start != a.end
    {
        fail(
            "ENVELOPE",
            "Activity envelope differs from execution segments",
        );
    }
    if a.end > a.start {
        let amount = m.phases[0]
            .requirements
            .iter()
            .find(|r| r.resource == m.primary)
            .unwrap()
            .amount;
        expected.push(Reservation {
            resource: m.primary.clone(),
            start: a.start,
            end: a.end,
            amount,
        });
    }
    let sort = |v: &mut Vec<Reservation>| {
        v.sort_by(|a, b| {
            a.resource
                .cmp(&b.resource)
                .then(a.start.cmp(&b.start))
                .then(a.end.cmp(&b.end))
                .then(a.amount.total_cmp(&b.amount))
        })
    };
    let mut actual = a.reservations.clone();
    sort(&mut expected);
    sort(&mut actual);
    if actual != expected {
        fail(
            "RESERVATION_COVERAGE",
            "Reservations differ from required work/retention intervals",
        );
    }
}

pub fn validate_customized(
    p: &Problem,
    s: &Schedule,
    extension: &dyn rules::Customization,
) -> Validation {
    match crate::extensions::lower(p, extension) {
        Ok(mut lowered) => {
            lowered.customization = None;
            crate::extensions::validate_lowered(&lowered, s, extension)
        }
        Err(diagnostics) => Validation {
            valid: false,
            diagnostics,
        },
    }
}
