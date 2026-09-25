use crate::model::*;
use std::collections::{HashMap, HashSet, VecDeque};

pub struct Compiled<'a> {
    pub urgency: Vec<crate::urgency::Urgency>,
    pub problem: &'a Problem,
    pub resources: HashMap<&'a str, usize>,
    pub tasks: HashMap<&'a str, usize>,
    pub predecessors: Vec<Vec<(usize, Time, Option<Time>)>>,
    pub successors: Vec<Vec<usize>>,
    /// Construction order also contains resource sequence locks. These do not wait
    /// for product release on independent post-processing resources.
    pub dispatch_predecessors: Vec<Vec<usize>>,
    pub dispatch_successors: Vec<Vec<usize>>,
}

pub fn compile(p: &Problem) -> Result<Compiled<'_>, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    crate::domain::check(p, &mut errors);
    crate::conditionals::check(p, &mut errors);
    crate::language::check(p, &mut errors);
    let extension = p
        .customization
        .as_ref()
        .and_then(|r| crate::extensions::registered(r).ok());
    let definitions = crate::queues::definitions(p, extension.as_deref());
    let native_objectives = extension
        .as_ref()
        .map(|e| e.objectives(p))
        .unwrap_or_else(|| p.native_objectives.clone());
    let policy = p
        .queue_policy
        .clone()
        .unwrap_or_else(|| crate::queues::default_policy(p, &definitions));
    if let Err(e) = crate::queues::check(p, &policy, &definitions) {
        errors.push(e);
    }
    for d in &p.queue_definitions {
        if !d.attribute.is_empty()
            && p.tasks
                .iter()
                .any(|t| !t.attributes.contains_key(&d.attribute))
        {
            errors.push(Diagnostic::new(
                "QUEUE_ATTRIBUTE",
                &d.id,
                "Declared attribute proxy requires its attribute on every task",
            ));
        }
    }
    for zone in &p.freeze_zones {
        if zone.until < 0
            || zone.dimensions.is_empty()
            || zone
                .dimensions
                .iter()
                .any(|d| !["start", "resource", "mode", "order"].contains(&d.as_str()))
            || zone.until > p.horizon
            || !p.resources.iter().any(|r| r.id == zone.resource)
            || zone
                .tasks
                .iter()
                .any(|id| !p.tasks.iter().any(|t| &t.id == id))
        {
            errors.push(Diagnostic::new(
                "FREEZE_ZONE",
                &zone.resource,
                "Invalid freeze provenance",
            ));
        }
    }
    if let Some(reference) = &p.customization
        && let Err(mut e) = crate::extensions::registered(reference)
    {
        errors.append(&mut e);
    }
    let mut emit = |code: &str, id: &str, msg: &str| errors.push(Diagnostic::new(code, id, msg));
    if !["apex.v3.1", "apex.v3.2", "apex.v3.3", "apex.v3.4"].contains(&p.schema_version.as_str()) {
        emit(
            "SCHEMA_VERSION",
            &p.id,
            "Expected executable apex.v3.1 through apex.v3.4",
        );
    }
    if p.horizon <= 0 || p.horizon > 1_000_000_000_000 {
        emit(
            "HORIZON",
            &p.id,
            "Horizon must be positive and at most 10^12 seconds",
        );
    }
    let mut resources = HashMap::new();
    for (i, r) in p.resources.iter().enumerate() {
        if r.id.is_empty() || resources.insert(r.id.as_str(), i).is_some() {
            emit(
                "DUPLICATE_ID",
                &r.id,
                "Resource IDs must be nonempty and unique",
            );
        }
        if !r.capacity.is_finite() || r.capacity <= 0.0 {
            emit("CAPACITY", &r.id, "Capacity must be finite and positive");
        }
        for windows in [&r.calendar, &r.retention_calendar] {
            let mut end = 0;
            for w in windows {
                if w.start < end
                    || w.start < 0
                    || w.end <= w.start
                    || w.end > p.horizon
                    || !w.capacity.is_finite()
                    || w.capacity < 0.0
                    || w.capacity > r.capacity
                    || !w.rate.is_finite()
                    || w.rate < 0.0
                    || w.rate > 1e6
                {
                    emit(
                        "CALENDAR",
                        &r.id,
                        "Windows must be sorted, disjoint, in horizon, with bounded nonnegative capacity/rate",
                    );
                }
                end = w.end;
            }
        }
    }
    let mut tasks = HashMap::new();
    for (i, t) in p.tasks.iter().enumerate() {
        if t.id.is_empty() || tasks.insert(t.id.as_str(), i).is_some() {
            emit(
                "DUPLICATE_ID",
                &t.id,
                "Task IDs must be nonempty and unique",
            );
        }
        if t.release < 0
            || t.release > p.horizon
            || !t.priority.is_finite()
            || t.priority < 0.0
            || t.due.is_some_and(|x| x < 0)
            || t.deadline.is_some_and(|x| x < t.release || x > p.horizon)
        {
            emit(
                "TASK_BOUNDS",
                &t.id,
                "Invalid release, deadline, due date or priority",
            );
        }
        for (name, amount) in t.consume.iter().chain(&t.produce).chain(&t.attributes) {
            if !amount.is_finite() || *amount < 0.0 {
                emit("QUANTITY", &t.id, &format!("Invalid value for {name}"));
            }
        }
    }
    for t in &p.tasks {
        check_modes(&t.id, &t.modes, &resources, p, &mut errors);
        let mut ids = HashSet::new();
        for c in t
            .pre
            .iter()
            .chain(&t.post)
            .chain(t.execution.iter().flat_map(|e| &e.restart))
        {
            if !ids.insert(&c.id) {
                errors.push(Diagnostic::new(
                    "DUPLICATE_ID",
                    &t.id,
                    "Conditional IDs must be unique per task",
                ));
            }
            check_conditional_modes(&t.id, &c.modes, &resources, p, &mut errors);
        }
        for mode in &t.modes {
            for (base, extra) in [(&t.pre, &mode.pre), (&t.post, &mode.post)] {
                let specs: Vec<_> = base
                    .iter()
                    .chain(extra)
                    .map(|c| (c.id.clone(), c))
                    .collect();
                if let Err(e) = crate::activities::graph(&specs, p.horizon) {
                    errors.push(e);
                }
            }
            for conditional in mode.pre.iter().chain(&mode.post) {
                check_conditional_modes(&t.id, &conditional.modes, &resources, p, &mut errors);
                if conditional
                    .modes
                    .iter()
                    .any(|m| !m.pre.is_empty() || !m.post.is_empty())
                {
                    errors.push(Diagnostic::new(
                        "NESTED_CONDITIONAL",
                        &conditional.id,
                        "Use stage dependencies instead of recursive conditional nesting",
                    ));
                }
            }
        }
        if let Some(e) = &t.execution {
            if let Some(m) = t.modes.iter().find(|m| m.id == e.mode) {
                if e.remaining_work.len() != m.phases.len()
                    || m.phases.iter().any(|ph| {
                        !e.remaining_work
                            .get(&ph.id)
                            .is_some_and(|v| v.is_finite() && *v >= 0.0 && *v <= 1e12)
                    })
                {
                    errors.push(Diagnostic::new(
                        "REMAINING_WORK",
                        &t.id,
                        "Supply remaining work for every selected phase",
                    ));
                }
            } else {
                errors.push(Diagnostic::new(
                    "EXECUTION_MODE",
                    &t.id,
                    "Unknown execution mode",
                ));
            }
            if e.as_of < 0
                || e.as_of > p.horizon
                || e.actual.task != t.id
                || e.actual.mode != e.mode
                || e.actual.end > e.as_of
                || e.actual.start < 0
                || e.actual.start > e.actual.end
                || e.actual.segments.iter().any(|s| {
                    s.start < e.actual.start
                        || s.end > e.actual.end
                        || s.start >= s.end
                        || !s.work.is_finite()
                        || s.work < 0.0
                })
                || e.actual.reservations.iter().any(|r| {
                    !resources.contains_key(r.resource.as_str())
                        || r.start < 0
                        || r.end > e.as_of
                        || r.start >= r.end
                        || !r.amount.is_finite()
                        || r.amount <= 0.0
                })
            {
                errors.push(Diagnostic::new(
                    "EXECUTION_HISTORY",
                    &t.id,
                    "Invalid actual activity or observation time",
                ));
            }
        }
    }
    let mut transitions = HashSet::new();
    for tr in &p.transitions {
        if !resources.contains_key(tr.resource.as_str())
            || !transitions.insert((&tr.resource, &tr.from, &tr.to))
        {
            errors.push(Diagnostic::new(
                "TRANSITION",
                &tr.id,
                "Unknown resource or duplicate transition pair",
            ));
        }
        if !tr.penalty.is_finite() || tr.penalty < 0.0 {
            errors.push(Diagnostic::new(
                "TRANSITION_PENALTY",
                &tr.id,
                "Penalty must be finite and nonnegative",
            ));
        }
        for c in tr.previous_post.iter().chain(&tr.next_pre) {
            check_conditional_modes(&tr.id, &c.modes, &resources, p, &mut errors);
        }
    }
    for r in &p.receipts {
        if r.at < 0 || r.at > p.horizon || !r.amount.is_finite() || r.amount < 0.0 {
            errors.push(Diagnostic::new(
                "RECEIPT",
                &r.item,
                "Invalid receipt time or quantity",
            ));
        }
    }
    if p.inventory.values().any(|v| !v.is_finite() || *v < 0.0) {
        errors.push(Diagnostic::new(
            "INVENTORY",
            &p.id,
            "Initial inventory must be nonnegative",
        ));
    }
    let mut objective_names = HashSet::new();
    for o in &p.objectives {
        if (!crate::metrics::supported(p, &o.metric)
            && !p
                .rules
                .iter()
                .any(|r| matches!(r,Rule::AttributeObjective{id,..} if id==&o.metric))
            && !p.planning.objectives.iter().any(|n| n.id == o.metric)
            && !native_objectives.iter().any(|n| n.metric == o.metric))
            || !objective_names.insert(&o.metric)
            || !o.weight.is_finite()
            || o.weight < 0.0
            || !o.scale.is_finite()
            || o.scale <= 0.0
        {
            errors.push(Diagnostic::new(
                "OBJECTIVE",
                &o.metric,
                "Unsupported or duplicate metric, invalid weight, or nonpositive scale",
            ));
        }
    }
    let mut rule_ids = HashSet::new();
    for rule in &p.rules {
        let id = match rule {
            Rule::SetupCharge { id, task, value } => {
                if !tasks.contains_key(task.as_str()) || !value.is_finite() || *value < 0.0 {
                    errors.push(Diagnostic::new("RULE", id, "Invalid task or setup charge"));
                }
                id
            }
            Rule::SequencePattern {
                id,
                resource,
                pattern,
                previous_post,
                next_pre,
                penalty,
            } => {
                if !resources.contains_key(resource.as_str())
                    || pattern.is_empty()
                    || !penalty.is_finite()
                    || *penalty < 0.0
                    || pattern.len() < 2 && !previous_post.is_empty()
                {
                    errors.push(Diagnostic::new(
                        "RULE",
                        id,
                        "Invalid resource, sequence pattern or penalty",
                    ));
                }
                for c in previous_post.iter().chain(next_pre) {
                    check_conditional_modes(id, &c.modes, &resources, p, &mut errors);
                }
                id
            }
            Rule::TaskWindow {
                id,
                task,
                earliest,
                latest,
            } => {
                if !tasks.contains_key(task.as_str())
                    || *earliest < 0
                    || latest < earliest
                    || *latest > p.horizon
                {
                    errors.push(Diagnostic::new("RULE", id, "Invalid task window"));
                }
                id
            }
            Rule::AttributeObjective {
                id,
                attribute,
                weight,
                ..
            } => {
                if crate::metrics::supported(p, id) {
                    errors.push(Diagnostic::new(
                        "RESERVED_METRIC",
                        id,
                        "An attribute objective cannot replace a built-in metric",
                    ));
                }
                if !weight.is_finite()
                    || *weight < 0.0
                    || p.tasks
                        .iter()
                        .any(|t| !t.attributes.contains_key(attribute))
                {
                    errors.push(Diagnostic::new(
                        "RULE",
                        id,
                        "Every task needs the objective attribute and a nonnegative weight",
                    ));
                }
                id
            }
        };
        if !rule_ids.insert(id) {
            errors.push(Diagnostic::new(
                "DUPLICATE_RULE",
                id,
                "Rule IDs must be unique",
            ));
        }
    }
    let mut predecessors = vec![vec![]; p.tasks.len()];
    let mut successors = vec![vec![]; p.tasks.len()];
    let mut dispatch_predecessors = vec![vec![]; p.tasks.len()];
    let mut dispatch_successors = vec![vec![]; p.tasks.len()];
    let mut add_edge = |a: &str, b: &str, min: Time, max: Option<Time>, temporal: bool| {
        if let (Some(&a), Some(&b)) = (tasks.get(a), tasks.get(b)) {
            if temporal {
                predecessors[b].push((a, min, max));
                successors[a].push(b);
            }
            dispatch_predecessors[b].push(a);
            dispatch_successors[a].push(b);
        } else {
            errors.push(Diagnostic::new(
                "REFERENCE",
                b,
                format!("Unknown dependency {a} -> {b}"),
            ));
        }
    };
    for d in &p.dependencies {
        add_edge(&d.before, &d.after, d.min_lag, d.max_lag, true);
    }
    for l in &p.locks {
        if let Lock::Order { tasks, .. } = l {
            for pair in tasks.windows(2) {
                add_edge(&pair[0], &pair[1], 0, None, false);
            }
        }
    }
    for d in &p.dependencies {
        if d.min_lag < 0
            || d.min_lag > p.horizon
            || d.max_lag.is_some_and(|m| m < d.min_lag || m > p.horizon)
        {
            errors.push(Diagnostic::new("LAG", &d.after, "Invalid temporal lag"));
        }
    }
    let mut block_members = HashSet::new();
    for l in &p.locks {
        let valid = match l {
            Lock::Mode { task, mode } => tasks
                .get(task.as_str())
                .is_some_and(|i| p.tasks[*i].modes.iter().any(|m| &m.id == mode)),
            Lock::Resource { task, resource } => tasks
                .get(task.as_str())
                .is_some_and(|i| p.tasks[*i].modes.iter().any(|m| &m.primary == resource)),
            Lock::Start { task, at } => {
                tasks.contains_key(task.as_str()) && *at >= 0 && *at < p.horizon
            }
            Lock::Order {
                resource,
                tasks: ids,
                consecutive,
            } => {
                resources.contains_key(resource.as_str())
                    && ids.len() >= 2
                    && ids.iter().collect::<HashSet<_>>().len() == ids.len()
                    && ids.iter().all(|id| {
                        tasks.get(id.as_str()).is_some_and(|i| {
                            p.tasks[*i].modes.iter().any(|m| &m.primary == resource)
                        })
                    })
                    && (!consecutive || ids.iter().all(|id| block_members.insert(id)))
            }
        };
        if !valid {
            errors.push(Diagnostic::new(
                "LOCK",
                &p.id,
                "Invalid lock reference, interval or overlapping block membership",
            ));
        }
    }
    let mut counts: Vec<_> = dispatch_predecessors.iter().map(Vec::len).collect();
    let mut q: VecDeque<_> = counts
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 0).then_some(i))
        .collect();
    let mut visited = 0;
    while let Some(i) = q.pop_front() {
        visited += 1;
        for &j in &dispatch_successors[i] {
            counts[j] -= 1;
            if counts[j] == 0 {
                q.push_back(j);
            }
        }
    }
    if p.routes.is_empty() && visited != p.tasks.len() {
        errors.push(Diagnostic::new(
            "CYCLE",
            &p.id,
            "Dependencies and order locks contain a cycle",
        ));
    }
    for e in &mut errors {
        if let Some(&i) = tasks.get(e.entity.as_str()) {
            e.source = p.tasks[i].source.clone();
        }
    }
    if errors.is_empty() {
        Ok(Compiled {
            urgency: crate::urgency::derive(p),
            problem: p,
            resources,
            tasks,
            predecessors,
            successors,
            dispatch_predecessors,
            dispatch_successors,
        })
    } else {
        Err(errors)
    }
}

