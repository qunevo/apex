//! Bounded, read-only presentation contracts. Providers inspect an immutable
//! revision and an optional result from that exact revision; they never plan.
use crate::{
    Actor, Control, Diagnostic, Error, PlanResult, Result, ResultId, Revision, Scenario,
    ScenarioId, packages::Package,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

pub const MAX_DOCUMENT_BYTES: usize = 64 * 1024;
pub const MAX_POINTS: usize = 24;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRef {
    pub label: String,
    pub sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSummary {
    pub sources: Vec<SourceRef>,
    pub counts: BTreeMap<String, u64>,
    /// A bounded sample; notice_count includes notices omitted from this list.
    pub notices: Vec<Diagnostic>,
    pub notice_count: u64,
}

fn bounded(text: &str, max: usize) -> bool {
    text.chars().count() <= max
}
pub fn label(text: &str) -> String {
    text.chars().take(160).collect()
}

impl SourceSummary {
    pub fn validate(&self) -> Result<()> {
        if self.sources.len() > 8
            || self.counts.len() > 32
            || self.notices.len() > 20
            || self.notice_count < self.notices.len() as u64
            || self.sources.iter().any(|s| {
                s.label.is_empty()
                    || !bounded(&s.label, 160)
                    || s.sha256.len() != 64
                    || !s.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                    || s.revision.as_ref().is_some_and(|r| !bounded(r, 160))
            })
            || self.counts.keys().any(|k| k.is_empty() || !bounded(k, 80))
            || self.notices.iter().any(|n| {
                !bounded(&n.code, 80)
                    || !bounded(&n.message, 500)
                    || n.entity.as_ref().is_some_and(|e| !bounded(e, 160))
            })
        {
            return Err(Error::invalid(
                "Source summary exceeds the bounded evidence contract",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Descriptor {
    pub id: String,
    pub title: String,
    pub package: Option<Package>,
    pub engine: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Metric {
    pub label: String,
    pub value: Option<f64>,
    pub unit: String,
    pub detail: String,
}
impl Metric {
    pub fn new(name: &str, value: f64, unit: &str, detail: &str) -> Self {
        Self {
            label: label(name),
            value: Some(value),
            unit: label(unit),
            detail: label(detail),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Point {
    pub label: String,
    pub value: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Visual {
    Bars {
        unit: String,
        points: Vec<Point>,
    },
    Donut {
        unit: String,
        points: Vec<Point>,
    },
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// Only explicitly bundled renderers run. Unknown renderers have a text fallback.
    Custom {
        renderer: String,
        data: Value,
    },
}

#[derive(Clone, Debug, Serialize)]
pub struct Panel {
    pub id: String,
    pub title: String,
    pub description: String,
    pub section: String,
    #[serde(flatten)]
    pub visual: Visual,
}

#[derive(Clone, Debug, Serialize)]
pub struct Dashboard {
    pub title: String,
    pub subtitle: String,
    pub metrics: Vec<Metric>,
    pub panels: Vec<Panel>,
    pub notes: Vec<String>,
}

/// Group small categories without losing their totals. Never send raw task arrays.
pub fn grouped(values: BTreeMap<String, f64>) -> Vec<Point> {
    let mut points: Vec<_> = values
        .into_iter()
        .map(|(key, value)| Point {
            label: label(&key),
            value,
        })
        .collect();
    points.sort_by(|a, b| b.value.total_cmp(&a.value).then(a.label.cmp(&b.label)));
    if points.len() > MAX_POINTS {
        let rest = points.split_off(MAX_POINTS - 1);
        points.push(Point {
            label: format!("Other ({} groups)", rest.len()),
            value: rest.iter().map(|p| p.value).sum(),
        });
    }
    points
}

pub struct Context<'a> {
    pub scenario: &'a Scenario,
    pub revision: &'a Revision,
    pub result: Option<&'a PlanResult>,
}

pub trait Provider: Send + Sync {
    fn descriptor(&self) -> Descriptor;
    fn build(&self, context: &Context<'_>) -> Result<Dashboard>;
}

#[derive(Clone)]
pub struct Registry {
    providers: Vec<Arc<dyn Provider>>,
}
impl Default for Registry {
    fn default() -> Self {
        Self {
            providers: vec![Arc::new(Overview)],
        }
    }
}
impl Registry {
    pub fn register(mut self, provider: Arc<dyn Provider>) -> Result<Self> {
        let descriptor = provider.descriptor();
        if descriptor.id.is_empty()
            || !bounded(&descriptor.id, 80)
            || !bounded(&descriptor.title, 160)
            || self
                .providers
                .iter()
                .any(|p| p.descriptor().id == descriptor.id)
        {
            return Err(Error::invalid("Invalid or duplicate view registration"));
        }
        self.providers.push(provider);
        Ok(self)
    }
    fn available(&self, context: &Context<'_>) -> Vec<&Arc<dyn Provider>> {
        self.providers
            .iter()
            .filter(|p| {
                let d = p.descriptor();
                d.engine
                    .as_ref()
                    .is_none_or(|e| e == &context.scenario.engine)
                    && d.package.as_ref().is_none_or(|p| {
                        Some(p) == context.revision.content.customization_package.as_ref()
                    })
            })
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub scenario_id: ScenarioId,
    #[serde(default)]
    pub revision: Option<u64>,
    #[serde(default)]
    pub result_id: Option<ResultId>,
    #[serde(default)]
    pub view_id: Option<String>,
}

#[derive(Serialize)]
pub struct Document {
    pub kind: &'static str,
    pub scenario_id: ScenarioId,
    pub scenario_name: String,
    pub revision: u64,
    pub content_hash: String,
    pub result_id: Option<ResultId>,
    pub result_status: Option<String>,
    pub validation_valid: Option<bool>,
    pub package: Option<Package>,
    pub view_id: String,
    pub available_views: Vec<Descriptor>,
    pub dashboard: Dashboard,
    pub source_summary: Option<SourceSummary>,
}

impl Control {
    pub async fn get_view(&self, actor: &Actor, request: Request) -> Result<Document> {
        let scenario = self.get_scenario(actor, request.scenario_id).await?;
        let result = match request.result_id {
            Some(id) => Some(self.get_result(actor, id).await?),
            None => None,
        };
        if result.as_ref().is_some_and(|r| {
            r.scenario != scenario.id || request.revision.is_some_and(|number| number != r.revision)
        }) {
            return Err(Error::invalid(
                "View result must belong to the requested scenario and revision",
            ));
        }
        let number = request
            .revision
            .or_else(|| result.as_ref().map(|r| r.revision))
            .unwrap_or(scenario.current_revision);
        let revision = self.get_revision(actor, scenario.id, number).await?;
        if result
            .as_ref()
            .is_some_and(|r| r.provenance.content_hash != revision.content_hash)
        {
            return Err(Error::invalid(
                "View result content hash does not match its revision",
            ));
        }
        let context = Context {
            scenario: &scenario,
            revision: &revision,
            result: result.as_ref(),
        };
        let available = self.views().available(&context);
        let provider = match request.view_id {
            Some(id) => available.iter().find(|p| p.descriptor().id == id).copied(),
            None => available
                .iter()
                .find(|p| p.descriptor().id == "overview")
                .copied(),
        }
        .ok_or_else(|| {
            Error::invalid("View is not registered for this engine and package version")
        })?;
        let dashboard = provider.build(&context)?;
        validate_dashboard(&dashboard, revision.content.customization_package.as_ref())?;
        let document = Document {
            kind: "apex_view",
            scenario_id: scenario.id,
            scenario_name: label(&scenario.name),
            revision: number,
            content_hash: revision.content_hash.clone(),
            result_id: result.as_ref().map(|r| r.id),
            result_status: result.as_ref().map(|r| r.status.as_str().into()),
            validation_valid: result.as_ref().map(|r| r.validation.valid),
            package: revision.content.customization_package.clone(),
            view_id: provider.descriptor().id,
            available_views: available.iter().map(|p| p.descriptor()).collect(),
            dashboard,
            source_summary: revision.content.source_summary.clone(),
        };
        if serde_json::to_vec(&document)
            .map_err(|e| Error::invalid(e.to_string()))?
            .len()
            > MAX_DOCUMENT_BYTES
        {
            return Err(Error::invalid("View exceeds the 64 KiB response limit"));
        }
        Ok(document)
    }
}

fn validate_dashboard(d: &Dashboard, package: Option<&Package>) -> Result<()> {
    let invalid = || Error::invalid("View provider returned an invalid or unbounded dashboard");
    if !bounded(&d.title, 160)
        || !bounded(&d.subtitle, 500)
        || d.metrics.len() > 12
        || d.panels.len() > 12
        || d.notes.len() > 12
        || d.notes.iter().any(|n| !bounded(n, 500))
        || d.metrics.iter().any(|m| {
            m.value.is_some_and(|value| !value.is_finite())
                || !bounded(&m.label, 160)
                || !bounded(&m.unit, 160)
                || !bounded(&m.detail, 500)
        })
    {
        return Err(invalid());
    }
    let mut ids = BTreeSet::new();
    for p in &d.panels {
        if p.id.is_empty()
            || !bounded(&p.id, 80)
            || !ids.insert(&p.id)
            || !bounded(&p.title, 160)
            || !bounded(&p.description, 500)
            || !matches!(
                p.section.as_str(),
                "input" | "result" | "orders" | "resources"
            )
        {
            return Err(invalid());
        }
        let valid = match &p.visual {
            Visual::Bars { unit, points } | Visual::Donut { unit, points } => {
                bounded(unit, 80)
                    && points.len() <= MAX_POINTS
                    && points
                        .iter()
                        .all(|p| bounded(&p.label, 160) && p.value.is_finite() && p.value >= 0.0)
            }
            Visual::Table { columns, rows } => {
                !columns.is_empty()
                    && columns.len() <= 8
                    && rows.len() <= 24
                    && columns.iter().all(|c| bounded(c, 160))
                    && rows
                        .iter()
                        .all(|r| r.len() == columns.len() && r.iter().all(|v| bounded(v, 160)))
            }
            Visual::Custom { renderer, data } => {
                package.is_some_and(|p| renderer.starts_with(&format!("{}.", p.id)))
                    && bounded(renderer, 80)
                    && serde_json::to_vec(data).is_ok_and(|v| v.len() <= 8192)
            }
        };
        if !valid {
            return Err(invalid());
        }
    }
    Ok(())
}

pub struct Overview;
impl Provider for Overview {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "overview".into(),
            title: "Overview".into(),
            package: None,
            engine: None,
        }
    }
    fn build(&self, c: &Context<'_>) -> Result<Dashboard> {
        #[cfg(feature = "apex")]
        if c.scenario.engine == "apex" {
            return crate::overview::build(c);
        }
        let mut d = Dashboard {
            title: "Planning overview".into(),
            subtitle: "An immutable input snapshot and its saved planning evidence.".into(),
            metrics: Vec::new(),
            panels: Vec::new(),
            notes: Vec::new(),
        };
        if let Some(result) = c.result {
            d.metrics = result
                .metrics
                .iter()
                .take(12)
                .map(|(k, v)| {
                    Metric::new(
                        &k.replace('_', " "),
                        *v,
                        "",
                        "Saved engine metric; native units",
                    )
                })
                .collect();
            let mut spans = BTreeMap::new();
            for op in &result.view.operations {
                let seconds = op.end.saturating_sub(op.start).max(0);
                *spans.entry(op.resource.clone()).or_default() += seconds as f64 / 3600.0;
            }
            d.panels.push(Panel { id: "resource-span".into(), title: "Time by primary resource".into(), section: "result".into(),
                description: "Sum of main-operation start-to-end spans, in hours. Includes pauses; excludes secondary resources and pre/post work. This is not utilization.".into(),
                visual: Visual::Bars { unit: "h".into(), points: grouped(spans) } });
            if result.metrics.len() > 12 {
                d.notes.push("Showing the first 12 saved metrics; results.get returns the complete metric set.".into());
            }
        } else {
            d.notes.push(
                "Input snapshot only. Select a saved result to see planning outcomes.".into(),
            );
        }
        if let Some(summary) = &c.revision.content.source_summary {
            d.panels.push(Panel {
                id: "source-counts".into(),
                title: "Imported source counts".into(),
                section: "input".into(),
                description: "Adapter-reported counts captured with this revision.".into(),
                visual: Visual::Bars {
                    unit: "records".into(),
                    points: grouped(
                        summary
                            .counts
                            .iter()
                            .map(|(k, v)| (k.replace('_', " "), *v as f64))
                            .collect(),
                    ),
                },
            });
        }
        Ok(d)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn providers_cannot_emit_unbounded_or_invalid_panels() {
        let mut dashboard = Dashboard {
            title: "Test".into(),
            subtitle: String::new(),
            metrics: Vec::new(),
            notes: Vec::new(),
            panels: vec![Panel {
                id: "test".into(),
                title: "Test".into(),
                description: String::new(),
                section: "input".into(),
                visual: Visual::Bars {
                    unit: "count".into(),
                    points: vec![Point {
                        label: "A".into(),
                        value: f64::NAN,
                    }],
                },
            }],
        };
        assert!(validate_dashboard(&dashboard, None).is_err());
        dashboard.panels[0].visual = Visual::Bars {
            unit: "count".into(),
            points: (0..25)
                .map(|i| Point {
                    label: i.to_string(),
                    value: 1.0,
                })
                .collect(),
        };
        assert!(validate_dashboard(&dashboard, None).is_err());
        dashboard.panels[0].visual = Visual::Custom {
            renderer: "other.component".into(),
            data: serde_json::json!({}),
        };
        let package = Package {
            id: "demo".into(),
            version: "1".into(),
        };
        assert!(validate_dashboard(&dashboard, Some(&package)).is_err());
        dashboard.panels[0].visual = Visual::Table {
            columns: vec!["One".into()],
            rows: vec![vec![]],
        };
        assert!(validate_dashboard(&dashboard, None).is_err());
        dashboard.panels[0].visual = Visual::Bars {
            unit: "count".into(),
            points: vec![],
        };
        assert!(validate_dashboard(&dashboard, None).is_ok());
        dashboard.panels.push(dashboard.panels[0].clone());
        assert!(validate_dashboard(&dashboard, None).is_err());
        assert!(Registry::default().register(Arc::new(Overview)).is_err());
    }
}
