//! Static MCP App resource. Data arrives through the authenticated host bridge.
use serde_json::{Value, json};

pub const URI: &str = "ui://apex/plan.html";
pub const MIME: &str = "text/html;profile=mcp-app";

pub fn descriptor() -> Value {
    json!({"uri": URI, "name": "apex-plan", "description": "APEX plan, resources and result provenance", "mimeType": MIME})
}

pub fn read() -> Value {
    let html = include_str!("../../../ui/mcp-app/index.html")
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
