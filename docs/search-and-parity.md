# Search and objectives

APEX 0.6 supports staged Q-policy training, UCT prefix search and [direct schedule evolution](direct-schedule-evolution.md). Use `schedule.create` or `schedule.improve` in planner conversations; [combined improvement](architecture/combined-improvement.md) shares one budget across Trainer, Plus and an optional direct GA. Standalone search tools remain available for explicit diagnostics. The [data-model reference](data-model.md) defines material-mode choices, conditional alternatives, chain urgency and KPI units.

The Rust core exposes the entry points below. Every successful result passes the same hard-constraint validator; training is optimization, not a stronger feasibility certificate.

| Entry point | Decision mechanism | Termination |
| --- | --- | --- |
| `schedule.create` | One dependency-ready construction using stages, eligible modes and normalized Q costs, followed by full decoding | One complete evaluation |
| `schedule.train` / `schedule.repair` | Evolution of staged Q policies and unpinned route/material-mode/conditional choices, with elitism and optional Pareto selection | First active time, evaluation or generation limit |
| `schedule.plus` | Persistent prefix tree, UCT branch selection and randomized top-k complete rollouts | First active time or evaluation limit |
| `schedule.improve` | Trainer/Plus portfolio, with an optional direct-GA reserve (default zero after mixed quality ablations) | One shared time/evaluation limit |
| `schedule.evolve` | Direct operation-priority/mode/route/conditional GA with adaptive operators and an optional validated incumbent | First active time, evaluation or generation limit |

Repair reconstructs schedules. Plus replays prefixes into fresh construction/decoding state; it does not yet maintain incremental decoder snapshots. Neither search proves optimality or infeasibility.

## Limits and parallelization

Pass the scenario ID returned by an import or scenario tool alongside the following `options` object.

```json
{
  "options": {
    "iterations": 128,
    "budget_ms": 5000,
    "seed": 42,
    "trainer": {
      "population_size": 16,
      "generations": 8,
      "workers": 4,
      "mutation_rate": 0.75,
      "crossover_rate": 0.25,
      "selection": "pareto",
      "archive_limit": 32
    },
    "plus": { "workers": 4, "branching": 4, "depth": 8, "exploration": 1.4 }
  }
}
```

Defaults are 128 evaluations, 5 seconds, population 16, one worker and lexicographic selection. There is no default generation limit. The UI supplies its displayed settings explicitly. `iterations` is a maximum number of complete evaluations, **not generations**. `trainer.max_evaluations` overrides it when supplied. Zero evaluations/time disables that limit; at least one applicable limit must remain. Plus requires time or evaluations even if a trainer generation limit is present.

Generation zero is the initial population. An evaluation cap below twice the configured population reduces the initial population to leave room for evolution (minimum two, still subject to the evaluation cap). Evaluations include rejected candidates. Time is checked between worker batches and is soft: no cancellation occurs inside a complete evaluation or prefix expansion. `search.stop_reason` records the first limit reached; generated offspring mutation/crossover counters can include a final unevaluated tail when a budget expires.

Trainer evaluates immutable inputs in scoped Rust threads, sorts results back into submission order and selects the next generation after the batch. Fixed seed/evaluation/generation settings reproduce the result across worker counts when time is disabled and customizations are deterministic. Plus batches use virtual visits; changing its worker count can change the exploration path. Same seed, worker count and evaluation budget reproduce its path. Time-limited runs can differ with machine load. Memory grows with concurrent full evaluations, so maximum worker count is not necessarily fastest.

## Actual objectives and Q proxies

An objective is `{metric, weight, priority, maximize, scale}`. Unsupported metrics, duplicates, negative/nonfinite weights and nonpositive scales fail readiness. Zero weight disables an objective. The final decision score for each priority level is:

`sum((maximize ? -1 : 1) * metric_value * weight / scale)`

Lower scores win; levels are compared lexicographically, smaller priority first. The default is job/order weighted tardiness when grouping exists, otherwise task weighted tardiness, followed by makespan. Attribute and native metric declarations can add objectives; an explicit objective of the same metric overrides its weight/direction/priority. Weights, priorities, scales and directions can be changed by scenario patch and replanning. Existing saved results retain their original inputs. Scores from different objective definitions must not be compared as if they expressed the same preference.

