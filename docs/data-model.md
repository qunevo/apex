# Executable data model: apex.v3.4

This is the current model reference for APEX **0.6.0**. The contract is generated from Rust types in [model.rs](../src/rust/model.rs) and [production.rs](../src/rust/production.rs): [canonical problem schema](../schemas/apex.v3.4.json), [production schema](../schemas/production.v3.4.json) and [planning-options schema](../schemas/options.v3.4.json).

Canonical `apex.v3.1`, `apex.v3.2` and `apex.v3.3` inputs remain accepted by the current Rust runtime without historical JSON schema files. The removed `3.0-draft.1` design is not executable input. Input compatibility does not make old result artifacts version-neutral validation certificates; regenerate results when comparing changed metrics or search behavior. The [migration audit](migration-audit.md) preserves the historical v2 comparison and its limits.

## Relationships and mapping

| Source concept | Canonical representation | Decision freedom / enforcement |
| --- | --- | --- |
| Production order | `Order` references jobs | Order readiness is the latest job readiness; due date is soft, deadline hard |
| Production lot | `Job` with item, quantity, due/deadline/priority | One or more operations reference `Task.job`; job readiness is the latest active task's product readiness |
| Alternative routing | `RouteChoice` with named alternatives | Exactly one alternative per choice; each controls task presence and its additional dependencies |
| Operation | `Task` with quantity, modes, family, source | Optional route membership; otherwise mandatory; mode and resource choices remain decisions |
| Machine/resource allocation | `Mode` with primary resource and phases | Main mode selects the complete allocation and its pre/post work and optional material overrides |
| Staffing/tooling | `Phase.requirements` with amounts | Simultaneous capacity requirements, including pools; explicit retention through interruptions |
| Shift/break/productivity | Processing and retention calendars | Available capacity and rate by interval; no work outside processing windows |
| Preparation/inspection/cleanup | Task- or mode-specific `pre` and `post` activities | Own resource alternatives, calendars and phase requirements; optional conditional DAG |
| Technological sequence | `Dependency` | Predecessor product readiness to successor main start, with min/max elapsed lag |
| Sequence-dependent setup | `Transition`, `sequence_pattern`, native sequence hook | Extra predecessor post-work / successor pre-work and independent penalty |
| Material availability | Inventory, receipts, task or mode consume/produce maps | Temporal balance by normalized item key; selected mode overrides the complete task map |
| Existing/running plan | `Execution`, mode/resource/start/order locks | Immutable actual work and explicit remaining work; separately selectable freeze dimensions |
| Factory-specific rule | Typed rule or versioned `CustomizationRef` | Shared construction, evaluation and independent validation semantics |

Time values are integer seconds relative to the optional display epoch. Intervals are half-open. Work is normalized productive seconds; resource rates affect elapsed time. Source adapters normalize timezones and physical units and preserve provenance in `Task.source`. A machine identifier, family or workplan ID does not acquire special meaning from its spelling.

## Routes and presence

```json
{
  "id": "JOB-1/routing",
  "selected": null,
  "alternatives": [
    { "id": "standard", "tasks": ["cut", "assemble"], "dependencies": [{ "before": "cut", "after": "assemble" }] },
    { "id": "external", "tasks": ["subcontract", "assemble"], "dependencies": [{ "before": "subcontract", "after": "assemble" }] }
  ]
}
```

Tasks may be shared by alternatives of the same choice. A task cannot belong to two different choices. Nonmembers are always active. Edges whose endpoints are inactive are removed; selected alternative edges are added to the active graph. Use explicit alternative edges when a dependency changes with the route. This is guarded presence, not automatic substitution of an absent predecessor with another task.

`selected` pins an alternative in the input. Planning options also accept `route_choices` and `mode_choices`. Locked, running or explicitly mode-selected tasks must remain active. Contradictory choices fail. The active graph is checked for cycles; readiness validation does not enumerate all combinations of routes or prove feasibility of every combination.

Fast Planner initially favors the route with the lowest estimated work. Trainer varies route/mode choices with reproducible seeds alongside dispatch strategies. Selection is heuristic, not exhaustive optimization.

## Quantities, workplans and orders

For each phase, required work is `work + task.quantity * work_per_unit`. Either term can be omitted; both absent is `MISSING_PROCESSING_TIME`. The fixed term occurs once per operation/lot, including setup if modeled there. Use explicit conditional activities when setup has different resources, calendars or sequence semantics.

