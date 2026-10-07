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

## Frozen business time

The shipped case starts on 5 October 2026 at 06:00, with a fixed snapshot at
**5 October 2026, 10:00 Europe/Berlin** and a source target period ending on
16 October at 22:00 (weeks 41-42). For this demo, "today", "now" and "this week"
refer to that snapshot and its calendar week, never the real system/chat date,
import time or scenario creation timestamp. Do not advance the MES clock or
shift orders, receipts, absences, downtime or workbook dates as real time passes.

Before answering time-sensitive questions, read `factory.as_of` from MES or
the saved revision's frozen-snapshot assumption and `DEMO_SNAPSHOT` source notice.
The import report also records `time_basis`, including the timezone, epoch and
source/effective horizon. State the reference time in the answer. If evidence
contains a different snapshot or lacks one, flag the discrepancy; do not silently
replace it with the current date or relabel an old scenario as the shipped case.

Distinguish unfinished work **already overdue at the snapshot** (due before that
instant) from **predicted plan lateness** (validated completion minus due date,
clamped at zero). Historical lateness uses actual completion. Excel proposals
alone do not establish predicted completion in a validated APEX plan. An explicit
what-if horizon extension does not move the snapshot or any source dates.

## Source intake

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
For the complete shipped factory, the adapter guide documents an explicitly
extended October 30 horizon and the free `release` strategy in `runs.start`.
Use that tested starting point only when extending the horizon is intended;
do not silently alter the source period, use Excel proposals as commitments or
claim optimality. Report lateness and the actual validated completion dates.
