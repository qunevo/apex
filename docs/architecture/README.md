# Current architecture

This is the implementation map for agents changing APEX **0.6.0**. Read it after [repository conventions](../../AGENTS.md), then follow the source and test links below. The executable input is `apex.v3.4`; canonical v3.1–v3.3 inputs use the same Rust implementation. There is no separate legacy scheduler in this checkout.

Use the [data model](../data-model.md) for field semantics, the [operating guide](../implementation.md) for commands, and the [migration audit](../migration-audit.md) for historical v2 evidence. This directory documents implemented behavior.

## Runtime boundaries

[Cargo.toml](../../Cargo.toml) defines one Rust package, the `apex` library and the `apex` binary. [lib.rs](../../src/rust/lib.rs) exposes the modules; [main.rs](../../src/rust/main.rs) selects CLI commands and transports. The scheduler does not depend on Python, an LLM or an external solver. Repository Python/Node scripts support fixtures and verification.

```mermaid
flowchart TD
  Agent[Agent client] --> Transport[transport.rs: MCP stdio, MCP HTTP, JSON HTTP]
  Viewer[web/index.html: embedded viewer] --> Transport
  CLI[main.rs: CLI] --> Service[service.rs: scenarios and saved artifacts]
  Transport --> Service
  CLI --> Search[search.rs, improve.rs, evolution.rs]
  CLI --> Engine[engine.rs: common evaluation]
  Service --> Search
  Service --> Engine
  Search --> Engine
  Engine --> Model[compile.rs, domain.rs, language.rs: active model]
  Model --> Dispatch[dispatch.rs: admissible construction decisions]
  Dispatch --> Decode[engine.rs and calendar.rs: placement]
  Decode --> Validate[validate.rs: independent checks and replay]
  Service --> Store[Local .apex artifact store]
```

The service owns persistence; scheduling modules operate on in-memory models. CLI `plan`, `train`, `plus`, `improve` and `evolve` also call those modules directly. Search changes candidate decisions or ranking parameters and uses the same evaluation path as quick planning.

## Source map

All module links below point into `src/rust` unless stated otherwise.

| Responsibility | Entry points and related modules |
| --- | --- |
| Serialized input, options and output | [model.rs](../../src/rust/model.rs): `Problem`, `Options`, `Schedule`, activities, commitments and diagnostics. Additional typed contracts live beside their implementation in `language`, `policy`, `queues` and `evolution`. |
| Production templates | [production.rs](../../src/rust/production.rs): `ProductionInput`, `expand`; expands orders, quantities and workplans into canonical tasks. It does not schedule them. |
| Readiness and indexes | [compile.rs](../../src/rust/compile.rs): `compile`, `Compiled`; validates supported inputs, resolves IDs and builds dependency and dispatch graphs. [urgency.rs](../../src/rust/urgency.rs) derives upstream dispatch urgency. |
| Active problem | [domain.rs](../../src/rust/domain.rs): `resolve`; selects routes, removes inactive work, expands quantity formulas and applies job/order defaults. [language.rs](../../src/rust/language.rs) lowers typed planning templates; [conditionals.rs](../../src/rust/conditionals.rs) checks and applies conditional choices. |
| Material preparation | [material.rs](../../src/rust/material.rs): existing-supply allocation, pegging and route/mode-aware preparation. Flexible allocation runs during active-model resolution; explicit preparation creates a separate scenario. |
| Construction and policy enforcement | [dispatch.rs](../../src/rust/dispatch.rs): `decisions_with`, `choices`; common ready-pool selection. [policy.rs](../../src/rust/policy.rs): mandatory filters and replay. [placement.rs](../../src/rust/placement.rs): private exact-placement oracle for those filters. |
| Ranking and objectives | [queues.rs](../../src/rust/queues.rs): Q definitions, normalization and stage policies. [rules.rs](../../src/rust/rules.rs): bounds, rank, exact objectives and score. [metrics.rs](../../src/rust/metrics.rs): KPI catalog and evaluation. |
| Schedule construction | [engine.rs](../../src/rust/engine.rs): `create`, `evaluate`, `decode`, `conditional_specs`; orchestrates evaluation and places main/pre/post/restart work. [activities.rs](../../src/rust/activities.rs) handles conditional DAGs; [calendar.rs](../../src/rust/calendar.rs) places phased work against resource calendars and occupancy. |
| Search | [search.rs](../../src/rust/search.rs): Trainer and Plus. [improve.rs](../../src/rust/improve.rs): shared-budget portfolio. [evolution.rs](../../src/rust/evolution.rs): direct schedule chromosomes and genetic operators. |
| Independent validation | [validate.rs](../../src/rust/validate.rs): `validate`, `validate_active`, `validate_customized`; reconstructs expected semantics and checks output. Uses `policy::verify` for governed construction and [extensions.rs](../../src/rust/extensions.rs) for native validation and metrics. |
| Native customization | `rules::Customization` declares the contract. [extensions.rs](../../src/rust/extensions.rs) registers, lowers and decorates models. [customizations/dummy_customer](../../customizations/dummy_customer/KNOWLEDGE.md) is the linked synthetic example. |
| Agent API and viewer | [service.rs](../../src/rust/service.rs): `Service::call`, tool implementations and storage. [transport.rs](../../src/rust/transport.rs): tool schemas, JSON-RPC, HTTP and OpenAPI. [web/index.html](../../web/index.html): embedded schedule viewer and optional development workbench. |

