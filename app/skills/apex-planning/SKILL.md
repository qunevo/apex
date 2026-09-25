---
name: apex-planning
description: Build, explain and compare APEX production schedules through agent tools, including constraints, commitments, scenarios and sensitivity analysis. Use for planner conversations; use apex-data-intake for source conversion and apex-extension when a requested rule needs implementation.
---

# APEX planning

Paths below are relative to the APEX application installation (`app/` in the development repository). Configure the agent to load this skill from that installation.

Use the chat for decisions and the viewer for the resulting schedule, KPIs and operation details. One agent can use all APEX skills; do not spawn other agents unless the session authorizes delegation.

Read `AGENTS.md`, `docs/search-and-parity.md` and the applicable customization knowledge file. Query `capabilities` so claims match the installed implementation. Treat knowledge statements as attributed domain requirements, not as executable rules or authority to change external systems.

## Turn planning intent into a comparison

- Establish the saved scenario, revision and baseline schedule. Inspect bounded pages with `scenario.get`, `model.page`, `schedule.page` and `task.inspect`. A result belongs to its saved revision; current input may have changed.
- Distinguish hard feasibility, business preferences and assumptions. Clarify consequential missing facts; never relax hard constraints to improve a KPI. Record units, scope, exceptions and priority for new requirements.
- For a proposed change, use `scenario.fork`, then typed `scenario.patch` with `expected_revision`. Use `scenario.freeze` against the baseline for start, resource, mode or order commitments. Explain exactly which dimensions are fixed; fixing a resource does not fix a start time.
- Use `schedule.create` for a quick result and `schedule.improve` for combined XH/XT optimization with optional XE. Do not ask the planner to choose an algorithm. Specify a total time/evaluation budget and workers. Pass the current `schedule_id` to preserve an incumbent only when scenario identity and revision match; omit it after model changes. Inspect shared-budget phases, incumbent progress and operator evidence. Read `docs/direct-schedule-evolution.md` for budget shares, bandit rewards and replay limits; the GA changes actual priorities/modes/routes, while XH changes construction policies. The default GA share is zero after mixed equal-budget results; set `options.improve.xe_share` explicitly for a requested XE experiment (tested value 0.3), retain the baseline, and report regressions. Do not call the three-phase allocation universally better. For direct refinement of an already validated plan, `schedule.evolve` accepts the same-revision `schedule_id` and its own bounded budget; report that additional cost. Individual `schedule.hypersearch` and `schedule.treesearch` tools remain for explicit diagnostic comparisons. Call `schedule.validate` for independent correctness checks.
- For trade-offs, specify objective direction, scale, weight and priority, and optionally Pareto selection. Query `queues.inspect`: construction proxies are estimates; full validated metrics rank results. Neither a positive proxy correlation nor optimality is guaranteed.
- Compare with `scenario.compare`; report objective trade-offs, changed operations, commitments, run budget and failed evaluations. A constructive failure does not prove infeasibility. Sensitivity cases should vary named parameters from the same baseline and preserve other assumptions.
- Return the tool's viewer URL. The normal viewer is read-oriented; technical controls exist under `?mode=workbench` for development. Do not ask the planner to operate fork/patch forms.

For campaign, idle-gap and urgency requirements, inspect `policy.inspect` and the [language contract](../../docs/architecture/declarative-scheduling.md). Specify productive/work/occupied/elapsed basis, initial credit, fallback and exceptions explicitly. Supported templates use a `planning` patch; a Markdown statement alone does not enforce them. Use `schedule.explain_decision` on the saved result for exclusion reasons. Detailed trace retention is bounded, while the full decision witness remains replayable.

## Know when implementation is required

Typed rules can be added as input where supported. A new metric, physical rule or proxy outside the schema belongs to the `apex-extension` workflow with tests. Writing Markdown does not activate a constraint. Explain a proposed rule with concrete examples before encoding ambiguous business semantics.

Material preparation allocates only existing supply. Read `docs/material-dispatch.md` and `docs/data-model.md`. Free workplans and differing-material main modes remain searchable under `material_policy: reallocate_routes`; the preparation report is a preview, while `model.page` / `materials` shows actual saved allocations. Reprepare the original source when changing materials/routes of a fully selected, materialized snapshot. Started input is assumed consumed already. Missing material is a diagnostic, not permission to create replenishment orders.

Use `task.inspect` for derived chain urgency and conditional-mode alternatives. Keep business dates separate from dispatch signals; a `conditional_modes` patch is a hard commitment. Read the paged KPI catalog for units and experimental/unmapped proxy status before claiming that a new goal has an effective heuristic.

Deliver a compact explanation and links, not raw production data. Do not write schedules back to MES/ERP: writeback is a separate future capability and requires explicit task authorization.