Canonical `consume`/`produce` values are already total quantities for that task. Setting `Task.quantity` does not multiply these maps again. By contrast, **workplan template** material values in `ProductionInput` are per unit and are scaled during expansion. Template task quantity is a multiplier on lot quantity. `$job` in a material key becomes a lot-specific key, preventing accidental sharing of internal work in progress across lots.

`ProductionInput` contains an empty canonical problem header, reusable `workplans`, and `demands`. Each demand chooses permissible workplans and an optional `max_lot`. Expansion conserves the last lot's remainder, produces stable order/job/task identities, and keeps alternative workplans selectable. `predecessors` names supply orders and expands completion-before-start dependencies across their tasks; it is an all-order relationship, not automatic partial-lot pegging.

Use `production.import` directly, or:

```text
apex expand examples/production-orders.json --out .apex/production-expanded.json
apex plan .apex/production-expanded.json --out .apex/production-schedule.json
```

Job completion aggregates active task **product readiness**, not just main processing end. Order completion aggregates its jobs. `job_weighted_tardiness` and `order_weighted_tardiness` count each respective group once. Explicit objective declarations control the score; when omitted, tardiness defaults to orders, then jobs, then tasks, followed by makespan. A member task inherits its job's priority; missing task due dates inherit the job due date, then the order due date. Job/order deadlines constrain all member tasks.

Expansion does not infer BOMs, units, yield or economic lot sizes from ERP data. Source adapters must supply these facts. The separate `material.prepare` step allocates declared existing supply after expansion; see [material preparation](material-dispatch.md). Running execution is imported as canonical tasks, not copied through templates. This is not a raw v2 JSON importer.

## Conditional activity graphs

`Task.pre/post` applies across modes. `Mode.pre/post` applies only when that main mode is selected. `owner_resource` records and validates the associated main-mode requirement; it does not implicitly reserve that resource for the conditional. The conditional's own phases declare actual occupancy.

```json
{
  "id": "join-preparation",
  "after": [
    { "before": "tool-preparation", "min_lag": 0 },
    { "before": "program-loading", "min_lag": 0, "max_lag": 600 }
  ],
  "modes": [{ "id": "inspect", "primary": "INSPECTION", "phases": [{ "id": "check", "work": 30, "requirements": [{ "resource": "INSPECTION" }] }] }]
}
```

`after` omitted/null preserves serial list order. `after: []` creates an independent branch within the stage. Explicit dependencies reference another conditional ID in the same active pre or post stage. Pre branches must finish before main work; post branches begin after main work. `releases_product: true` makes post completion part of product readiness. Other post-work can continue after release. Resource reservations and readiness are separate, so independent inspection need not block the original machine.

Nested conditional modes with their own pre/post or material overrides are rejected. Cross-task links target task readiness/main start, not arbitrary internal activity endpoints. Represent independent cross-task operations as ordinary tasks. This boundary is explicit; the model is not an unrestricted temporal event language.

## Sequence context and customizations

Transition tables match neighboring families on a primary resource and separately declare `penalty`, `previous_post` and `next_pre`. Required initial/final transitions and terminal cleanup remain enforced. `sequence_pattern` rules match a family suffix ending at the current task, allowing effects beyond one neighbor. `setup_penalty` is distinct from processing duration, transition work and mode cost.

Native customization hooks receive the complete selected primary-resource sequence and can add activities and charges. They also provide model lowering, dispatch rank, extra objective metrics and hard result validation. Independent validation reconstructs the context, regenerates decorations and recomputes metrics. The linked `dummy_customer@1` policy is a synthetic example; unknown versions fail explicitly.

Markdown records the domain knowledge and counterexamples. It is not executable code. A coding agent can extend the Rust customization, registry, tests and optional UI, then rebuild. Existing typed attribute objectives automatically create a dispatch signal; arbitrary new objectives still need an explicit heuristic implementation and quality tests.

## Locks, pauses and diagnostics

Processing calendars, retention calendars, rates, interruption policy, phased staffing, running/restart work, material readiness, temporal lags and terminal effects share the same construction and validation semantics. `scenario.freeze` accepts global `until` and `until_by_resource`; a resource override replaces the global cutoff before task selection. Freeze dimensions are mode, resource, main start and relative order. Consecutive resource blocks prevent interleaving while allowing time gaps and mode alternatives on that resource.

Unknown fields, unknown references, cycles, missing processing work and incompatible selections are explicit diagnostics. Estimates must be supplied and recorded; no missing parameter is silently invented. Successful construction always passes independent validation. Failed construction is not a mathematical infeasibility proof.

