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
On the compatibility catalog, check `capabilities` with `customization: "demo"`,
retain that context on every follow-up call and use the returned viewer link.
Treat source facts separately from planner proposals and fixed commitments.

The factory adapter is pending. Do not claim `demo.create` imports the MES, or
that choosing this package synchronizes Excel, activates native rules or writes
a plan back to production. Report the missing integration explicitly.
