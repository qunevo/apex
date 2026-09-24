# Direct schedule evolution (APEX 0.6)

The planner still uses `schedule.create` or `schedule.improve`. Improvement supports Trainer → Plus → an optional direct schedule GA under **one total evaluation/time allowance**. No additional algorithm choice is required in the normal viewer. The agent can call `schedule.evolve` with a same-revision `schedule_id` to refine a saved plan directly; this spends a separate, explicitly requested budget and retains that validated incumbent. `apex evolve INPUT --options OPTIONS [--incumbent SCHEDULE] --out RESULT` is a diagnostic entry point.

## Search contract

Trainer evolves construction policies. Plus explores decision prefixes under these policies. The new GA evolves a **permutation of operation priorities**, complete workplan selections, main execution modes and conditional modes. It can change orders across Q construction stages. Each proposal still goes through the same dependency-ready set, hard mode/resource/sequence commitments, mandatory dispatch filters, calendar decoder, objective evaluation and independent validator.

`Options.decision_order` is a soft, unique task-priority list; unlisted active tasks follow listed ones. Inactive workplan tasks may appear in the list. Unknown/duplicate IDs are errors. A GA chromosome always covers all model tasks, including inactive alternatives. `decision_prefix` remains a hard commitment. Input locks, started work, selected routes and caller-supplied mode/conditional/prefix choices cannot be relaxed by inherited genes. Native dispatch *ranking* is advisory; mandatory native filters remain authoritative.

The ready-set decoder repairs priority permutations by deferring unavailable operations. It also gives existing fixed/running work its established handling. This is not an unrestricted start-time genome, arbitrary preemption, a complete feasibility solver or an incremental schedule simulator. Infeasible choices are rejected; no constraint penalties buy violations. Inherited conditional commitments that disappear after a sequence change are rejected. The `conditional_reset` operator releases inherited choices, while explicit caller/input commitments remain protected.

`Schedule.construction` records the actual task/mode decision sequence separately from input-ordered assignments. Old saved results without it remain readable; GA seeds from those results use assignment start order as a proposal and retain the original incumbent separately. Replay options reconstruct the selected new result exactly. Inspection metadata cannot substitute for independent validation.

## Population and operators

The population retains distinct chromosomes up to `evolution.population_size`, or `trainer.population_size` when the override is null (the compatibility default). The override accepts 2 through 256. Duplicate chromosomes are removed before survival ranking. Parent selection defaults to a binary tournament over the survival ordering; `evolution.parent_selection = "uniform"` retains random mating. Selection uses the existing lexicographic objective vector or nondominated sorting/crowding (`trainer.selection = "pareto"`). The displayed incumbent always uses the configured lexicographic priorities. A separate incumbent survives population truncation; a supplied incumbent is validated against the same model and requested commitments.

Built-in version-2 operators (applicability and mutation behavior changed; historical version-1 results remain separate):

| Operator | Proposal |
|---|---|
| `insert`, `swap` | Move or exchange operation priorities |
| `job_block` | Move operations of one job together, preserving their relative priority |
| `resource_block` | Move the operations currently assigned to one primary resource |
| `tardy_insert` | Move a currently late operation earlier, using propagated chain urgency |
| `main_mode` | Change an execution mode and release inherited conditional choices on that task |
| `route` | Change a complete workplan; active tasks, dependencies and material allocation are resolved again |
| `conditional_mode` | Change a declared conditional-resource alternative |
| `conditional_reset` | Release inherited conditional choices for local decoder selection |
| `job_crossover` | Preserve a sampled set of job positions from parent A and fill remaining positions in parent B's order; exchange corresponding mode/conditional genes and sampled routes |

Parent-specific applicability excludes unavailable mode/route/conditional changes and tardiness moves without an actually late, movable task. Explicit caller choices, input commitments and running work remain protected. Order mutations use active, unprotected operations. Native hooks retain authority over domain-specific applicability; returning no proposal is handled safely. There are no new bottleneck/setup-focused operators.

Unchanged and previously proposed or decoded chromosomes are skipped before evaluation, recorded in `operators[].skipped`, and receive zero operator credit. They consume wall time but no evaluation allowance. Malformed or rejected proposals still count as failed evaluation attempts. Initialization evaluations remain charged even if they construct the same plan. At most 32 proposals are attempted per requested offspring in a generation; a generation with no new candidates stops with `no_new_candidates`, retaining the incumbent. This is a bounded stopping condition, not an optimality certificate.

Every offspring has **one operator**, including crossover. Consequently its reward is attributable without guessing which of several chained changes caused an improvement. Crossover is credited relative to the better parent by displayed priority score; in Pareto mode it is rewarded for dominance against that reference or a distinct nondominated trade-off. This is an explicit local reward convention, not hypervolume maximization.

`evolution.crossover_share` defaults to 0.5. When both operator kinds are applicable, this first chooses crossover versus mutation; the bandit then selects within that kind. If the chosen kind is unavailable, the available pool is used. Null restores competition across all eligible operators. The share is a proposal probability, not a guarantee about the fraction remaining after duplicate filtering.

`evolution.selection` accepts `uniform` or the default `discounted_ucb`. UCB uses discounted reward and discounted pull count, exploration coefficient 1, discount 0.99, and 10% uniform exploration by default. Untried applicable arms are explored, accounting for pending proposals and recorded skips. A lexicographic improvement earns 1; otherwise 0. In Pareto mode dominance earns 1, a distinct mutually nondominated outcome earns 0.5. Rejected and unchanged proposals earn 0. Counts and rewards are both discounted. Raw evaluation time is recorded separately; operator choice is **not normalized by noisy wall time**.

