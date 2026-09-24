# Scheduling core: mandatory operational semantics

> Historical source references: the v2 implementation was removed on 24 September 2026. Paths below identify former source files; see the [removal note](../reports/legacy-baseline.md).

Design contract, 23 September 2026. These requirements extend the [draft input model](input-model.md) and [architecture](agentic-scheduler.md). The [current Rust implementation](../implementation.md) covers a tested subset; this contract remains the broader migration target. It is not a claim that either engine enforces every case below. All examples are synthetic.

Rust is the selected implementation language. The [accepted architecture decision](decisions/0001-rust-core.md) specifies customization hooks, bounded imports and the optional solver boundary.

The replacement core must preserve the intended operational capabilities of v2: breaks, shift changes, interruptions, pre- and post-conditional operations, running work, frozen zones, fixed sequences and resource assignments. These are minimum requirements before replacing v2, not optional customization features. Small implementation milestones may cover subsets, but must expose their coverage and reject unsupported hard rules.

Preserve intended behavior, and record deliberate corrections to legacy defects separately. Matching an invalid legacy result is not a migration success criterion.

## 1. What the existing code establishes

| Area | Observed v2 behavior | Required target semantics |
| --- | --- | --- |
| Calendars and breaks | The Cython decoder (`src/v2/cython/fastdecoder/fastdecoder.pyx`, removed) adds crossed breaks to elapsed duration and considers shift capacity and workstation productivity. | Explicit work segments, rates and resource reservations; distinguish processing time from elapsed time. |
| Calendar defaults | Shift initialization (`src/v2/data/data_container/helper/shift_init.py`, removed) supplies availability when shifts are absent and can extend availability after the last shift. | Declare outside-calendar behavior; a migration adapter must report any such assumption. |
| Conditional operations | Allocations (`src/v2/data/workplan_operation.py`, removed) associate pre/post operations with individual resource allocations. Conditionals can have their own resource alternatives. | Keep the owner task and requirement association, trigger, modes and occupied resources. |
| Neighbor-dependent work | The sequence resolver (`src/v2/decoder/sequence_resolver_default.py`, removed) can change the current operation's pre-conditional and the previous operation's post-conditional. | Recompute both sides of affected resource transitions when sequence decisions change. |
| Conditional boundaries | The task network (`src/v2/data/data_container/helper/tasknet_initializer.py`, removed) links pre/main/post tasks; ordinary operation dependencies are separate. | Explicit events define when downstream production may start and when resources become free. |
| Running operations | The task factory (`src/v2/data/data_container/helper/task_factory.py`, removed) skips pre-conditionals for progressed tasks but still generates post-conditionals. | Preserve completed preparation, remaining work and outstanding post-work; restart preparation requires an explicit rule. |
| Frozen horizon | The constraint handler (`src/v2/preprocessing/constraint_handler_default.py`, removed) selects previous tasks by baseline start time, with global and workstation horizons, then marks them frozen. | Compile a versioned baseline and freeze policy into precise locks. A frozen flag alone does not specify fixed timestamps. |
| Fixed sequences | Fastplanner filtering (`src/v2/scheduler/fastplanner/filtering.py`, removed) reserves a workstation for members of a started fixed sequence until its last member. | Model a contiguous block in resource order separately from relative order and time continuity. |

Some fields and enum members exceed the implemented importer/decoder coverage. For example, conditional slack and several sequence-effect targets are explicitly unimplemented. They need explicit capability decisions, not an assumption of parity based on their names.

## 2. Work, elapsed time and resource occupancy

Use three separate concepts:

1. **Required work:** normalized processing effort derived from quantity, mode and duration formula, or explicit remaining work for an already started phase.
2. **Execution segments:** intervals in which work actually progresses, with the applicable processing rate and assignment.
3. **Reservations:** intervals in which a resource remains occupied, including waiting or breaks where required.

The task's start/end envelope is derived for display and temporal relations. It is insufficient to validate productive work, resource occupancy or utilization. Results must expose segments and reservations, including their task, phase and conditional-activity IDs.

