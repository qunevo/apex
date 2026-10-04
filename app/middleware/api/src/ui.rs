//! Static MCP App resource. Data arrives through the authenticated host bridge.
use serde_json::{Value, json};

pub const URI: &str = "ui://apex/plan.html";
pub const MIME: &str = "text/html;profile=mcp-app";
pub const INSIGHTS_URI: &str = "ui://apex/insights.html";

fn branded(html: String) -> String {
    let brand = include_str!("../../../ui/mcp-app/brand/brand.html").replace(
        "/* APEX_LOGO */",
        include_str!("../../../ui/mcp-app/brand/apex-logo.svg"),
    );
    html.replace("/* APEX_BRAND */", &brand).replace(
        "/* APEX_BRAND_STYLE */",
        include_str!("../../../ui/mcp-app/brand/brand.css"),
    )
}

pub fn insights_descriptor() -> Value {
    json!({"uri": INSIGHTS_URI, "name": "apex-insights", "description": "Interactive scenario statistics and customization dashboards", "mimeType": MIME})
}

pub fn read_insights() -> Value {
    let html = branded(include_str!("../../../ui/mcp-app/insights.html").into())
        .replace(
            "/* APEX_SCHEDULE */",
            include_str!("../../../ui/mcp-app/schedule.js"),
        )
        .replace(
            "/* APEX_CUSTOM_STYLE */",
            include_str!("../../../customization/demo/ui/style.css"),
        )
        .replace(
            "/* APEX_INSIGHTS_STYLE */",
            include_str!("../../../ui/mcp-app/insights.css"),
        )
        .replace(
            "/* APEX_APP_BRIDGE */",
            include_str!("../../../ui/mcp-app/bridge.js"),
        )
        .replace(
            "/* APEX_INSIGHTS_WIDGETS */",
            include_str!("../../../ui/mcp-app/widgets.js"),
        )
        .replace(
            "/* APEX_CUSTOM_VIEWS */",
            include_str!("../../../customization/demo/ui/renderers.js"),
        )
        .replace(
            "/* APEX_INSIGHTS_VIEW */",
            include_str!("../../../ui/mcp-app/insights.js"),
        );
    json!({"contents": [{"uri": INSIGHTS_URI, "mimeType": MIME, "text": html,
        "_meta": {"ui": {"prefersBorder": true, "csp": {"connectDomains": [], "resourceDomains": []}}}}]})
}

pub fn descriptor() -> Value {
    json!({"uri": URI, "name": "apex-plan", "description": "APEX plan, resources and result provenance", "mimeType": MIME})
}

pub fn read() -> Value {
    let html = branded(include_str!("../../../ui/mcp-app/index.html").into())
        .replace(
            "/* APEX_APP_STYLE */",
            include_str!("../../../ui/mcp-app/style.css"),
        )
        .replace(
            "/* APEX_APP_BRIDGE */",
            include_str!("../../../ui/mcp-app/bridge.js"),
        )
        .replace(
            "/* APEX_APP_VIEW */",
            include_str!("../../../ui/mcp-app/view.js"),
        );
    json!({"contents": [{"uri": URI, "mimeType": MIME, "text": html,
        "_meta": {"ui": {"prefersBorder": true, "csp": {"connectDomains": [], "resourceDomains": []}}}}]})
}