fn check_modes(
    id: &str,
    modes: &[Mode],
    resources: &HashMap<&str, usize>,
    p: &Problem,
    errors: &mut Vec<Diagnostic>,
) {
    if modes.is_empty() {
        errors.push(Diagnostic::new(
            "MISSING_MODE",
            id,
            "At least one execution mode is required",
        ));
    }
    let mut seen = HashSet::new();
    for m in modes {
        if !seen.insert(&m.id)
            || !resources.contains_key(m.primary.as_str())
            || m.phases.is_empty()
            || !m.cost.is_finite()
            || m.cost < 0.0
        {
            errors.push(Diagnostic::new(
                "MODE",
                id,
                "Invalid or duplicate mode, primary resource or phases",
            ));
        }
        let mut phases = HashSet::new();
        for ph in &m.phases {
            if !phases.insert(&ph.id) || ph.requirements.is_empty() {
                errors.push(Diagnostic::new(
                    "PHASE",
                    id,
                    "Duplicate phase or missing requirements",
                ));
            }
            if (ph.work.is_none() && ph.work_per_unit.is_none())
                || ph
                    .work
                    .is_some_and(|v| !v.is_finite() || !(0.0..=1e12).contains(&v))
                || ph
                    .work_per_unit
                    .is_some_and(|v| !v.is_finite() || !(0.0..=1e12).contains(&v))
            {
                errors.push(Diagnostic::new(
                    "MISSING_PROCESSING_TIME",
                    id,
                    format!(
                        "Mode {} phase {} needs explicit nonnegative work seconds",
                        m.id, ph.id
                    ),
                ));
            }
            let mut reqs = HashSet::new();
            for req in &ph.requirements {
                if !reqs.insert(&req.resource)
                    || !resources.get(req.resource.as_str()).is_some_and(|i| {
                        req.amount > 0.0
                            && req.amount.is_finite()
                            && req.amount <= p.resources[*i].capacity
                    })
                {
                    errors.push(Diagnostic::new(
                        "REQUIREMENT",
                        id,
                        "Unknown resource, duplicate requirement or impossible demand",
                    ));
                }
            }
            if !reqs.contains(&m.primary)
                || ph.rate_resource.as_ref().is_some_and(|r| !reqs.contains(r))
            {
                errors.push(Diagnostic::new(
                    "PRIMARY_RESOURCE",
                    id,
                    "Primary and optional rate resource must be required in every phase",
                ));
            }
        }
        if let Some(first) = m
            .phases
            .first()
            .and_then(|ph| ph.requirements.iter().find(|r| r.resource == m.primary))
            && m.phases.iter().any(|ph| {
                ph.requirements
                    .iter()
                    .any(|r| r.resource == m.primary && (r.amount != first.amount || !r.retain))
            })
        {
            errors.push(Diagnostic::new(
                "PRIMARY_RETENTION",
                id,
                "Primary demand must be constant and retained across phases and pauses",
            ));
        }
    }
}

fn check_conditional_modes(
    id: &str,
    modes: &[Mode],
    resources: &HashMap<&str, usize>,
    p: &Problem,
    errors: &mut Vec<Diagnostic>,
) {
    check_modes(id, modes, resources, p, errors);
    if modes.iter().any(|m| {
        !m.pre.is_empty() || !m.post.is_empty() || m.consume.is_some() || m.produce.is_some()
    }) {
        errors.push(Diagnostic::new("NESTED_CONDITIONAL",id,"Conditional modes must use phases/stage dependencies; nested activities and material overrides belong to main tasks"));
    }
}