## One scheduling evaluation

Trace `engine::create` / `create_internal` when debugging a result. `engine::evaluate` is the shared entry used by search workers.

1. **Check options and lower a native extension.** If a customization is selected, `extensions::registered` resolves its version and `extensions::lower` produces typed input with declared native objectives, Qs and conditional choices. Unknown customizations fail. Validate requested strategies and decision options before constructing a schedule.
2. **Compile and resolve the active model.** `compile::compile` checks the input. `domain::resolve` lowers the planning block, chooses workplan alternatives, applies quantities and conditional choices, and performs configured material allocation. Hard commitments constrain these choices. Compile the resolved problem when it differs from the original.
3. **Choose an admissible construction order.** `dispatch::decisions_with` tracks dependency readiness, mode/resource/order commitments and consecutive blocks. Mandatory policies filter eligible task/mode pairs before heuristic ranking. Qs, classic strategies and soft priority orders rank only the remaining choices. A forced prefix must survive the same checks.
4. **Decorate and decode.** Native sequence hooks can add declared pre/post activities and setup charges. The decoder builds conditional graphs from task/mode activities, neighbor transitions, sequence patterns, terminal cleanup and running-work restart records. `calendar::place` places work and reservations; the engine enforces dependency timing and material availability. A placement failure rejects the candidate.
5. **Evaluate the result.** Compute actual KPIs, objective values and score; attach route/material choices, construction order, replay options and any policy witness. Native metric hooks run here. Non-finite values or objectives without an evaluator fail.
6. **Validate before accepting.** The validator resolves saved choices, checks physical assignments and metrics, and verifies mandatory-policy witnesses. A customized result also passes native validation and independent native metric evaluation. Only an accepted result becomes a search candidate or saved schedule.

Validation does not trust success from the constructor. It shares typed semantics and some helpers with construction, so a new rule still needs negative and deliberately corrupted-output tests.

## Data and semantic boundaries

| Concept | Meaning when changing code |
| --- | --- |
| `Problem` and `Options` | `Problem` contains model facts and persistent commitments. `Options` selects a run's search settings and requested decisions. Do not silently write search preferences back into business facts. |
| `Compiled` | Borrowed problem plus lookup tables, precedence graphs and derived urgency. Rebuild it after changing the active problem. |
| `Decisions` and `Schedule` | Decisions select task order and modes. Decoding produces timed activities. `Schedule.construction` records actual decoder order; assignment array order is not a replacement for it. |
| Work segments and reservations | Segments describe productive work. Reservations describe resource occupancy, including retained pauses. Calendar breaks, rates and retention mean occupied time and work are different quantities. |
| Main end and product readiness | `Assignment.end` is main completion; `ready` includes required post-processing. Product dependencies and completion metrics must use their declared event, not assume these timestamps coincide. |
| Dependency and resource order | A product dependency waits for the predecessor's release plus lag. A resource sequence lock constrains construction/resource order without automatically waiting for independent post-processing resources. `Compiled` keeps separate graphs for these meanings. |
| Constraint, policy and Q | A physical constraint applies to the final schedule. A dispatch policy restricts the next decision against its prospective prefix. A Q is a heuristic ranking value. One cannot substitute for another. |
| Hard prefix and soft order | `decision_prefix` pins decisions. `decision_order` expresses task priorities for direct evolution; dependencies, locks and mandatory filters remain binding. Caller-supplied route/mode/conditional choices must also survive search. |
| Due date and urgency | Due dates remain business facts. Derived upstream urgency can prioritize predecessors without changing their original dates or completion metrics. Soft due dates do not become hard deadlines. |