## Objectives, queues and replay

| Location | Fields and purpose |
| --- | --- |
| `Problem` | `queue_policy`, `queue_definitions`, `freeze_zones`, `planning`, `material_policy` |
| `Task` | `stage` (JSON default `default`) and hard `conditional_modes` commitments |
| `Objective` | `metric`, `weight`, `priority`, `maximize` (default false), `scale` (default 1) |
| `Options` | Staged Q policy, route/main/conditional choices, decision prefix/order, Trainer, Plus, combined-improvement and evolution settings |
| `Schedule` | Replay options, actual construction order, search evidence, objective values and material allocation report |

`tardiness` is the unweighted sum of task lateness in seconds. For compatibility, `on_time_delivery` counts due-dated tasks completed on time. The explicit `task_on_time_delivery`, `job_on_time_delivery` and `order_on_time_delivery` metrics use fractions as described in the [KPI catalog](#kpi-catalog-and-units). Never silently map a legacy delivery percentage to a count.

The [search contract](search-and-parity.md) specifies directions, scales, queue semantics, limits and reproducibility. Example staged policy:

```json
{
  "normalization": "robust",
  "candidate_limit": 64,
  "stages": {
    "*": { "deadline_interval_fit": 4, "apparent_tardiness": 4, "setup_penalty": 1 },
    "assembly": { "shortest_work": 2, "slack": 4 }
  }
}
```

A Q ranks candidates; it is not a hard constraint. Mandatory production rules belong in typed constraints, locks, [dispatch policies](architecture/declarative-scheduling.md) or validated native hooks. `decision_prefix` is a hard choice; `decision_order` is a soft task-priority order decoded under those rules. See [direct schedule evolution](direct-schedule-evolution.md) for the latter's search and replay contract.

## Material allocation

`material.prepare` allocates existing stock, confirmed receipts and declared production output. It preserves free workplans and differing-material main modes with `material_policy: "reallocate_routes"`; each complete evaluation allocates supply after its actual route and material-mode selection. The saved allocation report is independently reconstructed during validation.

A flexible model's preparation report is a preview, not the final allocation. Fully selected routes and material modes use the materialized-snapshot contract. Exact workflow, quantities, source preference and re-preparation rules are documented in [material preparation](material-dispatch.md). No missing production or purchase orders are generated, and pegging itself is not globally optimized.

## Urgency through job and order chains

The core derives two dispatch signals per active task: the earliest relevant due date and the highest priority. Tasks within a job share urgency. Explicit job/order dates and priorities contribute, even when the operation has a more relaxed date. Signals propagate backwards over physical dependencies, including actual material-allocation links. Resource sequence locks do not propagate demand urgency.

This affects initial ranking, due/slack/priority/apparent-tardiness Qs, material preparation ordering and urgency exceptions in mandatory policies. Native Q hooks receive `Context.dispatch_urgency`. `task.inspect` returns `dispatch_urgency` separately from the original task.

Business dates and priorities are not overwritten by this propagation. Final task/job/order metrics retain their existing due-date semantics. This is not critical-path backward date scheduling: the propagated date does not subtract all downstream duration, calendars or lag. The existing remaining-work proxy supplies a separate estimate. An explicit new chain-slack metric would need its own semantics and validation.

## Searchable conditional resource alternatives

Each conditional already declares `modes`, including its resources, work, calendars and resource-retention requirements. An optional hard commitment selects a mode:

```json
{"conditional_modes": {"pre:setup": "independent-technician"}}
```

This field belongs to a `Task`. A scenario patch uses `{"kind":"conditional_modes","task":"PRODUCE-FAST","choices":{"pre:setup":"independent-technician"}}`; the map replaces that task's conditional commitments. An empty map removes those commitments. Existing resource/mode/start/order locks remain separate.

Search/replay options use `conditional_choices: {"PRODUCE-FAST":{"pre:setup":"independent-technician"}}`. Keys are exact activity suffixes: `pre:ID`, `post:ID`, `restart:ID`, `transition:TRANSITION:pre:ID` / `post:ID`, and `pattern:RULE:pre:ID` / `post:ID`. Unknown choices, conflicting commitments, absent activities and incompatible routes fail explicitly. A commitment to a task in a route requires that task to remain active. A main mode must support the selected mode-specific conditional.

- **Quick planning:** uncommitted conditionals retain the local earliest-finish choice.
- **Trainer:** optional conditional-mode genes join staged Qs and workplans. Mutation changes one unprotected conditional mode with probability 0.6 when such alternatives exist; crossover exchanges per-task conditional maps. Main-mode/sequence-dependent choices may be infeasible and then count as failed evaluations. Input and explicitly supplied option commitments are protected.
- **Plus:** branches on actually present uncommitted conditional alternatives in the selected prefix, then on subsequent task/main-mode decisions. Conditional branching also works at the configured task-prefix depth boundary. The full evaluation/time limits still apply.
- **Combined improvement:** carries conditional choices from Trainer/incumbent into Plus seed policies. Those choices define that Plus branch's policy; another portfolio seed can explore other alternatives. The optional direct GA also evolves conditional choices under the same hard commitments; see [direct evolution](direct-schedule-evolution.md).

Full decoding, mandatory dispatch policies, replay and independent validation enforce the choices. Static alternatives created by a native `compile` hook are searchable. Sequence-generated alternatives are searchable when a native `Customization::conditional_modes` hook declares a stable catalog in advance. The declaration does not create the activity: decoding must produce the selected activity and mode, or evaluation fails. Undeclared sequence-generated alternatives remain local decoder choices. See the [native extension contract](direct-schedule-evolution.md#native-extension-contract).

## KPI catalog and units

`model.page {schedule_id, section: "metrics", offset: 0, limit: 40}` lists values, units and proxy status. The viewer's collapsed **All KPIs** section reads this endpoint. No new planning controls are required. Saved custom metrics are included with `extension_defined` units unless the built-in catalog defines them.

| Family | Metrics | Semantics |
| --- | --- | --- |
| `task_`, `job_`, `order_` | `tardiness`, `max_tardiness`, `late_count`, `due_count`, `on_time_delivery`, `flow_time` | Completion uses product readiness; tardiness/flow in seconds. Group flow starts at the earliest release of its participating active tasks. |
| Delivery fraction | `*_on_time_delivery` | On-time entities divided by entities with an explicit applicable due date. No due entities yields 1; inspect `*_due_count` to distinguish an empty denominator. Existing `on_time_delivery` remains a task **count** for compatibility. |
| Conditional operations | `conditional_time`, `conditional_mode_cost`, `total_mode_cost` | Productive conditional segment seconds, excluding pauses; sum of selected conditional mode costs; main plus conditional mode cost. |
| `resource:ID:` | `makespan`, `productive_time`, `occupied_time`, `available_time`, `utilization`, `utilization_7d` | Makespan is last reservation end. Productive time counts main/actual processing × required capacity. Occupied time also includes preparation and retention. Capacity-time denominators use positive-rate calendar capacity. |
| Resource utilization | `utilization`, `utilization_7d` | Productive capacity-seconds / available capacity-seconds. First uses [0, resource makespan]; second uses [0, min(horizon, 604800)]. Zero available capacity yields 0. These are not OEE or throughput efficiency. |
| `stage:ID:` | `tardiness`, `conditional_time`, `processing_time`, `utilization_mean`, `utilization_stddev` | Time sums for active tasks in that stage; mean/population deviation of the utilization of the primary resources actually used by that stage. Shared-resource utilization includes its other stages too. |

All built-ins can be explicit objectives with direction, weight, priority and scale. Exact evaluated metrics determine fitness. New group-delivery/max-lateness/count proxies are **experimental**, reusing local due/slack/short-work signals. The held-out diagnostic exposes counterexamples. Resource utilization and conditional/total mode-cost objectives have no automatic standard proxy; `unmapped` remains visible. Adding a metric does not manufacture a well-correlated Q.

Schedules carry `metric_version: 1`. Validation requires the expanded KPI set and detects corrupted values. Version-0 stored schedules may omit the new metrics. JSON parsing enables exact floating-point round trips, including fractional objectives and native extension metrics.

## Reproduce a chat-first scenario

1. Import [the synthetic chain model](../examples/chain-routing.json) with `problem.import`.
2. Call `material.prepare` and retain the returned scenario ID. Routes remain free.
3. Use `schedule.create` or `schedule.improve` with a bounded budget.
4. Inspect `PRODUCE-FAST`: its business due date is 800, while the downstream order produces derived due 75 / priority 9.
5. Page actual material links and KPIs, then open the returned viewer link.
6. Fork before a what-if commitment to a different setup technician. Replan, validate and compare against the original saved result.

An adapter maps source operation resources to conditional modes and BOM lines to explicit operation quantities. An agent can create this input artifact, but missing work, units or unsupported semantics require a diagnostic or an explicit modeling decision.
