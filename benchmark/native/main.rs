// The benchmark compiles unchanged APEX modules in a separate executable.
#[path = "../../src/rust/activities.rs"]
pub mod activities;
#[path = "../../src/rust/calendar.rs"]
pub mod calendar;
#[path = "../../src/rust/compile.rs"]
pub mod compile;
#[path = "../../src/rust/conditionals.rs"]
pub mod conditionals;
#[path = "../../src/rust/demo.rs"]
pub mod demo;
#[path = "../../src/rust/dispatch.rs"]
pub mod dispatch;
#[path = "../../src/rust/domain.rs"]
pub mod domain;
#[path = "../../customizations/dummy_customer/policy.rs"]
pub mod dummy_customer;
#[path = "../../src/rust/engine.rs"]
pub mod engine;
#[path = "../../src/rust/evolution.rs"]
pub mod evolution;
#[path = "../../src/rust/extensions.rs"]
pub mod extensions;
#[path = "../../src/rust/improve.rs"]
pub mod improve;
#[path = "../../src/rust/language.rs"]
pub mod language;
#[path = "../../src/rust/material.rs"]
pub mod material;
#[path = "../../src/rust/metrics.rs"]
pub mod metrics;
#[path = "../../src/rust/model.rs"]
pub mod model;
#[path = "../../src/rust/placement.rs"]
mod placement;
#[path = "../../src/rust/policy.rs"]
pub mod policy;
#[path = "../../src/rust/production.rs"]
pub mod production;
#[path = "../../src/rust/queues.rs"]
pub mod queues;
#[path = "../../src/rust/rules.rs"]
pub mod rules;
#[path = "../../src/rust/search.rs"]
pub mod search;
#[path = "../../src/rust/service.rs"]
pub mod service;
#[path = "../../src/rust/transport.rs"]
pub mod transport;
#[path = "../../src/rust/urgency.rs"]
pub mod urgency;
#[path = "../../src/rust/validate.rs"]
pub mod validate;

use model::*;
use rules::Customization;
use serde_json::{Value, json};
use std::io::{self, Read};
use std::time::Instant;