Pareto selection compares individual active metric values after direction and scale conversion. It uses nondominated fronts and crowding distance; priorities and positive weights do not change dominance. The returned single winner still follows the configured lexicographic/weighted decision policy. The bounded `search.archive` contains nondominated trade-offs with complete replay options, not a proof that the true Pareto front was found. Replay with `schedule.create` against the **same input revision and extension version**. The workbench blocks archive replay after a scenario revision change.

### Construction queues

There are **19 standard Qs**: due, earliest start, earliest-start binary, earliest-alternative binary, deadline interval fit, conditional work, downtime, downtime binary, most downtime, setup penalty, shortest work, longest work, remaining work, slack, priority, apparent tardiness, mode cost, fewest alternatives and most alternatives. The binary alternative comparison uses job grouping when available, otherwise the operation's modes. Some declared but inactive/unimplemented legacy queues are not represented as working v2 capabilities.

`Task.stage` partitions construction stages. Numeric stages sort numerically, then nonnumeric names lexically; dependency readiness and fixed/running work remain authoritative. `QueuePolicy.stages` maps stage IDs to queue weights, with `*` as fallback. Lower normalized cost wins. Each Q is normalized over the current eligible candidate pool, then multiplied by its stage weight and summed. `minmax` and the legacy median/IQR sigmoid with minmax fallback are supported. `reverse:QUEUE_ID` reverses a signal. These are alternative priority rankings combined by score, not a pipeline of independent scheduling engines.

The default candidate window is 64 ready tasks plus their eligible modes. `candidate_limit: 0` scans the whole ready set at higher cost. A small window is a speed/quality trade-off, not equivalent to evaluating all tasks. Classic `due`, `shortest`, `priority`, `release`, `random`, `weighted`, `objective` and `setup` strategies remain available. CLI/tool calls default to Q dispatch; the Rust library's `Options::default()` retains `due` for existing callers.

| Actual metric | Initial construction proxies |
| --- | --- |
| Task/job/order weighted tardiness | Apparent tardiness ×4, slack ×1, earliest start ×1 |
| Unweighted task tardiness / on-time task count | Due ×1, slack ×4, shortest work ×2; reversed for an opposing direction |
| Makespan | Earliest start ×2, downtime ×2, remaining path work ×1 |
| Flow time | Shortest work ×3, earliest start ×1 |
| Conditional work / setup penalty / mode cost | Corresponding local cost ×4 |
| Attribute-weighted completion | Automatically generated `attribute:ID`, preferring attribute / estimated work |
| Native metric | Explicit queue definition and native signal hook, or a declared attribute proxy |

Objective weights scale the initial proxy contributions; goal priorities and metric scales are enforced in final fitness, not identically projected into this heuristic sum. The protected deadline Q starts at 4; an implicit horizon does not manufacture task-specific urgency. Apparent tardiness is a local urgency-per-work estimate damped exponentially by positive slack: `priority / work * exp(-positive_slack / (2 * mean_candidate_work))`. Q costs use its negative. Calendar availability, remaining DAG work and conditional/setup effects enter estimates; these are not the full decoder.

The attribute ratio follows the weighted-completion pairwise ordering for independent available work on one machine. Precedence, calendars, multiple resources and releases break that simple guarantee. The [proxy diagnostic](../tests/search_report.rs) exercises cases where the local ranking disagrees with complete-schedule quality. A metric name cannot automatically reveal its correct scheduling heuristic.

`queues.inspect` exposes the registry, mapping, initial policy and unmapped goals. `queue_definitions` can register a task attribute proxy; native `Customization::objectives`, `queue_definitions` and `queue_value` declare the evaluator/proxy contract. Native hooks are `Send + Sync`; their outputs must be finite and deterministic. Missing values fail explicitly. New native code still requires tests, registry linkage and a build. Arbitrary chat text is not executed as a rule.

## Trainer evolution

The genome contains complete per-stage Q weight maps, normalization, route choices, material-mode choices and conditional choices. The initial population includes the requested construction policy, the goal-derived Q policy and a small number of classic reference strategies; remaining members receive four mutations. Reference strategies are converted to Q genomes for offspring.