All intervals are half-open. The compiler expands shifts and exceptions into ordered, disjoint profiles. Resource capacity, permission to process, permission to retain a resource and processing rate are distinct. A closed production shift can permit a fixture to remain occupied overnight; maintenance may forbid that reservation. Retained capacity cannot be allocated to another task.

For a phase with normalized work `W`, completion requires the accumulated work over its execution segments to equal `W`, within the declared tick-rounding policy. Rate changes alter progress, not required work. Zero rate means no progress. Multiple simultaneous requirements must be feasible together; their rates are not multiplied implicitly. The duration model identifies how the effective processing rate is derived.

### Breaks, shift changes and interruptions

| Policy | Meaning |
| --- | --- |
| `non_interruptible` | Planned execution must fit a continuous processing window with all required resources. |
| `calendar_resumable` | Work may pause at declared calendar closures or staffing gaps and continue with remaining work. |
| Optional planned preemption | The optimizer may insert additional pauses only if an explicit policy permits it. This is a separate capability from crossing a lunch break. |
| External interruption | An observed stop is an execution fact. Apply the declared resume, restart, rework or failure rule, including any lost work. A planning policy cannot erase an actual stop. |

Specify these policies per phase. Each requirement declares whether a pause retains or releases its resource. Resumption also declares whether the same physical resource is required or a compatible replacement is permitted. A shift change with continuous availability is not automatically an interruption. A named-worker replacement may require a handover activity and simultaneous staffing during handover.

Unattended processing can continue through a worker break only when that phase does not require the absent worker. Resource occupancy and processing permission must still permit the run.

Synthetic examples, all times UTC and all unspecified capacities available:

- A phase needs 90 minutes of work at rate 1, starts at 11:30 and is calendar-resumable. A break from 12:00 to 12:30 produces work segments `[11:30, 12:00)` and `[12:30, 13:30)`. A retained machine is reserved through 13:30; a released worker has no reservation during the break.
- With the same calendar and earliest start, a non-interruptible 90-minute phase starts at 12:30 and ends at 14:00.
- A phase needs 60 normalized minutes and starts at 13:30. Rate is 1 until 14:00 and 0.5 afterwards, with no break. It completes at 15:00: 30 minutes of work in the first segment and 30 in the following hour.

## 3. Pre- and post-conditionals are scheduled activities

Use one activity model for main work, preparation, cleaning, inspection, restart work and handovers. An activity has a role, owner, trigger, execution modes, phases, requirements and temporal links. A conditional is not merely a duration added to its owner's task.

Keep two independent classifications: **placement** (`pre`, `main`, `post`) and **activation** (always required, state-dependent, sequence-dependent or execution-event-dependent). A post-inspection can be mandatory even if it has no sequence dependence. Conditions must resolve deterministically from declared data; no language-model call belongs in activation or move evaluation.

The trigger context may include the selected mode, associated resource requirement, previous/next activity on that resource, initial resource state, and execution progress. Preserve this context rather than caching a single setup duration per job.

Declare the temporal relationship: finish-before-start with permitted waiting, immediate adjacency, or an explicitly supported overlap. Pre/main/post labels alone do not define lag limits or which resources remain occupied while waiting. Preserve alternatives for the conditional's own resources and processing duration.

Completion and release are explicit events:

| Example | Required event semantics |
| --- | --- |
| Post-production quality inspection | Material becomes available and dependent production may start only after inspection, if the rule requires release. |
| Machine cleaning after production | Product may already be available while cleaning continues to occupy the machine and cleaning staff. |
| Setup before processing | The machine must be in the required state at processing start; retained setup and any expiry rule are explicit. |
| Last operation on a machine | Outstanding terminal post-work remains required even without a following production task. |

Initial-state and end-of-sequence rules must be explicit. A missing transition entry is not silently interpreted as zero work. An explicit zero-work transition is valid. Define ownership when previous post-work and next pre-work describe the same physical action; do not double-count or automatically merge two distinct activities.

