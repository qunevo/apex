# Stateful dispatch customization audit

> Historical source references: the v2 implementation was removed on 24 September 2026. Paths below identify former source files; see the [removal note](legacy-baseline.md).

Historical audit of 0.3. The [0.4 implementation report](v0.4-declarative-scheduling.md) records the subsequent shared dispatch layer, tests and remaining limits.

23 September 2026. Source review of a locally available legacy extension, the generic v2 construction path and the current Rust implementation. No private extension source, customer identifiers or production input is included in this report.

**There is a concrete migration gap:** Rust supports static hard restrictions and custom Q scores, but has no native hook equivalent to v2's dynamic `update_masterqueue` candidate filter. The complete-sequence decoration and final validation hooks do not fill that construction-time gap.

## What the legacy path actually does

The generic Fast Planner (`src/v2/scheduler/fastplanner/fastplanner_default.py`, removed) builds and scores a master queue, invokes `update_masterqueue`, then selects from the returned candidates. A removed candidate cannot win by receiving a higher Q weight. The caller skips this hook when only one task is available. Insertion (`src/v2/scheduler/fastplanner/insert.py`, removed) appends the selected task and its conditionals to the construction sequence; this hook is not an arbitrary time-gap insertion algorithm.

The inspected extension uses machine identity, the existing machine sequence, consecutive campaign duration, product/process compatibility and current candidate timing. Its policy order is:

1. Filter candidates using adaptive per-machine downtime limits.
2. If deadline-critical candidates remain, select exclusively from that subset.
3. Otherwise restrict the campaign machine to compatible continuations while a configured minimum campaign duration has not been reached. A particularly compatible continuation may remain exclusive after that duration.
4. Retain other-machine candidates and explicit fallback behavior when there is no fitting continuation.

Compatibility is also exposed as a custom Q. The numerical ranking and the removal of candidates are distinct mechanisms, even when they use the same underlying compatibility signal. This is a hard restriction on a particular construction decision, not an unconditional physical feasibility rule or a static product-to-machine assignment.

The v2 Trainer evaluator (`src/v2/trainer/eval.py`, removed) reruns the configured pipeline with evolved queue weights. When that pipeline injects the extension, the filter remains active during Trainer evaluations. The Explorer (`src/v2/scheduler/fastplanner/controller.py`, removed) also obtains its planner through dependency injection. This does not establish enforcement by every separate legacy refinement path or by an independent schedule validator.

## Current Rust coverage

| Requirement | Current implementation | Boundary |
| --- | --- | --- |
| Restrict a task/product to a machine or execution mode | Modes, resource/mode locks and `Customization::compile` | A mapping policy must actually emit those restrictions; arbitrary product attributes are not interpreted automatically. |
| Fix start time or resource sequence/block | Start/order locks and time-window rules | The membership/order is defined in advance, rather than discovered from the evolving campaign. |
| Prefer a compatible next operation | `dispatch_rank`, native Q definitions and `queue_value` | Soft scoring; weights, other signals and alternative search choices can change the winner. |
| Add sequence-dependent preparation, cleanup or penalties | `sequence` decorations | Runs on the already selected complete primary-resource sequence. |
| Reject a schedule violating a domain rule | Native `validate` | Can reject a result but does not guide construction toward a permitted next candidate. |
| Dynamically remove next-step candidates using campaign and pool state | **Missing** | Needs an explicit construction filter shared by Fast Planner, Trainer and Plus. |

See the [customization trait](../../src/rust/rules.rs), [decision construction](../../src/rust/engine.rs), [Q context](../../src/rust/queues.rs) and [sequence decoration](../../src/rust/extensions.rs). Current dispatch timing is an estimate; it must not be described as a fully decoded occupancy state.

## Executed evidence and legacy defects

Nine isolated probes executed the original candidate-filter method and helper definitions against synthetic objects. Deployment imports and pipeline constructors were omitted. [Probe results](dispatch-policy-probes.json) record the source-text hash, expected retained candidates and exact scope. These probes are not end-to-end v2 schedules or evidence of Rust parity.

The probes cover campaign continuation before/after the minimum, no-fitting-candidate fallback, strong compatibility after the minimum, deadline precedence, downtime preceding deadline selection, adaptive downtime thresholds, preservation of other-machine choices and a defective future-due filter.

The future-due filter subtracts a set of `Task` objects from a set of `QueueObject` wrappers. Those classes use distinct object identity, so the intended removal does not occur. The probe reproduces retention of the future-due candidate. Migration should make the desired due-window behavior explicit instead of silently copying the defect. The single-candidate bypass and deadline/fallback precedence likewise prevent describing the legacy policy as an unconditional minimum-duration constraint.

## Required extension contract (proposed, not implemented)

- Add a deterministic, versioned candidate-filter hook receiving the ready task/mode pool, selected modes and resource history, relevant timing state and commitments. Return retained candidate identities and bounded reason codes; do not fabricate tasks or relax existing hard restrictions.
- Distinguish mandatory feasibility rules from construction policies. A mandatory rule requires independent final validation. A construction policy requires replayable selection evidence, including its explicit fallback and urgency behavior; it need not make every alternative final schedule physically invalid.
- Apply the policy consistently to greedy selection, Trainer evaluations, Plus branch expansion and forced-prefix replay. Do not let a prefix bypass it. Mutation must not turn a mandatory filter into a zero-weight Q.
- Define the interaction with running work, fixed starts, frozen sequences, stages and material readiness. Return a diagnostic for a conflict instead of silently relaxing either contract.
- Resolve candidate-window semantics. An eligible continuation outside the default first 64 ready tasks must not be mistaken for the absence of any continuation; use indexed full-pool queries or explicit widening before a pool-dependent fallback.
- Make timing precision explicit. A campaign rule based on actual occupied/working time needs suitable decoded state or exact checks; summing estimated work can differ across modes, breaks, rates and conditional activities.
- Add synthetic positive, negative, corrupted-output and parallel/replay tests across all three planners before claiming this migration gap is closed.

The scheduling engine was not changed during this audit.
