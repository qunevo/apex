---
name: apex-extension
description: Implement and test customer scheduling rules, objectives, Q proxies and customization hooks. Prefer package extensions; require scoped approval for changes to the base application. Use when planner requirements exceed existing typed rules, not for routine scenario edits or source-system writeback.
---

# APEX extension

Paths below are relative to the APEX application installation (`app/` in the development repository). Configure the agent to load this skill from that installation.

Make the requested domain behavior a tested repository change. Read `AGENTS.md`, the [current architecture and change map](../../docs/architecture/README.md), `docs/implementation.md`, `docs/search-and-parity.md` and the applicable customization knowledge file. Keep customer-specific code under customization and only synthetic examples in distribution. A skill is workflow guidance, not a runtime plugin.

## Customization boundary and approval

For customer customization, identify the target `customization/<id>/` package and inspect existing typed rules, adapters and hooks before editing. Prefer translating customer input in the package adapter over changing the shared input model. Classify the proposed files and behavior:

| Change | Authorization and upgrade impact |
| --- | --- |
| Package code, skills, tests or views using existing contracts | Implement within the user's requested scope without an additional approval step. Retest against a new APEX release; package isolation is not an upgrade guarantee. |
| Registration or build wiring outside the package | Identify the exact changes, such as registration in `core/extensions.rs` and a module entry in `core/lib.rs`, and obtain scoped approval before editing them. This is a limited integration change that may need reapplication on upgrade. |
| Shared input types, hook interfaces, scheduling behavior, API, storage or other base application files | Explain that this creates a customer-maintained product modification. Obtain scoped approval before editing; future upgrades require reviewing and possibly adapting that patch. |

Before changing shipped files outside the package, present a concrete plan in chat: affected files and purpose, why those outside-package edits are needed, a supported alternative if one exists, upgrade implications and validation checks. Explain that this skill's customization boundary requires confirmation, ask for explicit approval of that scope and wait before making those edits. A general request to implement a business rule does not by itself approve an undisclosed base application change. Read-only investigation and independently useful work within the authorized package may continue while approval is pending.

An existing explicit approval covering those changes and their upgrade impact counts; do not ask again for the same scope. Ask again only when additional files or behavior materially expand the approved impact. Apply this boundary to shipped source, schemas, shared tests, manifests, deployment defaults and shared skills. Normal builds, ignored local output and authorized scenario operations do not themselves cross it. Code inside a package that rewrites base application files still requires approval for those target changes.

Keep the approved change isolated and reviewable. Record the base APEX version/source revision, changed files outside the package, their purpose, approval scope and upgrade verification steps in the package's existing knowledge record, with links to commits/diffs and tests. This customer workflow does not add a second approval gate to explicitly authorized APEX contributor development or maintenance.

## Define and implement one behavioral contract

- Record examples, counterexamples, units, scope, edge cases, hard/soft classification, objective direction and rule provenance. Resolve material ambiguities with the planner rather than inventing business policy.
- Reuse supported input rules or existing customization hooks where sufficient. If the approved scope requires a base application change, update affected typed schema, readiness checks, lowering/hooks, construction decisions and independent validation together. Keep the core generic. Rust customization hooks are linked at build time; arbitrary source text is not executed at runtime.
- For each objective implement the actual schedule metric first, including direction, scale and priority. Connect a supported standard Q or implement an explicit custom queue definition/value hook. Do not claim a metric formula can always be mechanically converted into a useful proxy.
- Evaluate the proposed Q on complete schedules with varied synthetic instances and held-out cases. Report correlation direction, rank/regret measures, negative cases, search budget and comparison to the previous policy. Fix poor mappings or label them experimental. Exact fitness and independent hard-rule validation remain authoritative.
- Exercise XH and direct-GA mutation/crossover, parallel evaluation, Pareto/priority ranking and XT where the changed rule affects their candidate space. Preserve deterministic results under fixed evaluation budgets where promised.
- Add semantic, negative and corrupted-output tests. Include relevant interactions with calendars, material supply, running work, fixed decisions, modes/routes and conditional operations. Run the checks required by `AGENTS.md`; rebuild before tool/browser verification.
- Update affected schemas, bounded tool diagnostics, capability descriptions and domain documentation within the approved scope. Add only the UI explanation needed to inspect the new decision; keep routine scenario editing in the chat.

For mandatory selection rules, use the shared `dispatch.rs` / `policy.rs` path and the [language contract](../../docs/architecture/declarative-scheduling.md). Never add a Fast-Planner-only bypass. Native filters receive the complete eligible pool; exact placement facts require a tested prefix-decoration contract. Cover forced prefixes, singleton pools, candidates beyond the Q window, XH/XT/XE and corrupted replay witnesses. Compare unchanged and equivalent synthetic inputs under fixed evaluation budgets; measure additional policy work separately from other overhead.

For genetic operators and sequence-generated conditional alternatives, follow [the direct-search contract](../../docs/direct-schedule-evolution.md). Native operators propose chromosomes; shared evaluation remains authoritative. Version operator identities, preserve deterministic seeded execution and test malformed proposals, fixed commitments, replay, operator attribution and equal-budget quality. Declare stable conditional activity/mode IDs before searching alternatives emitted by a sequence hook.

## Keep the knowledge record honest

Link each implemented domain rule to its schema/hook, tests and provenance. Separate proposed, implemented and deprecated rules. Unknown facts and planner-approved estimates remain explicit. Updating Markdown alone does not enforce a rule. Do not publish customer information, install generated code into a live system or perform external writeback as a side effect of implementation.

Deliver the behavior change, validation evidence, limits, a reproducible synthetic scenario and any approved changes outside the package with their upgrade implications. Do not report full legacy parity when a relevant legacy case remains untested.