Proposals are generated by one seeded coordinator for a whole generation. Skipped proposals are credited by that coordinator; evaluated results and their bandit updates are consumed in submission order. Under a fixed evaluation budget, deterministic extensions and the same population/seed, GA results and operator attribution are independent of worker count. A soft wall deadline may stop at a different worker batch. Plus retains its separate worker-dependent tree behavior.

## Shared budgets and evidence

The released default retains 40% Trainer and 60% Plus, with `improve.evolution_share = 0`. Equal-budget evidence did not justify promoting a fixed GA reserve. To run all three phases, an agent can set `improve.evolution_share = 0.3`: 40% Trainer, 30% Plus and 30% direct GA. Unused work carries forward; the actual GA allowance is the remaining global budget. Tiny budgets can finish before GA runs. Keep `improve.evolution_share = 0` for the established two-phase allocation. Shares are scheduling settings for agents/developers, not another planner-facing selector.

The evaluation limit is `trainer.max_evaluations` or `iterations`. Rejected native proposals count as failed candidate attempts even when rejected before decode. A reused internal seed is not evaluated again. `trainer.generations` caps Trainer and GA separately; it does not replace the total bound required by combined improvement. GA counts its initial population as generation 1. Its evaluation allowance is an upper bound: candidate generation can stop early when no new candidates are found. Trainer mutation/crossover probabilities govern Trainer; the direct GA instead selects exactly one operator per offspring.

Time is soft, checked between batches. Model setup, native proposal work, selection and validation contribute to total elapsed time. No cancellation interrupts a running hook/decoder. Fixed-budget quality comparisons must also report runtime. Retaining an incumbent proves non-regression against observed valid candidates **within that run**, not superiority over another method given the whole budget.

`SearchReport.operators` exposes versioned ID, operator kind, evaluated attempts, skipped proposals, rejected/unchanged/reordered evaluated candidates, strict improvements, cumulative reward, evaluation milliseconds, discounted counts/rewards and up to eight distinct rejection codes plus `OTHER`. `reordered` counts valid candidates whose decoded order differs from their active priority permutation; it does not count moved operations. The viewer displays this evidence in one collapsed detail table. No Q formula or operator source code is synthesized at runtime.

## Native extension contract

`Customization::evolution_operators` returns at most 32 unique local IDs with mutation/crossover kind. Their recorded identity includes extension ID and version. `Customization::evolve` receives immutable problem, both parental chromosomes, one parent schedule, and a deterministic seed. It returns one chromosome, `None` for no proposal, or a diagnostic. It cannot return a trusted schedule or replace fitness/validation. All code is linked and tested at build time. Operator changes require a new customization version.

`Customization::conditional_modes` declares stable task → `pre:ID`/`post:ID` → mode IDs for activities that the sequence hook may emit. Unknown tasks, empty IDs and duplicate/empty mode lists fail lowering. The declaration makes choices searchable but does not create an activity. Selected activities must actually appear, with that mode, after decoration; otherwise evaluation/validation rejects the result. Plus examines prefix decorations only when the extension supports that contract, or at a complete prefix. Native filters needing exact placement still require the existing prefix-decoration contract.

## Material-mode special case

`material.prepare` now preserves differing-material main modes as well as free workplans. The existing serialized `reallocate_routes` policy name remains compatible; allocation is performed after actual route **and material-mode** selection. Quick construction proposes the shortest eligible nominal-work mode (stable ID tie-break); random/weighted construction uses seeded mode choices. This proposal can fail through shortage; it is not proof of infeasibility.

Trainer can mutate unprotected material modes. Plus branches on them before conditional/task decisions once routes are fixed. GA evolves execution modes directly. Each complete candidate redoes existing-supply pegging, producer/consumer dependencies and balances. Actual allocation reports record the selected material modes and are independently reconstructed. Prepared fixed snapshots keep their existing commitments. No purchase or production orders are generated and supply allocation itself is not globally optimized.

Tests: [evolution](../tests/evolution.rs), [material](../tests/material.rs), [migration](../tests/migration.rs), [combined improvement](../tests/improve.rs). Historical v2 comparison evidence and unresolved migration boundaries are consolidated in the [migration audit](migration-audit.md).

## Tool-process upgrade

There are 32 agent tools, including `schedule.evolve`. Rebuild and restart the local service after source changes. Existing stdio clients must reconnect to obtain the new executable/tool catalog; a browser reload only updates the viewer. The HTTP service exposes the same operation through `/api/tools/schedule.evolve` and MCP tool discovery.

## Independent larger-population profile

This profile changes only XE. Leaving `population_size` null preserves existing callers that configure the population through Trainer. These settings are configurable experiment choices, not universally optimal parameters.

The 90-run MS/job-flowtime follow-up at 1,000 evaluations found lower mean hypervolume deficits with population 16 than 64 in each of JSP, FJSP and PFSP. Consequently the compatibility default remains unchanged. Population 64 is an experimental option for other budgets, not the recommended default from this pilot. Reproduction and local result paths are in [the benchmark guide](../benchmark/README.md).

```json
{
  "evolution": {
    "population_size": 64,
    "crossover_share": 0.5,
    "parent_selection": "tournament",
    "selection": "discounted_ucb"
  }
}
```
