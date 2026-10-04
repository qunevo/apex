//! Standard APEX presentation. Read saved completion maps and engine metrics;
//! never infer product completion from the main-operation Gantt envelope.
use crate::{
    Error, Result,
    views::{Context, Dashboard, Metric, Panel, Point, Visual, grouped, label},
};
use chrono::{DateTime, Duration};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Default, Deserialize)]
#[serde(default)]
struct Input {
    epoch: Option<String>,
    horizon: i64,
    tasks: Vec<Task>,
    jobs: Vec<Entity>,
    orders: Vec<Entity>,
    resources: Vec<Resource>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Task {
    id: String,
    stage: String,
    due: Option<i64>,
    #[serde(default = "one")]
    priority: f64,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Entity {
    id: String,
    due: Option<i64>,
    #[serde(default = "one")]
    priority: f64,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Resource {
    id: String,
    calendar: Vec<Window>,
}
#[derive(Deserialize)]
struct Window {
    start: i64,
    end: i64,
    #[serde(default = "one")]
    capacity: f64,
    #[serde(default = "one")]
    rate: f64,
}
fn one() -> f64 {
    1.0
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Saved {
    order_completions: BTreeMap<String, i64>,
    job_completions: BTreeMap<String, i64>,
    assignments: Vec<Assignment>,
}
#[derive(Deserialize)]
struct Assignment {
    task: String,
    ready: i64,
}

fn metric(name: &str, value: Option<f64>, unit: &str, detail: &str) -> Metric {
    Metric {
        label: name.into(),
        value,
        unit: unit.into(),
        detail: detail.into(),
    }
}
fn table(
    id: &str,
    title: &str,
    description: String,
    section: &str,
    columns: &[&str],
    rows: Vec<Vec<String>>,
) -> Panel {
    Panel {
        id: id.into(),
        title: title.into(),
        description,
        section: section.into(),
        visual: Visual::Table {
            columns: columns.iter().map(|c| (*c).into()).collect(),
            rows,
        },
    }
}
fn time(epoch: Option<&str>, seconds: i64) -> String {
    epoch
        .and_then(|e| DateTime::parse_from_rfc3339(e).ok())
        .and_then(|date| {
            Duration::try_seconds(seconds).and_then(|duration| date.checked_add_signed(duration))
        })
        .map(|date| {
            date.with_timezone(&chrono::Utc)
                .format("%d %b %H:%M")
                .to_string()
        })
        .unwrap_or_else(|| format!("{:.1} h", seconds as f64 / 3600.0))
}
fn ranked(mut points: Vec<Point>) -> Vec<Point> {
    points.sort_by(|a, b| b.value.total_cmp(&a.value).then(a.label.cmp(&b.label)));
    points.truncate(24);
    points
}

pub fn build(c: &Context<'_>) -> Result<Dashboard> {
    // Deserialize the small projection by reference; expanded mode catalogs stay on the server.
    let p = Input::deserialize(&c.revision.content.facts)
        .map_err(|e| Error::invalid(format!("Overview input: {e}")))?;
    let result = c.result.filter(|r| r.validation.valid);
    let saved = result
        .map(|r| Saved::deserialize(&r.schedule))
        .transpose()
        .map_err(|e| Error::invalid(format!("Overview schedule: {e}")))?;
    let (singular, plural, prefix, entities, completions) = if !p.orders.is_empty() {
        (
            "Order",
            "orders",
            "order",
            p.orders,
            saved
                .as_ref()
                .map(|s| s.order_completions.clone())
                .unwrap_or_default(),
        )
    } else if !p.jobs.is_empty() {
        (
            "Lot",
            "lots",
            "job",
            p.jobs,
            saved
                .as_ref()
                .map(|s| s.job_completions.clone())
                .unwrap_or_default(),
        )
    } else {
        (
            "Operation",
            "operations",
            "task",
            p.tasks
                .iter()
                .map(|t| Entity {
                    id: t.id.clone(),
                    due: t.due,
                    priority: t.priority,
                })
                .collect(),
            saved
                .as_ref()
                .map(|s| {
                    s.assignments
                        .iter()
                        .map(|a| (a.task.clone(), a.ready))
                        .collect()
                })
                .unwrap_or_default(),
        )
    };
    let measured = |key: &str| result.and_then(|r| r.metrics.get(key)).copied();
    let due_count = measured(&format!("{prefix}_due_count"));
    let on_time = due_count
        .filter(|count| *count > 0.0)
        .and_then(|_| measured(&format!("{prefix}_on_time_delivery")))
        .map(|v| v * 100.0);
    let mut d = Dashboard {
        title: "Planning overview".into(),
        subtitle: "Delivery commitments, workload and the next decisions.".into(),
        metrics: vec![
            metric(
                &format!("{singular}s in scope"),
                Some(entities.len() as f64),
                "",
                "Records in this input revision",
            ),
            metric(
                "Operations",
                Some(p.tasks.len() as f64),
                "",
                "Input operations; selected routes may use a subset",
            ),
            metric(
                &format!("On-time {plural}"),
                on_time,
                "%",
                "Planned deliveries with a due date",
            ),
            metric(
                &format!("Late {plural}"),
                measured(&format!("{prefix}_late_count")),
                "",
                "Predicted from a validated saved plan",
            ),
            metric(
                "Plan span",
                measured("makespan").map(|v| v / 3600.0),
                "h",
                "Time from planning origin to completion",
            ),
            metric(
                "Resources",
                Some(p.resources.len() as f64),
                "",
                "All equipment, people and capacity pools in scope",
            ),
        ],
        panels: Vec::new(),
        notes: Vec::new(),
    };
    let mut statuses = BTreeMap::new();
    let mut critical = Vec::new();
    for entity in &entities {
        let completion = completions.get(&entity.id).copied();
        let slack = completion
            .zip(entity.due)
            .map(|(end, due)| due.saturating_sub(end));
        let (status, rank) = match (result, completion, entity.due, slack) {
            (None, _, _, _) => ("Not planned", 3),
            (_, None, _, _) => ("Not scheduled", 0),
            (_, _, None, _) => ("No due date", 4),
            (_, _, _, Some(value)) if value < 0 => ("Late", 1),
            (_, _, _, Some(value)) if value <= 3600 => ("Tight buffer", 2),
            _ => ("On time", 5),
        };
        let category = match status {
            "On time" => "On time · buffer > 1 h",
            "Tight buffer" => "On time · buffer ≤ 1 h",
            _ => status,
        };
        *statuses.entry(category.to_string()).or_default() += 1.0;
        if result.is_none() || rank <= 2 {
            critical.push((entity, completion, slack, status, rank));
        }
    }
    critical.sort_by(|a, b| {
        a.4.cmp(&b.4)
            .then_with(|| {
                if result.is_some() {
                    a.2.unwrap_or(i64::MIN).cmp(&b.2.unwrap_or(i64::MIN))
                } else {
                    a.0.due
                        .unwrap_or(i64::MAX)
                        .cmp(&b.0.due.unwrap_or(i64::MAX))
                }
            })
            .then_with(|| b.0.priority.total_cmp(&a.0.priority))
            .then(a.0.id.cmp(&b.0.id))
    });
    let count = critical.len();
    let rows = critical
        .into_iter()
        .take(12)
        .map(|(e, end, slack, status, _)| {
            vec![
                label(&e.id),
                e.due
                    .map(|v| time(p.epoch.as_deref(), v))
                    .unwrap_or_else(|| "Not set".into()),
                end.map(|v| time(p.epoch.as_deref(), v))
                    .unwrap_or_else(|| "—".into()),
                slack
                    .map(|v| {
                        if v < 0 {
                            format!("{:.1} h late", -(v as f64) / 3600.0)
                        } else {
                            format!("{:.1} h buffer", v as f64 / 3600.0)
                        }
                    })
                    .unwrap_or_else(|| "—".into()),
                format!("{}", e.priority),
                status.into(),
            ]
        })
        .collect();
    let title = if result.is_some() {
        format!("Critical {plural}")
    } else {
        format!("Upcoming {plural}")
    };
    let description = if result.is_some() {
        format!(
            "{count} need attention. Showing up to 12, ordered by missing completion, lateness and buffer. A tight buffer means at most 1 hour before the due date."
        )
    } else {
        "Ordered by due date, then priority. Delivery risk is unknown until a validated plan exists. Showing up to 12.".into()
    };
    let (due_label, ready_label) = if p
        .epoch
        .as_deref()
        .is_some_and(|e| DateTime::parse_from_rfc3339(e).is_ok())
    {
        ("Due · UTC", "Product ready · UTC")
    } else {
        ("Due · from origin", "Product ready · from origin")
    };
    d.panels.push(table(
        "critical-orders",
        &title,
        description,
        "orders",
        &[
            singular,
            due_label,
            ready_label,
            "Timing",
            "Priority",
            "Status",
        ],
        rows,
    ));
    d.panels.push(Panel { id: "delivery-outlook".into(), title: "Delivery outlook".into(), section: "orders".into(),
        description: format!("All {plural} in scope, using product-ready completion including required post-processing. Tight-buffer deliveries still count as on time. Undated and unscheduled records stay visible."),
        visual: Visual::Donut { unit: plural.into(), points: grouped(statuses) } });
    let resources = p.resources.len();
    let (title, unit, description, points) = if result.is_some() {
        let points: Vec<_> = p
            .resources
            .iter()
            .filter_map(|r| {
                measured(&format!("resource:{}:utilization_7d", r.id)).map(|value| Point {
                    label: label(&r.id),
                    value: value * 100.0,
                })
            })
            .collect();
        let count = points.len();
        (
            "Resource utilization",
            "%",
            format!(
                "Main-operation capacity used in the first 7 days, capped by the planning horizon. Includes recorded actual work; excludes pre/post-processing. Showing the highest {} of {count} reported resources.",
                count.min(24)
            ),
            ranked(points),
        )
    } else {
        let points = p
            .resources
            .iter()
            .map(|r| Point {
                label: label(&r.id),
                value: r
                    .calendar
                    .iter()
                    .filter(|w| w.rate > 0.0)
                    .map(|w| {
                        w.end.min(p.horizon).saturating_sub(w.start.max(0)).max(0) as f64
                            * w.capacity
                            / 3600.0
                    })
                    .sum(),
            })
            .collect();
        (
            "Available capacity",
            "capacity h",
            format!(
                "Calendar capacity across the input horizon, excluding windows with zero rate. No scheduled load yet. Showing up to 24 of {resources} resources."
            ),
            ranked(points),
        )
    };
    d.panels.push(Panel {
        id: "resource-load".into(),
        title: title.into(),
        description,
        section: "resources".into(),
        visual: Visual::Bars {
            unit: unit.into(),
            points,
        },
    });
    let mut stages = BTreeMap::new();
    for task in &p.tasks {
        *stages
            .entry(if task.stage.is_empty() {
                "Unspecified".into()
            } else {
                task.stage.clone()
            })
            .or_default() += 1.0;
    }
    d.panels.push(Panel { id: "work-mix".into(), title: "Work by stage".into(), section: "input".into(), description: "Operation counts in the input. Alternative-route candidates may be present before route selection.".into(),
        visual: Visual::Bars { unit: "operations".into(), points: grouped(stages) } });
    if let Some(r) = result {
        let definitions = [
            ("makespan".to_string(), "Plan span", "h", 3600.0),
            (
                format!("{prefix}_due_count"),
                "Records with due dates",
                "count",
                1.0,
            ),
            (
                format!("{prefix}_late_count"),
                "Late deliveries",
                "count",
                1.0,
            ),
            (
                format!("{prefix}_tardiness"),
                "Total delivery lateness",
                "h",
                3600.0,
            ),
            (
                format!("{prefix}_max_tardiness"),
                "Maximum delivery lateness",
                "h",
                3600.0,
            ),
            (
                "conditional_time".to_string(),
                "Conditional activity time",
                "h",
                3600.0,
            ),
            (
                "total_mode_cost".to_string(),
                "Mode cost",
                "configured units",
                1.0,
            ),
        ];
        let rows = definitions
            .iter()
            .filter_map(|(key, name, unit, divisor)| {
                r.metrics.get(key).map(|value| {
                    vec![
                        (*name).into(),
                        format!("{:.2}", value / divisor),
                        (*unit).into(),
                    ]
                })
            })
            .collect();
        d.panels.push(table("plan-metrics", "Plan metrics", "Selected engine KPIs with explicit units. Delivery metrics use the same record level as the overview cards.".into(), "result", &["Metric", "Value", "Unit"], rows));
    }
    let undated = entities.iter().filter(|e| e.due.is_none()).count();
    if undated > 0 {
        d.notes.push(format!("{undated} {plural} have no due date. They are excluded from on-time delivery percentages."));
    }
    if c.result.is_some_and(|r| !r.validation.valid) {
        d.notes.push("The selected result failed validation. Outcome KPIs and completion predictions are withheld; input remains available.".into());
    } else if result.is_none() {
        d.notes.push("No validated plan selected. A dash means the outcome is unknown, not zero. Use the chat to create or select a plan.".into());
    }
    if let Some(summary) = &c.revision.content.source_summary
        && summary.notice_count > 0
    {
        d.notes.push(format!("The import reports {} notices. Inspect the source details before changing planning facts.", summary.notice_count));
    }
    Ok(d)
}
