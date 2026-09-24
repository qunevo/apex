# v2 to Rust: model and behavior parity audit

> Historical source references: the v2 implementation was removed on 24 September 2026. Paths below identify former source files; see the [removal note](legacy-baseline.md).

## Current update

Version 0.6 implements the direct scheduling GA with adaptive operators, differing-material main-mode search and declared sequence-hook conditional alternatives. See [the 0.6 report](v0.6-evolution.md) for tested cases and remaining boundaries. Full V2 parity is still not claimed.

Version 0.5 adds searchable material/workplan allocation, upstream job/order urgency, explicit conditional-resource choices and additional KPIs. See [the 0.5 report](v0.5-migration.md). The tables below are historical, not a current missing-feature checklist. These additions did not establish full V2 parity; direct schedule evolution was added subsequently in 0.6.

Version 0.3 adds evolutionary stage/Q training, Pareto selection, prefix-tree search, parallel evaluation and actual synthetic legacy decoder comparisons. See [the 0.3 report](v0.3-search-parity.md). The tables below remain historical; they do not describe the final search implementation.

## Status after implementation 0.2

The findings below describe the **0.1 baseline** and are retained as migration evidence. Version 0.2 implements the following responses; details and verification are in [the 0.2 report](v0.2-parity-and-ui.md) and [current executable contract](../data-model.md).

| Audit area | Implemented in 0.2 | Remaining acceptance boundary |
| --- | --- | --- |
| Whole workplans | Route choices with active task presence, alternative edges and route/mode material flows | No raw v2 importer or exhaustive route-combination search |
| Allocation-specific conditionals | Main-mode pre/post activities with validated owner-resource association | Map legacy allocation semantics explicitly; ownership does not infer occupancy |
| Conditional graphs | Parallel/joined pre/post DAGs with temporal lags | Stage-local graphs; unrestricted cross-task internal events remain outside the contract |
| Sequence duration and penalties | Separate penalties, suffix patterns and complete native sequence-context decoration | Private legacy callbacks need case-specific porting and acceptance |
| Job/order/material preparation | Quantity formulas, lot splitting, workplan templates, job/order aggregation and explicit predecessor orders | Automatic BOM explosion, supply allocation and all legacy preparation policies remain adapter work |
| Objectives | Job/order readiness and tardiness, native metrics and dispatch hooks | Full legacy custom KPI catalog is not implicitly reproduced |
| Freeze policies | Global cutoff plus resource overrides, existing independent lock dimensions | Equivalent legacy running/fixation cases still need end-to-end migration comparison |
| Customization | Compile, sequence, dispatch, metrics and hard validation hooks with independent replay | Linked Rust extensions require a build; no arbitrary hot-loaded source |

The synthetic tests establish these declared semantics. They do not establish full binary or behavioral equivalence with v2, and no execution comparison with private extensions is claimed.

## Original 0.1 audit (historical findings)

23 September 2026. Source review of the generic v2 implementation and the current Rust executable contract. This is not an execution comparison against v2 or an audit of private customization binaries.

**The current Rust model is not a lossless replacement for v2.** The calendar, resource, precedence and execution primitives cover important operational cases, but several relationships and decision choices are narrower. Passing the current test suite establishes those tested cases, not complete migration parity.

## Supported primitives

The Rust implementation includes alternative execution modes, simultaneous resource requirements, phase-specific occupancy, processing and retention calendars, changing rates, resumable/non-interruptible execution, task dependencies, material events, pre/post activities, primary-resource transitions, execution snapshots and independent mode/resource/start/order locks. See [the executable model](../../src/rust/model.rs), [engine](../../src/rust/engine.rs) and [semantic tests](../../tests/core.rs).

## Concrete gaps found in the source comparison

