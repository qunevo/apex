//! Optional MES/Excel evidence view. General planning belongs to the standard overview.
use crate::{
    Result,
    packages::Package,
    views::{Context, Dashboard, Descriptor, Metric, Panel, Provider, Visual, label},
};
use serde_json::json;

pub struct SourceDetails;
impl Provider for SourceDetails {
    fn descriptor(&self) -> Descriptor {
        Descriptor {
            id: "demo.sources".into(),
            title: "Source details".into(),
            package: Some(Package {
                id: "demo".into(),
                version: "1".into(),
            }),
            engine: Some("apex".into()),
        }
    }
    fn build(&self, c: &Context<'_>) -> Result<Dashboard> {
        let mut d = Dashboard {
            title: "Source details".into(),
            subtitle: "How MES facts and Excel proposals form this input.".into(),
            metrics: Vec::new(),
            panels: Vec::new(),
            notes: Vec::new(),
        };
        let Some(summary) = &c.revision.content.source_summary else {
            d.notes.push("No adapter source summary was saved. MES/Excel coverage is unknown; re-import to include it.".into());
            return Ok(d);
        };
        let counts = &summary.counts;
        let tasks = c.revision.content.facts["tasks"]
            .as_array()
            .map_or(0, Vec::len);
        if let (Some(total), Some(missing)) = (
            counts.get("planned_operations"),
            counts.get("operations_without_excel"),
        ) {
            if *total == tasks as u64 && missing <= total {
                d.metrics.push(Metric::new(
                    "Open input operations",
                    *total as f64,
                    "",
                    "MES total minus closed operations",
                ));
                if *total > 0 {
                    d.metrics.push(Metric::new(
                        "With Excel proposal",
                        (total - missing) as f64 / *total as f64 * 100.0,
                        "%",
                        "Matched rows among open input operations",
                    ));
                }
                d.metrics.push(Metric::new(
                    "Without Excel proposal",
                    *missing as f64,
                    "",
                    "Kept in the input using MES facts",
                ));
                d.panels.push(Panel { id: "source-flow".into(), title: "Source reconciliation".into(), section: "input".into(),
                    description: "MES contains open and closed operations. Excel adds proposals to matching IDs. Open operations without an Excel row remain in the input.".into(),
                    visual: Visual::Custom { renderer: "demo.import-flow".into(), data: json!({
                        "mes": counts.get("mes_operations"), "excel": counts.get("excel_operations"),
                        "remaining": total, "matched": total - missing, "missing": missing,
                        "closed": counts.get("closed_operations"), "sources": summary.sources
                    }) } });
            } else {
                d.notes.push("Adapter counts do not match this input. Excel coverage is unavailable; re-import to refresh the evidence.".into());
            }
        }
        d.metrics.push(Metric::new(
            "Import notices",
            summary.notice_count as f64,
            "",
            "Adapter diagnostics, separate from plan validation",
        ));
        if !summary.notices.is_empty() {
            d.panels.push(Panel {
                id: "import-notices".into(),
                title: "Import attention".into(),
                section: "input".into(),
                description: format!(
                    "{} notices in the adapter report; showing {}.",
                    summary.notice_count,
                    summary.notices.len()
                ),
                visual: Visual::Table {
                    columns: vec!["Code".into(), "Entity".into(), "Message".into()],
                    rows: summary
                        .notices
                        .iter()
                        .map(|n| {
                            vec![
                                label(&n.code),
                                label(n.entity.as_deref().unwrap_or("—")),
                                label(&n.message),
                            ]
                        })
                        .collect(),
                },
            });
        }
        d.notes.push("Adapter-reported source evidence for this saved revision. Refresh does not reread MES or Excel.".into());
        Ok(d)
    }
}