For a sequence change from `A -> B -> C` to `A -> C`, invalidate the removed transitions and evaluate the new `A -> C` transition. Inserting B elsewhere also changes both neighbors there. The affected work can include A's post-conditional and C's pre-conditional, followed by changes to shared-resource occupancy, downstream readiness and material events.

Generated activities need stable semantic IDs tied to owner, rule and occurrence; array position is not external identity. Replacing a transition records removal/addition and provenance so schedule differences remain explainable.

## 4. Frozen zones compile into independent locks

| Lock | Binds | Leaves free unless separately constrained |
| --- | --- | --- |
| Presence | Whether an operation or route choice must be present | Its time and assignments |
| Resource assignment | A particular requirement to a resource, such as machine M2 | Worker/tool alternatives and timing |
| Mode | The selected complete execution alternative | Timing |
| Time | A specified start, end, interval or permitted time window | Assignments consistent with that time constraint |
| Relative order | A precedes B in a named resource sequence | Intervening work and idle time |
| Sequence block | Ordered members remain consecutive on the named resource | Block placement and calendar-induced gaps |
| Executed history | Actual segments, consumption and completed state changes | Only the explicitly unexecuted remainder |

A consecutive sequence block permits its members' required conditional activities. Other production tasks cannot interleave. It does not imply no-wait execution, and it does not freeze absolute start times. Scope the block to named resources; unrelated machines may continue working.

A freeze-zone policy names its baseline revision, time boundary, global/resource scope, task selection rule and lock dimensions. For example, selection can use baseline starts before the boundary or baseline occupancy intersecting the zone. These are different policies. Resolve per-resource overrides before producing locks.

Membership is evaluated against the pinned baseline, not against the candidate being optimized: moving a task beyond the boundary must not let it escape its locks. Define the closure over member tasks, their conditional activities, selected assignments and executed segments. A lock on a production task does not automatically mean that every adjacent transition is immutable; the policy must say which associated activities are locked.

If a new downtime fact conflicts with a hard frozen assignment/time, report the conflicting lock, resource and interval. Do not silently thaw the plan. Soft stability preferences are a separate objective. Missing fixation targets, contradictory locks and unsupported lock kinds are errors with entity references.

Running work keeps actual history and completed conditionals. Remaining work uses a stated quantity/work basis and timestamp; it does not default to an arbitrary duration. Resume or restart semantics determine whether preparation must be repeated. Outstanding post-work must not disappear.

## 5. Compact data and one decision kernel

Keep the external JSON readable and versioned. Compile it once into indexed Rust structures; external IDs resolve to typed dense IDs at the boundary. Do not scan JSON, resolve strings or repeatedly expand templates inside search.

| Structure | Contents |
| --- | --- |
| Immutable `CompiledProblem` | Tasks, modes, phase/requirement tables, calendars, dependency edges, material events, transition rules, execution facts and locks |
| Mutable `ScenarioState` | Selected modes, resource sequences, activity presence, work segments, reservations, resource states and derived metrics |
| Reversible `MoveDelta` | Proposed changes, affected IDs, invalidated derived values and enough information to commit or roll back |
| `ValidationReport` | Violations, checked rule versions and coverage, assumptions, input revision and candidate revision |

Represent one logical task with several modes instead of duplicating the full task for each resource-list alternative as the legacy task factory (`src/v2/data/data_container/helper/task_factory.py`, removed) does. Share immutable tables between scenarios. Use contiguous storage and indexes suited to measured queries; benchmark calendar and occupancy indexes before committing to a more complex structure.

`create`, `repair`, `improve` and agent-requested moves use the same operational semantics. Their search strategies and budgets differ. Tuning selects heuristic parameters; it neither supplies missing semantics nor validates a plan.

The common move path is:

