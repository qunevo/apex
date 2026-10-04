//! XT: UCT-guided tree search over admissible construction prefixes.
use crate::{model::*, queues, rules::Customization, search::*, xg};
use std::time::Instant;

pub const ID: &str = "XT";

struct Node {
    options: Options,
    children: Vec<usize>,
    untried: Vec<Options>,
    expanded: bool,
    visits: usize,
    reward: f64,
}
fn expand(
    p: &Problem,
    node: &Options,
    extension: Option<&dyn Customization>,
    errors: &mut Vec<Diagnostic>,
) -> Vec<Options> {
    match crate::material::branches(p, node, extension).and_then(|branches| {
        if branches.is_some() {
            Ok(branches)
        } else {
            crate::conditionals::branches(p, node, extension)
        }
    }) {
        Ok(Some(mut children)) => {
            children.reverse();
            return children;
        }
        Err(e) => {
            if errors.is_empty() {
                errors.extend(e);
            }
            return vec![];
        }
        Ok(None) => {}
    }
    if node.decision_prefix.len() >= node.xt.depth {
        return vec![];
    }
    let mut children = xg::next_choices(p, node, extension)
        .unwrap_or_else(|e| {
            if errors.is_empty() {
                errors.extend(e);
            }
            vec![]
        })
        .into_iter()
        .map(|d| {
            let mut o = node.clone();
            o.decision_prefix.push(d);
            o
        })
        .collect::<Vec<_>>();
    // Keep route choices in the tree as well; a fixed route or frozen task remains protected by resolution.
    if node.decision_prefix.is_empty() && node.route_choices.is_empty() {
        for route in &p.routes {
            if route.selected.is_none() {
                for alternative in &route.alternatives {
                    let mut o = node.clone();
                    o.route_choices
                        .insert(route.id.clone(), alternative.id.clone());
                    children.push(o);
                }
            }
            if children.len() >= node.xt.branching * 2 {
                break;
            }
        }
    }
    children.reverse();
    children
}
fn reward(reference: &[f64], score: &[f64]) -> f64 {
    for (a, b) in reference.iter().zip(score) {
        if a != b {
            return 1.0 / (1.0 + ((b - a) / (a.abs() + 1.0)).clamp(-30.0, 30.0).exp());
        }
    }
    0.5
}
pub fn search(p: &Problem, o: &Options) -> Outcome {
    search_customized(p, o, None)
}
pub fn search_customized(
    p: &Problem,
    o: &Options,
    provided: Option<&dyn Customization>,
) -> Outcome {
    search_observed(p, o, provided, None, &mut |_, _| {})
}
pub(crate) fn search_observed(
    p: &Problem,
    o: &Options,
    provided: Option<&dyn Customization>,
    initial: Option<Schedule>,
    observe: &mut dyn FnMut(&Options, &Outcome),
) -> Outcome {
    if crate::language::needs_lowering(p) {
        return search_observed(&crate::language::lower(p)?, o, provided, initial, observe);
    }
    if o.xh.max_evaluations.unwrap_or(o.iterations) == 0 && o.budget_ms == 0 {
        return Err(vec![Diagnostic::new(
            "SEARCH_OPTIONS",
            "xt",
            "XT requires an evaluation or time limit",
        )]);
    }
    let owned = p
        .customization
        .as_ref()
        .map(crate::extensions::registered)
        .transpose()?;
    let extension = provided.or(owned.as_deref());
    let max = limits(o)?;
    let start = Instant::now();
    let mut base = queue_options(p, o, extension);
    base.top_k = 2;
    let mut nodes = vec![Node {
        options: base,
        children: vec![],
        untried: vec![],
        expanded: false,
        visits: 0,
        reward: 0.0,
    }];
    // An internal handoff carries an already validated seed. Do not evaluate it twice.
    let seeded = initial.is_some();
    let initial = initial.map_or_else(|| xg::evaluate(p, o, extension), Ok);
    if !seeded {
        observe(o, &initial);
    }
    let mut errors = vec![];
    let mut best = None;
    let mut reference = vec![];
    let mut report = SearchReport {
        algorithm: ID.into(),
        workers: o.xt.workers,
        evaluations: usize::from(!seeded),
        unmapped_objectives: queues::unmapped(p, &queues::definitions(p, extension)),
        ..Default::default()
    };
    match initial {
        Ok(s) => {
            reference = s.score.clone();
            archive(&mut report, &s, o.xh.archive_limit);
            progress(&mut report, &s, start);
            best = Some(s);
        }
        Err(e) => {
            errors = e;
            report.failed_evaluations += 1;
        }
    }
    while report.evaluations < max && !time_up(start, o) {
        let mut jobs = vec![];
        let mut paths = vec![];
        for worker in 0..o.xt.workers.min(max - report.evaluations) {
            let mut index = 0;
            let mut path = vec![0];
            loop {
                if !nodes[index].expanded {
                    nodes[index].untried = expand(p, &nodes[index].options, extension, &mut errors);
                    nodes[index].expanded = true;
                }
                if let Some(options) = nodes[index].untried.pop() {
                    let next = nodes.len();
                    nodes.push(Node {
                        options,
                        children: vec![],
                        untried: vec![],
                        expanded: false,
                        visits: 0,
                        reward: 0.0,
                    });
                    nodes[index].children.push(next);
                    index = next;
                    path.push(index);
                    break;
                }
                if nodes[index].children.is_empty() {
                    break;
                }
                let parent_visits = nodes[index].visits.max(1) as f64;
                index = *nodes[index]
                    .children
                    .iter()
                    .max_by(|a, b| {
                        let uct = |i: usize| {
                            if nodes[i].visits == 0 {
                                f64::INFINITY
                            } else {
                                nodes[i].reward / nodes[i].visits as f64
                                    + o.xt.exploration
                                        * (parent_visits.ln() / nodes[i].visits as f64).sqrt()
                            }
                        };
                        uct(**a).total_cmp(&uct(**b)).then(b.cmp(a))
                    })
                    .unwrap();
                path.push(index);
            }
            if nodes[index].visits > 0 {
                report.revisits += 1;
            }
            for i in &path {
                nodes[*i].visits += 1;
            }
            let mut options = nodes[index].options.clone();
            options.seed = o.seed.wrapping_add((report.evaluations + worker) as u64);
            jobs.push(options);
            paths.push(path);
        }
        for ((path, options), result) in
            paths
                .iter()
                .zip(&jobs)
                .zip(parallel(p, &jobs, o.xt.workers, extension))
        {
            report.evaluations += 1;
            observe(options, &result);
            let value = match result {
                Ok(s) => {
                    if reference.is_empty() {
                        reference = s.score.clone();
                    }
                    let value = reward(&reference, &s.score);
                    archive(&mut report, &s, o.xh.archive_limit);
                    if best.as_ref().is_none_or(|b| s.score < b.score) {
                        progress(&mut report, &s, start);
                        best = Some(s);
                    }
                    value
                }
                Err(e) => {
                    errors = e;
                    report.failed_evaluations += 1;
                    0.0
                }
            };
            for i in path {
                nodes[*i].reward += value;
            }
        }
    }
    report.nodes = nodes.len();
    report.stop_reason = if report.evaluations >= max {
        "evaluation_limit"
    } else {
        "time_limit"
    }
    .into();
    if let Some(mut s) = best {
        s.evaluations = report.evaluations;
        s.elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
        s.search = Some(report);
        Ok(s)
    } else {
        Err(errors)
    }
}