See [declarative scheduling](declarative-scheduling.md) for filter order, exact probes, the native prefix contract and bounded replay evidence.

## Search shares construction

| Entry point | What it varies |
| --- | --- |
| `schedule.create` / `engine::create` | One constructive pass using the requested/default policy. The CLI and service default to `queues`; `Options::default()` in the library defaults to `due`. |
| `schedule.train` / `search::train` | Evolves Q-policy configurations, weights/normalization and supported choices; evaluates complete schedules. It is instance-specific search. |
| `schedule.plus` / `search::plus` | Retains a bounded UCT prefix tree and completes branches with shared construction. It does not enumerate all feasible schedules. |
| `schedule.evolve` / `evolution::evolve` | Evolves task-priority chromosomes and route/main-mode/conditional genes. Native operators propose candidates; they cannot bypass evaluation. |
| `schedule.improve` / `improve::improve_from` | Runs Trainer, then Plus from retained policies, then optional GA, sharing one budget, incumbent and archive. Defaults are 40% Trainer, remaining allowance Plus, zero reserved GA share. |
| `schedule.repair` | Uses the Trainer path on the current scenario, including its commitments. It reconstructs a schedule; it does not patch a cached schedule in place. |

Search evaluates candidates on worker threads when configured, while portfolio phases run sequentially. Time budgets are soft checks between evaluations/batches; a single expensive evaluation can overrun. Evaluation counts include rejected candidates. An exhausted constructor or search does not prove infeasibility.

The [combined-improvement contract](combined-improvement.md) specifies budget division and incumbent handling. [Search semantics](../search-and-parity.md) and [direct evolution](../direct-schedule-evolution.md) define candidate spaces, reproducibility and operator evidence.

## Service state, transport and UI

The default store is `.apex` under the configured workspace. `service.rs` stores separate JSON artifacts for imports, scenarios and schedules. Writes use a temporary file, flush/sync and rename. `Service::call` holds an exclusive `store.lock` across the entire tool call, including scheduling. Calls sharing a store therefore serialize; search workers provide parallel candidate evaluation inside a call. Chunked imports reduce request/context size, but the complete problem is still held in memory for planning.

A scenario has a revision, problem and optional parent. Patches require `expected_revision`. A saved schedule contains its scenario identity/revision, the exact problem snapshot and the result. Improve/evolve can reuse an incumbent only for the same scenario and revision; after a patch or fork, create a result for that model. Validation and explanations use the saved problem, not the current mutable scenario.

`service::tool_names` and `transport::tools` expose the same 32 operations over MCP stdio, stateless MCP HTTP at `/mcp`, and JSON HTTP. Responses larger than 64 KiB are retained as local artifacts with a bounded reference; paging keeps routine tool output small. Transport/authentication and import contracts are in [agent integration](../agent-integration.md).

The viewer is compiled into the Rust binary with `include_str!`; HTML edits require a rebuild before browser verification. The default UI reads saved plans, KPIs and commitments. `?mode=workbench` enables development controls. Keep ordinary planning and rule changes in the agent workflow; the UI does not run its own scheduling engine.

## Customization boundary

`rules::Customization` covers typed lowering, sequence decorations, candidate filters/rank, Qs, objectives/metrics, validation and genetic proposals. Native implementations are deterministic, versioned, statically linked and `Send + Sync`. Registration is explicit in `extensions::registered`; `dummy_customer@1` is the current bundled implementation.