| Area | v2 evidence | Current Rust limitation and migration requirement |
| --- | --- | --- |
| Alternative whole workplans | Task-network initialization (`src/v2/data/data_container/helper/tasknet_initializer.py`, removed) builds `task_alt_workplan_tasks`; solution selection (`src/v2/data/solution/solution.py`, removed) deactivates tasks and conditionals belonging to unselected workplans. | Modes choose how one task runs; all supplied tasks are mandatory. There is no coupled route choice controlling task presence, dependencies and material flows. Selecting a route before import removes a decision that v2 can make during construction. |
| Mode/allocation-specific conditionals | Allocation and ConditionalOperation (`src/v2/data/workplan_operation.py`, removed) bind conditionals to a resource allocation inside a particular resource list. Task-network initialization (`src/v2/data/data_container/helper/tasknet_initializer.py`, removed) preserves these relationships. | `Task.pre/post` applies to the whole task. A conditional's own mode alternatives do not bind its activation to the selected main-task mode or requirement. Primary-resource transitions and phases can express some cases, but not the general v2 association without restricting choices or extending the model. |
| Conditional activity graph | v2 generates individual conditional tasks and predecessor edges, with resource-specific associations. | The Rust decoder schedules each task's pre activities in sequence, then main work, then post activities in sequence. The canonical input cannot independently express all conditional branches and their temporal relationships. General activity dependencies and activation rules are needed for a faithful translation. |
| Sequence effects and penalties | Sequence resolver (`src/v2/decoder/sequence_resolver_default.py`, removed) supports operation-property/custom sequence keys, `DependentDurationSec` and separate `SetupPenalty`; KPI extraction (`src/v2/decoder/kpi_extractor_default.py`, removed) aggregates those penalties. Custom resolution receives the surrounding sequence. | Rust uses task `family` pairs on the selected primary resource, with additional pre/post work. It lacks a separate transition penalty and sequence-context customization hook. `transition_work` measures duration and `mode_cost` is constant per mode; neither preserves arbitrary sequence-dependent penalty semantics. Static property matrices may be expanded into composite family states, but this is a limited conversion, not parity for arbitrary custom logic. |
| Job/order/material preparation | Job model (`src/v2/data/item_order_job.py`, removed), job factory (`src/v2/preprocessing/materialdispatching/job_factory_materialdispatching.py`, removed) and material dispatching preserve order/job/quantity relationships and generate or connect jobs. | The Rust core starts from normalized tasks and explicit consume/produce quantities. Templates, unit-based duration calculation, bucket generation, supply allocation and source identity need an adapter/domain layer. Such a general v2 adapter has not been implemented. A temporal inventory ledger alone does not preserve the full preparation model. |
| Objective semantics | v2 KPI extraction (`src/v2/decoder/kpi_extractor_default.py`, removed) derives job completion and lateness and exposes several aggregate/custom KPIs. | Rust currently scores task readiness. Mapping a due date to a final completion task can express some job semantics, but simply copying all task due dates changes the objective. General job/order aggregation and the full v2 KPI set are absent. |
| Fixation policy translation | Constraint handler (`src/v2/preprocessing/constraint_handler_default.py`, removed) resolves global and workstation-specific horizons; fixed sequence elements (`src/v2/data/task.py`, removed) retain alternative resource lists on the same workstation. | Rust resource-scoped order blocks can retain mode alternatives on that resource. Freeze calls select baseline tasks by start time and optional resource. The primitives are available, but a migration layer must resolve the old override policy and interaction tests must establish equivalent behavior. v2 also requires the fixed block to stay on one workstation. |
| Customization surface | v2 extension interfaces (`src/v2/customization/di_base.py`, removed) expose preprocessing, job creation, sequence resolution, KPI extraction and other pipeline components. | The Rust `Customization` trait currently lowers the input model before planning. Arbitrary sequence-dependent evaluation and custom objective/search hooks cannot be expressed solely by this trait; new semantics require changes to the engine and validator. |

The first four rows are substantive decision/model gaps. They cannot all be closed by renaming JSON fields. Precomputing a choice externally may make one instance schedulable but does not preserve the planner's original decision freedom.

## Differences that should not be counted as lost working v2 features

- Explicit buffer material input is rejected by the ordinary v2 importer (`src/v2/preprocessing/preprocessor_default_v2_0.py`, removed).
- Several sequence-effect targets raise `NotImplementedError` in the v2 resolver.
- Additional shift restriction handling is disabled and marked unimplemented in the v2 shift class (`src/v2/data/interval_shift.py`, removed).
- Arbitrary preemption, dynamic named-worker replacement and simultaneous production batches must not be inferred from calendar breaks or resource capacity alone.

These may be future requirements, but their existence as a class, enum or input field does not establish a working legacy capability.

## Required migration acceptance

Before replacing v2, build synthetic cases for each active legacy behavior, translate them without discarding decisions, and validate feasibility and domain meaning in both implementations. Exact heuristic output need not match. Cover at least:

1. Two whole workplans with different tasks, conditional presence and material flows.
2. Different main-task modes with different preparation/cleanup requirements and conditional resource alternatives.
3. Independent conditional branches and transitions affecting the predecessor's post-work and successor's pre-work.
4. Sequence-dependent duration and a separate penalty that can prefer a different order despite equal duration.
5. Job completion across branches, quantity-based work, material supply and objective aggregation.
6. Resource-specific freeze overrides, fixed blocks with alternatives and running work in those blocks.
7. A customization whose decision depends on sequence context, enforced consistently by construction, training and final validation.

The existing 37 tests remain useful evidence for the implemented subset. They are not a substitute for these translation and interaction cases. The next migration milestone should close these gaps before a claim of full v2 coverage or additional performance claims.