struct PermutationPolicy;
impl Customization for PermutationPolicy {
    fn id(&self) -> &str {
        "benchmark_pfsp"
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
    fn supports_prefix_decoration(&self) -> bool {
        true
    }
    fn filter_candidates(
        &self,
        ctx: &policy::Context<'_>,
    ) -> Result<Vec<policy::Rejection>, Diagnostic> {
        let first: Vec<_> = ctx
            .prefix
            .order
            .iter()
            .filter_map(|&i| {
                let task = &ctx.problem.tasks[i];
                (task.attributes["operation"] == 0.0).then_some(task.job.as_ref().unwrap())
            })
            .collect();
        let mut counts = vec![0; ctx.problem.resources.len()];
        for &i in &ctx.prefix.order {
            counts[ctx.problem.tasks[i].attributes["operation"] as usize] += 1;
        }
        Ok(ctx
            .candidates
            .iter()
            .filter_map(|c| {
                let task = &ctx.problem.tasks[c.task];
                let k = task.attributes["operation"] as usize;
                (k > 0 && first.get(counts[k]).copied() != task.job.as_ref()).then(|| {
                    policy::Rejection {
                        task: c.task,
                        mode: c.mode,
                        reason: "Preserve the first-machine job permutation".into(),
                    }
                })
            })
            .collect())
    }
    fn validate(&self, p: &Problem, s: &Schedule) -> Vec<Diagnostic> {
        let mut first: Option<Vec<&str>> = None;
        for resource in &p.resources {
            let mut rows: Vec<_> = s
                .assignments
                .iter()
                .filter(|a| a.primary == resource.id)
                .collect();
            rows.sort_by_key(|a| a.start);
            let sequence: Vec<_> = rows
                .iter()
                .map(|a| a.task.split('_').next().unwrap())
                .collect();
            if let Some(ref expected) = first {
                if &sequence != expected {
                    return vec![Diagnostic::new(
                        "PFSP_PERMUTATION",
                        &resource.id,
                        "Different job order",
                    )];
                }
            } else {
                first = Some(sequence);
            }
        }
        vec![]
    }
}
fn compact(s: &Schedule) -> Value {
    let assignments: Vec<_> = s
        .assignments
        .iter()
        .map(|a| {
            let (j, k) = a.task.split_once('_').unwrap();
            json!([
                j[1..].parse::<usize>().unwrap(),
                k[1..].parse::<usize>().unwrap(),
                a.primary[1..].parse::<usize>().unwrap(),
                a.start,
                a.ready
            ])
        })
        .collect();
    json!({"objectives":[s.metrics["makespan"],s.metrics["job_flow_time"]], "schedule":assignments})
}
fn dominates(a: &[f64; 2], b: &[f64; 2]) -> bool {
    a[0] <= b[0] && a[1] <= b[1]
}
fn run(request: Value) -> Result<Value, String> {
    let p: Problem =
        serde_json::from_value(request["problem"].clone()).map_err(|e| e.to_string())?;
    let method = request["method"].as_str().ok_or("Missing method")?;
    let max = request["evaluations"]
        .as_u64()
        .ok_or("Missing evaluations")? as usize;
    let mut options = Options {
        strategy: "queues".into(),
        iterations: max,
        budget_ms: 0,
        seed: request["seed"].as_u64().unwrap_or(42),
        ..Default::default()
    };
    options.trainer.selection = "pareto".into();
    if let Some(config) = request.get("evolution") {
        options.evolution = serde_json::from_value(config.clone()).map_err(|e| e.to_string())?;
    }
    let phase_names: Vec<_> = method.split('-').collect();
    let extension: Option<&dyn Customization> = if request["kind"] == "PFSP" {
        Some(&PermutationPolicy)
    } else {
        None
    };
    let started = Instant::now();
    let mut archive: Vec<([f64; 2], Value)> = vec![];
    let mut trace = vec![];
    let mut phases = vec![];
    let mut completed = 0;
    let mut failures = 0;
    let mut used = 0;
    let mut incumbent: Option<Schedule> = None;
    let mut policy_seed: Option<Schedule> = None;
    for (phase_index, name) in phase_names.iter().enumerate() {
        let mut current = options.clone();
        current.iterations = (max - used) / (phase_names.len() - phase_index);
        if let Some(previous) = &policy_seed
            && *name == "B"
            && let Some(replay) = &previous.replay
        {
            current.queue_policy = replay.queue_policy.clone();
            current.route_choices = replay.route_choices.clone();
            current.mode_choices = replay.mode_choices.clone();
            current.conditional_choices = replay.conditional_choices.clone();
        }
        let phase_start = Instant::now();
        let before_completed = completed;
        let tree_initial = policy_seed.clone();
        let mut observe = |evaluated_options: &Options,
                           result: &Result<Schedule, Vec<Diagnostic>>| {
            completed += 1;
            match result {
                Ok(s) => {
                    // Preserve a genuinely evaluated Q policy even if a classic
                    // reference strategy wins Trainer's scalar incumbent score.
                    if *name == "T"
                        && (evaluated_options.strategy == "queues"
                            || evaluated_options.queue_policy.is_some())
                        && policy_seed.as_ref().is_none_or(|old| s.score < old.score)
                    {
                        let mut seed = s.clone();
                        seed.replay = Some(Box::new(search::queue_options(
                            &p,
                            evaluated_options,
                            extension,
                        )));
                        policy_seed = Some(seed);
                    }
                    let f = [s.metrics["makespan"], s.metrics["job_flow_time"]];
                    let timestamp = started.elapsed().as_secs_f64();
                    let mut added = false;
                    if !archive.iter().any(|(a, _)| dominates(a, &f)) {
                        archive.retain(|(a, _)| !dominates(&f, a));
                        archive.push((f, compact(s)));
                        added = true;
                    }
                    trace.push(json!({"evaluation":completed,"seconds":timestamp,"objectives":f,"archive_changed":added,"phase":name}));
                }
                Err(e) => {
                    failures += 1;
                    trace.push(json!({"evaluation":completed,"seconds":started.elapsed().as_secs_f64(),"error":e,"phase":name}));
                }
            }
        };
        let result = match *name {
            "F" => {
                let result = engine::evaluate(&p, &current, extension);
                observe(&current, &result);
                result
            }
            "T" => search::train_observed(&p, &current, extension, &mut observe),
            "B" => search::plus_observed(&p, &current, extension, tree_initial, &mut observe),
            "G" => evolution::evolve_observed(
                &p,
                &current,
                extension,
                incumbent.as_slice(),
                &mut observe,
            ),
            _ => return Err(format!("Unknown phase {name}")),
        };
        match result {
            Ok(mut s) => {
                let count = s.evaluations;
                used += count;
                let report = s.search.take();
                phases.push(json!({"phase":name,"seconds":phase_start.elapsed().as_secs_f64(),
                    "evaluations":count,"observed_attempts":completed-before_completed,
                    "nodes":report.as_ref().map(|r|r.nodes),"operators":report.as_ref().map(|r|&r.operators),
                    "stop_reason":report.as_ref().map(|r|&r.stop_reason),
                    "settings":current}));
                if incumbent.as_ref().is_none_or(|old| s.score < old.score) {
                    incumbent = Some(s);
                }
            }
            Err(e) => return Err(serde_json::to_string(&e).unwrap()),
        }
    }
    let seconds = started.elapsed().as_secs_f64();
    archive.sort_by(|a, b| a.0[0].total_cmp(&b.0[0]));
    Ok(
        json!({"seconds":seconds,"evaluations":used,"observed_attempts":completed,"failed_evaluations":failures,
        "archive":archive.into_iter().map(|(_,v)|v).collect::<Vec<_>>(),"trace":trace,"phases":phases,
        "method":method,"options":options,"handoff":"Single validated incumbent; T-to-B transfers replay policy; GA receives an unpinned schedule genome"}),
    )
}
fn main() {
    let mut text = String::new();
    io::stdin().read_to_string(&mut text).unwrap();
    let result = serde_json::from_str(&text)
        .map_err(|e| e.to_string())
        .and_then(run);
    match result {
        Ok(value) => println!("{value}"),
        Err(error) => {
            println!("{}", json!({"error":error}));
            std::process::exit(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn equal_points_dominate_and_tradeoffs_do_not() {
        assert!(dominates(&[1.0, 2.0], &[1.0, 2.0]));
        assert!(!dominates(&[1.0, 3.0], &[2.0, 2.0]));
    }
    #[test]
    fn pfsp_policy_rejects_mismatched_order() {
        let p: Problem = serde_json::from_value(json!({
            "id":"synthetic","horizon":10,"resources":[
                {"id":"M0","calendar":[{"start":0,"end":10}]},
                {"id":"M1","calendar":[{"start":0,"end":10}]}]
        }))
        .unwrap();
        let mut s = Schedule::default();
        for (task, machine, start) in [
            ("J0_O0", "M0", 0),
            ("J1_O0", "M0", 1),
            ("J1_O1", "M1", 2),
            ("J0_O1", "M1", 3),
        ] {
            let a: Assignment = serde_json::from_value(json!({"task":task,"mode":"m",
                "primary":machine,"start":start,"end":start+1,"ready":start+1,"activities":[]}))
            .unwrap();
            s.assignments.push(a);
        }
        assert!(!PermutationPolicy.validate(&p, &s).is_empty());
    }
}
