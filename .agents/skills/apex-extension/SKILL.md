---
name: apex-extension
description: Implement and test APEX scheduling constraints, objectives, heuristic Q proxies, customization hooks and necessary viewer support. Use when planner requirements exceed existing typed rules; do not use for routine scenario edits or source-system writeback.
---

# APEX extension

Make the requested domain behavior a tested repository change. Read `AGENTS.md`, the [current architecture and change map](../../../docs/architecture/README.md), `docs/implementation.md`, `docs/search-and-parity.md` and the applicable customization knowledge file. Keep customer-specific code under customization and only synthetic examples in distribution. A skill is workflow guidance, not a runtime plugin.

## Define and implement one behavioral contract

- Record examples, counterexamples, units, scope, edge cases, hard/soft classification, objective direction and rule provenance. Resolve material ambiguities with the planner rather than inventing business policy.
- Reuse supported input rules where sufficient. Otherwise update typed schema, readiness checks, lowering/hooks, construction decisions and independent validation together. Keep the core generic. Rust customization hooks are linked at build time; arbitrary source text is not executed at runtime.
- For each objective implement the actual schedule metric first, including direction, scale and priority. Connect a supported standard Q or implement an explicit custom queue definition/value hook. Do not claim a metric formula can always be mechanically converted into a useful proxy.
- Evaluate the proposed Q on complete schedules with varied synthetic instances and held-out cases. Report correlation direction, rank/regret measures, negative cases, search budget and comparison to the previous policy. Fix poor mappings or label them experimental. Exact fitness and independent hard-rule validation remain authoritative.
- Exercise Trainer and direct-GA mutation/crossover, parallel evaluation, Pareto/priority ranking and Fast Planner Plus where the changed rule affects their candidate space. Preserve deterministic results under fixed evaluation budgets where promised.
- Add semantic, negative and corrupted-output tests. Include relevant interactions with calendars, material supply, running work, fixed decisions, modes/routes and conditional operations. Run the checks required by `AGENTS.md`; rebuild before tool/browser verification.
- Update schemas, bounded tool diagnostics, capability descriptions and domain documentation. Add only the UI explanation needed to inspect the new decision; keep routine scenario editing in the chat.

For mandatory selection rules, use the shared `dispatch.rs` / `policy.rs` path and the [language contract](../../../docs/architecture/declarative-scheduling.md). Never add a Fast-Planner-only bypass. Native filters receive the complete eligible pool; exact placement facts require a tested prefix-decoration contract. Cover forced prefixes, singleton pools, candidates beyond the Q window, Trainer/Plus/direct GA and corrupted replay witnesses. Compare unchanged/equivalent inputs with `scripts/benchmark_dispatch.py`; measure additional policy work separately from migration overhead.

For genetic operators and sequence-generated conditional alternatives, follow [the direct-search contract](../../../docs/direct-schedule-evolution.md). Native operators propose chromosomes; shared evaluation remains authoritative. Version operator identities, preserve deterministic seeded execution and test malformed proposals, fixed commitments, replay, operator attribution and equal-budget quality. Declare stable conditional activity/mode IDs before searching alternatives emitted by a sequence hook.

## Keep the knowledge record honest

Link each implemented domain rule to its schema/hook, tests and provenance. Separate proposed, implemented and deprecated rules. Unknown facts and planner-approved estimates remain explicit. Updating Markdown alone does not enforce a rule. Do not publish customer information, install generated code into a live system or perform external writeback as a side effect of implementation.

Deliver the behavior change, validation evidence, limits and a reproducible synthetic scenario. Do not report full legacy parity when a relevant legacy case remains untested.