1. Check the expected scenario revision and cheaply reject moves that directly violate locks or execution facts.
2. Apply a reversible decision change and identify affected resource neighbors, modes, conditional triggers and dependencies.
3. Rebuild affected activities and find feasible work segments and reservations using the same calendar/resource evaluator.
4. Propagate changed readiness, state, material and capacity effects until dependent values stabilize. Shared tools or workers can propagate effects beyond immediate machine neighbors.
5. Update objective deltas and retain or roll back the candidate. If incremental scope cannot be established safely, perform a full evaluation.
6. Independently validate the complete candidate before publishing it as a valid scenario result.

Independent validation reads final assignments and recomputes hard-rule checks; it must not merely trust the evaluator's feasibility flag or caches. It can share domain definitions and unit primitives. An infeasible candidate or failed heuristic repair does not prove the whole planning problem infeasible.

Compiled Rust extension rules should declare their inputs and affected entities so invalidation remains correct. Rules with unknown/global dependencies require broader reevaluation. No external I/O occurs in the evaluation loop. Every active hard extension needs validation coverage. An optional solver backend must advertise which of these semantics it can encode and reject unsupported cases.

## 6. Required synthetic acceptance cases

These are migration acceptance specifications. The [versioned implementation reports](../reports/README.md) record the exercised cases and limitations; full incremental propagation, for example, remains unimplemented. Test intended semantics independently of the legacy result; retain separate cases reproducing known legacy defects.

| ID | Scenario | Required assertion |
| --- | --- | --- |
| C01 | Resumable work crosses a lunch break | The 90-minute example ends at 13:30; the break contributes no processing work. |
| C02 | The same work cannot be interrupted | Earliest placement is 12:30–14:00. |
| C03 | Contiguous shifts change processing rate | The 60-minute example ends at 15:00 without an invented break. |
| C04 | Machine retained, worker released during a pause | Another task cannot take the machine; worker reservations exclude the break. |
| C05 | Staffing changes or handover is required | Every segment and handover meets capacity and identity/qualification requirements. |
| C06 | Closed calendar versus retention-forbidden maintenance | Overnight retention is allowed only by its profile; a conflicting maintenance reservation is rejected. |
| C07 | Pre-conditional needs a separate tool and worker | Its own calendar, duration and simultaneous capacity are enforced. |
| C08 | Post-cleaning versus post-inspection | Resource release and product readiness follow their distinct declared events. |
| C09 | Move a task between two machine sequences | Both source and destination transitions, including previous post-work, are recalculated. |
| C10 | First/last task has conditional work | Initial state and required terminal cleanup are covered; missing transition data is diagnosed. |
| C11 | Freeze machine M2 only | A feasible time or worker change is allowed; a move to M1 is rejected. |
| C12 | Relative order versus sequence block versus no-wait | A/B can retain order with X between only under relative-order semantics; block gaps do not violate the block alone. |
| C13 | Global freeze and resource-specific override | Membership follows the specified baseline policy; candidate moves cannot evade it. |
| C14 | New downtime conflicts with frozen work | A structured conflict is returned without changing the lock. |
| C15 | Resume an already started operation | Actual work stays fixed, remaining work is counted once, completed preparation stays complete, outstanding post-work remains. |
| C16 | Observed failure requires restart work | Lost work and any repeated setup follow the restart rule; they are not silently discarded. |
| C17 | Capacity, material or temporal dependency propagates a change | Incremental and full evaluation agree, including effects on other resources. |
| C18 | Required duration, transition, fixation target or interruption policy is unresolved | Readiness identifies the missing fact or ambiguity; work is not silently omitted or estimated. |

Run these semantics through initial construction, replanning, agent moves and final validation. Reject manually corrupted outputs as well as invalid proposed moves. Compare incremental evaluation against full recomputation on seeded move sequences. Compare feasible results and required events with v2 where its behavior is intended; identical heuristic order is not required.

After correctness, measure time to first valid plan, repair latency distributions, improvement quality at fixed budgets and memory usage on synthetic instances spanning task counts, mode alternatives, calendar fragmentation and conditional density. A Rust port is successful when it preserves the required semantics and delivers measured benefits; language choice alone provides neither guarantee.
