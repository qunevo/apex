# Migration audit: legacy v2 to Rust

This audit consolidates the migration findings for the former v2 SaaS scheduler and the Rust implementation as of **24 September 2026**, APEX **0.6.0**, source baseline [`9bc9dbf`](https://github.com/qunevo/apex/commit/9bc9dbfe9f9d169b1ea1abf1894004384c9be7d7). It distinguishes implemented capabilities, historical comparisons and remaining acceptance work. It is not a claim of complete v2 parity, production readiness or superior optimization performance.

## Scope and source removal

The reviewed v2 standard pipeline used Python orchestration and Cython fast decoding. Fast Planner constructed dispatch sequences, Trainer tuned queue parameters, and refinement/deep search reused fast decoding. An OR-Tools/CP-SAT path existed but was not selected by the reviewed standard pipeline; its result transfer was incomplete.

On 24 September 2026, the v2 source, deployment files, Python bootstrap and legacy execution/comparison harnesses were removed. Rust in `app/src` is the sole scheduling implementation. Historical `src/v2/...` references identify removed source, not runnable dependencies. The [current architecture](../../app/docs/architecture/README.md) maps the executable modules.

This audit replaces the individual release and migration reports in the current source tree. Their original narratives remain in Git history at the source baseline above. Raw measurements, source-hash manifests and screenshots were local evidence and were not included in that source-only commit. The legacy side of the historical comparisons cannot be rerun from this checkout.

## Capability comparison

| Area | Rust 0.6 coverage | Remaining migration boundary |
| --- | --- | --- |
| Workplans and task presence | Whole-route alternatives select active tasks, edges and material flows; selected routes and commitments remain binding. | No generic importer for v2 exports and no exhaustive route-combination search. Source relationships need explicit mapping. |
| Resource allocations and quantities | Main execution modes retain alternative resource combinations; phased requirements, per-unit work, lot splitting, jobs and orders are modeled. | Units, yield, lot policies and private preprocessing semantics must be supplied by the adapter. |
| Conditional work | Task- and main-mode-specific preparation/cleanup, stage-local conditional DAGs, own resources and calendars, searchable conditional modes and declared native alternatives. | Owner-resource association does not infer occupancy. Arbitrary cross-task internal event graphs are not supported; native sequence-created choices need stable advance declarations. |
| Calendars and execution | Processing rates, breaks, retention, resumable/non-interruptible work, running remainders, restart activities and product readiness are independently checked. | Arbitrary preemption and dynamic named-worker replacement within one phase remain outside the contract. |
| Sequence effects | Neighbor transitions, terminal cleanup, family-suffix rules, separate setup penalties and complete native sequence decoration. | Private callbacks require case-specific implementation and acceptance. Tight maximum lags can defeat construction despite another feasible schedule existing. |
| Material and demand chains | Existing stock, receipts and supplied production output are pegged after actual route/material-mode selection. Upstream urgency propagates without rewriting business due dates. | No MRP or automatic creation of missing production/purchase orders. Pegging is deterministic, not globally optimized; full legacy preparation-policy equivalence is unproven. |
| Objectives and KPIs | Task/job/order completion and delivery metrics, resource/stage KPIs, conditional costs, explicit directions/scales and native objectives. | The complete legacy KPI catalog, slack aggregates and private formulas are not ported. Legacy units and aggregation must be mapped explicitly. |
| Freeze and commitments | Separate mode/resource/start/order locks, consecutive resource blocks, baseline freeze zones and per-resource cutoff overrides. | Each production-specific running/fixation policy still needs end-to-end acceptance; a frozen flag alone does not establish equivalent semantics. |
| Mandatory dispatch policies | Shared candidate filtering, exact placement probes and deterministic policy replay apply to XG, XH, XT and XE. | Supported templates and tested native hooks are bounded; arbitrary legacy policies are not automatically imported. |
| Search | Evolutionary Q policies, Pareto selection, UCT prefix search, direct priority/mode/route/conditional evolution and native genetic operators. | Search trajectories, annealing and scoring are not identical to v2. Repair and evaluations reconstruct schedules; there is no general incremental simulator or optimality certificate. |
| Customization and service | Linked Rust hooks cover lowering, sequences, candidate filters/ranking, metrics, genetic proposals and hard validation. MCP/HTTP expose versioned scenarios and saved results. | New native code requires a build. This does not migrate the old SaaS deployment, tenant identities, customer adapters or private integrations. |

Current field semantics and operational limits are in the [data model](../../app/docs/data-model.md), [operating guide](../../app/docs/implementation.md), [material contract](../../app/docs/material-dispatch.md), [search contract](../../app/docs/search-and-parity.md) and [declarative scheduling contract](../../app/docs/architecture/declarative-scheduling.md).

## Historical execution evidence

Six deliberately synthetic cases ran the unmodified v2 Python/Cython pipeline. The comparison then pinned its route, mode and order decisions in Rust to compare decoding, not optimizer quality or stochastic trajectories. Five cases matched main-work intervals; one documented a deliberate rate correction.

| Case | Recorded result |
| --- | --- |
| Quantities, modes and technological precedence | Same four active operations and main-work intervals; makespan 1,140 seconds |
| Resumable calendar break | Matching intervals; makespan 1,440 seconds |
| Mode preparation and predecessor cleanup from a sequence matrix | Matching main-work intervals; makespan 1,220 seconds |
| Reversed consecutive sequence on a fixed resource | Matching intervals; makespan 1,365 seconds |
| Alternative whole workplan | Same two active operations; makespan 390 seconds |
| Productivity changing from 0.5 to 1.0 | Rust completed 300 units of work at time 375; legacy reported time 300 |

The productivity difference came from the legacy overflow branch using an already clipped interval end. Rust integrates `150 * 0.5 + 225 * 1 = 300`; reproducing the shorter legacy interval would violate that work requirement. The 0.6 verification recorded the same five matching cases and the same intentional correction.

Two additional workplan/material cases compared normalized allocation quantities against the v2 `MaterialDispatcher`: two raw units came from stock, with one produced part each allocated to assembly and packing. Both Rust routes validated. This tested source quantities, not identical routing heuristics or complete job-adapter equivalence.

Nine isolated probes examined a legacy candidate-filter extension on synthetic objects. They covered campaign continuation, fallback, compatibility, deadlines, downtime and future-due filtering. They were not end-to-end schedules or proof of Rust parity. The probes identified a future-due filter subtracting incompatible object types and a single-candidate bypass; the Rust policy contract makes its intended rules and exceptions explicit.

## Intentional differences and unsupported assumptions

- Business dates remain separate from derived dispatch urgency. Legacy recursive date changes and day-based metrics must not be copied into second-based Rust objectives without an explicit mapping.
- `on_time_delivery` remains a task count for compatibility. Explicit `task_on_time_delivery`, `job_on_time_delivery` and `order_on_time_delivery` metrics are fractions; they are not interchangeable with that count.
- Legacy conditional durations could be filled by sequence effects even when the initial conditional work was zero. Importing only the initial duration would discard required work.
- XT (formerly Plus) retains a UCT tree; the reviewed v2 explorer selected and committed partial prefixes. The direct Rust GA is a redesigned capability, not a reproduction of v2 random trajectories or annealing.
- A declared but rejected or unimplemented v2 feature is not evidence of a lost working capability. The reviewed importer rejected explicit buffer-material input, some sequence-effect targets were unimplemented, and additional shift restrictions were disabled. Neither arbitrary preemption nor simultaneous production batches follow from the existence of resource-capacity fields.

## Acceptance still required

Runnable synthetic coverage is in [core](../../app/tests/core.rs), [parity](../../app/tests/parity.rs), [migration](../../app/tests/migration.rs), [material](../../app/tests/material.rs), [dispatch](../../app/tests/dispatch.rs), [customization](../../app/tests/customization.rs) and [evolution](../../app/tests/xe.rs). These tests include semantic cases, rejected inputs and corrupted outputs; they establish their tested contracts, not every former customer workflow.

For a real migration, map each active v2 behavior and customization to typed input, construction, evaluation and independent validation. Preserve route/resource freedom, material quantities, conditional readiness, running work and freeze dimensions. Record intentional corrections separately from equivalent behavior and validate representative interactions with domain owners.

Runtime and solution quality require separate matched-budget measurements. A retained incumbent establishes non-regression within that run, not superiority over another method. Removing v2 source, passing synthetic tests or adding a new operator does not by itself establish complete migration parity.