Keep reusable physical semantics in the core and optional domain behavior in customization modules. New native code needs a build. Tool arguments, Markdown knowledge and `.agents/skills` guide the agent; they do not execute plugins or enforce rules. Sequence hooks used by exact dispatch policies must support meaningful prefix decoration, with tests for every affected interaction.

## Change map for coding agents

Read existing tests in the affected row before editing. Synthetic examples are in [examples](../../examples); never turn customer exports into fixtures.

| Change | Update together | Verification starting points |
| --- | --- | --- |
| Input field or physical rule | Typed model, readiness in `compile`/domain checks, lowering, decoder/calendar semantics, independent validator and schema/docs | [core](../../tests/core.rs), [migration](../../tests/migration.rs), [parity](../../tests/parity.rs) |
| Material/order/workplan behavior | `production`, `material`, `domain`, consumption/release handling in `engine` and `validate` | [material](../../tests/material.rs), [migration](../../tests/migration.rs), [material tool workflow](../../tests/material-workflow.mjs) |
| Mandatory next-choice rule | `language`, `policy`, shared `dispatch`, exact `placement` where needed, replay/diagnostics | [dispatch](../../tests/dispatch.rs), [customization](../../tests/customization.rs); include singleton, forced-prefix and beyond-Q-window cases |
| Objective or Q | Exact `rules`/`metrics` evaluator, direction/scale/priority, readiness, independent metric check, explicit `queues` mapping and search behavior | [search](../../tests/search.rs), [improve](../../tests/improve.rs), [evolution](../../tests/evolution.rs); test actual fitness separately from the heuristic proxy |
| Search or genetic operator | `search`/`improve`/`evolution`, option types, budget accounting, commitment preservation, replay and evidence | [search](../../tests/search.rs), [improve](../../tests/improve.rs), [evolution](../../tests/evolution.rs) |
| Native domain rule | `Customization` implementation, registry/version, typed lowering, metric/validation hooks and synthetic knowledge bundle | [customization](../../tests/customization.rs), relevant dispatch/evolution tests |
| Tool or persistence behavior | Service dispatch, `tool_names`, `transport::tools` schema/OpenAPI and saved-artifact revision semantics | [service](../../tests/service.rs), [MCP smoke](../../tests/mcp-smoke.mjs), [HTTP/MCP](../../tests/http-mcp.mjs) |
| Viewer explanation | `web/index.html` and bounded service fields needed to explain the decision | [viewer plan smoke](../../tests/viewer-plan-smoke.mjs), [dispatch viewer](../../tests/viewer-dispatch-smoke.mjs), [evolution viewer](../../tests/viewer-evolution.mjs) |

For Rust changes run the required checks from the repository root:

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

When serialized types change, regenerate and review affected current schemas with the rebuilt binary:

```text
target/release/apex schema --out schemas/apex.v3.4.json
target/release/apex schema --model production --out schemas/production.v3.4.json
target/release/apex schema --model options --out schemas/options.v3.4.json
```

Use `target/release/apex.exe` on Windows. Keep older schema files for supported input compatibility. Rebuild before MCP/browser verification; follow the [operating guide](../implementation.md) for relevant smoke commands. Generated reports and `.apex` artifacts stay local. Documentation-only edits need link, example and source-reference checks rather than scheduler benchmarks.

## Current limits

- The decoder constructs append-based schedules with integer-second time and fixed resource identities within each phase. It has no completeness or optimality guarantee.
- Exact policy probes use a specialized independent-task fast path or reconstruct `prefix + candidate`. There is no general transactional rollback or incremental cross-resource repair engine.
- The service uses a local file store and serialized tool calls. It is not the removed v2 SaaS deployment or a tenant-management system.
- There is no external solver backend, stochastic simulator, general simultaneous batch formation, arbitrary plugin hot-loading, automatic Q-formula generation or automatic operator synthesis.
- Supported input versions share one runtime; passing current tests does not establish complete v2 parity. Keep measured comparisons in the [migration audit](../migration-audit.md) and separate benchmark workstream.
