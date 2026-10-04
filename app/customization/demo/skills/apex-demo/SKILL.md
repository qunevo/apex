---
name: apex-demo
description: Explain the synthetic valve-factory integration boundary and use the demo package context with general APEX planning tools.
---

# Demo planning context

Read [model knowledge](../../model/KNOWLEDGE.md), then use the general
`apex-data-intake` and `apex-planning` skills from the application installation.
Discover the connected tools first. On the central middleware, select
`customization: "demo"` on `scenarios.create`; later calls use the resulting
scenario, revision and result IDs, and `results.get` supplies the MCP App.
Use `views.get` with `scenario_id` after intake for the standard Planning overview.
Add `result_id` after a successful run for delivery KPIs, critical orders and
resource utilization. Select `view_id: "demo.sources"` for Source details:
MES/Excel coverage and import notices. Keep the revision explicit when discussing
a saved snapshot. Missing evidence is unknown coverage; refresh does not import.
Charts do not establish feasibility; only validated saved plans supply outcome KPIs.
On the compatibility catalog, check `capabilities` with `customization: "demo"`,
retain that context on every follow-up call and use the returned viewer link.
Treat source facts separately from planner proposals and fixed commitments.

Use the [source adapter](../../adapter/README.md) to read the live MES API and
saved Excel working file into a canonical `problem.json` and import report.
Review diagnostics and assumptions, then submit the generated `scenario.json`
through `scenarios.create`, or revise an existing scenario with its current
`expected_revision`. The command itself only exports files. New imports use new
output directories. Read the actual saved workbook, never the MES seed previews.

Do not claim `demo.create` imports the MES, or that choosing this package
synchronizes Excel, activates native rules or writes a plan back to production.
Keep uncommitted Excel proposals separate from fixed model decisions. A successful
input check does not establish feasibility; run and independently validate before
presenting a usable plan. The demo baseline can extend beyond the source horizon.