Mutation chooses a mutable stage/Q weight from `{0,1,2,4,8,16}`. With probability 0.6 it moves to an adjacent level, otherwise another level. `deadline_interval_fit` is protected. A mutation also has a 0.1 chance of changing normalization and a 0.3 chance of changing an unpinned route, and changes the deterministic seed. Input-selected routes and explicit route choices stay pinned; locks and running work constrain all candidates. Unprotected material modes and conditional alternatives can also mutate; see [conditional choices](data-model.md#searchable-conditional-resource-alternatives) and [material-mode search](direct-schedule-evolution.md#material-mode-special-case).

Crossover chooses whole stage weight blocks from either parent, can inherit the other normalization, and exchanges route and conditional genes. Main-mode choices are retained from the first parent and can change through mutation. It does not splice raw operation arrays into potentially invalid schedules. Decode and independent validation reject infeasible offspring; parent elites remain. Default mutation/crossover probabilities match the old 0.75/0.25 configuration, but the population size, normalization mutation and route genes are deliberate differences. This is evolutionary instance optimization, not RL or a learned policy guaranteed to generalize to new factories.

## Fast Planner Plus

Nodes retain a route configuration and a task/mode decision prefix. Expansion offers alternative eligible next decisions; root branching can also explore unpinned routes and material modes. Declared conditional alternatives are searched under the [conditional-choice contract](data-model.md#searchable-conditional-resource-alternatives). UCT selects between expanded branches. Complete rollouts randomize among the top two Q candidates and are scored by actual validated objectives. Rewards propagate up the path; other branches remain available and can be revisited. `nodes`, `revisits` and rejected evaluation counts expose what happened. `revisits` counts repeated rollout leaves, not every traversal through an internal node.

This is a bounded Monte Carlo style tree search. It is not exhaustive over all mode/route combinations, and the depth limit restricts explicit prefix exploration. In v2, the explorer evaluated short paths in parallel, selected one partial prefix and discarded alternatives. It did not retain a UCT tree or backtrack through previous committed prefixes. This was reviewed in the former `src/v2/scheduler/fastplanner/controller.py`; see the [migration audit](migration-audit.md).

## Fixations and evidence

The workbench shades baseline freeze horizons per resource, marks fixed time/resource/mode/sequence/block decisions as T/R/M/S/B, marks running work and connects visible fixed-sequence members. The inspector lists exact commitments. A relative sequence may contain intervening work; a consecutive block may contain time gaps but no intervening operations on that resource. Freezing one dimension does not implicitly fix every other dimension.

Freeze-zone records are provenance/display metadata. Executable `locks` are authoritative. The freeze tool writes both from the same pinned baseline, supports per-resource cutoffs, and preserves baseline membership after replanning. Large pages show only their visible members; absence from a displayed page is not absence from the model.

The [migration audit](migration-audit.md) consolidates historical v2 decoder comparisons, intentional corrections and unverified migration areas. No unrestricted full-system v2 parity claim is made.

## Resource sequence and preparation semantics

Resource order locks constrain construction and primary-resource order; they are not product-release dependencies. Independent post-processing may overlap the next operation on the released primary resource. Explicit operation/material dependencies still wait for product release. A regression test covers fixed starts and consecutive sequences in Fast Planner, Trainer and Plus.

The material ledger's monotone consumption frontier applies at main start. It does not postpone preparatory work solely because another operation has already consumed material at the same main-start timestamp. This preserves feasible frozen production baselines with preparation on separate resources. See the [parity tests](../tests/parity.rs) and [material contract](material-dispatch.md).

## Shared declarative dispatch

The construction-time filter gap identified by the [migration audit](migration-audit.md) is now addressed by the bounded [planning language and native filter hook](architecture/declarative-scheduling.md). All strategies, Trainer evaluations, Plus branch enumeration, direct GA candidates and forced prefixes use the same mandatory filtering stage. Detailed semantics deliberately improve on selected legacy defects and do not claim complete customer-specific parity.

With mandatory policies enabled, the complete eligible ready task/mode pool is filtered before stage selection and Q normalization; the Q candidate window cannot hide a continuation. Exact temporal facts come from calendar placement or complete prospective-prefix decoding, including previous-post effects. Physical feasibility and deterministic policy replay are checked separately. Without policies, the previous bounded-window path remains available.

Evaluate quality and runtime on equivalent models separately from the additional cost and objective trade-offs of new restrictions. The [architecture limits](architecture/README.md#current-limits) describe the current placement and reconstruction boundary.
