# Rust implementation: results and capability report

23 September 2026. Engine 0.1.0, executable input `apex.v3.1`.

**Historical report.** The executable, model, agent transports and UI have since been extended. Read [the operating guide](../implementation.md) for current capabilities and the [report index](README.md) for subsequent measurements. The measurements below remain evidence for the original 0.1 build.

The repository now contains a runnable Rust planning service, a local workbench and a tested MCP integration. Fast planning, heuristic training, scenario changes and independent validation share one typed domain model. The initial implementation is useful for synthetic experiments and for developing agent workflows. It is not yet a complete migration of every legacy behavior or a production release.

The subsequent [source-based v2 parity audit](v2-parity-audit.md) identifies concrete model gaps, including whole-workplan choice, allocation-specific conditionals, conditional activity graphs and separate sequence penalties. These require more than additional test coverage.

## Delivered implementation

| Module | Implemented behavior |
| --- | --- |
| Typed input and readiness | Canonical tasks, alternative modes, phases, resources, calendars, dependencies, materials, execution snapshots, locks and rules; generated JSON Schema; source-linked errors for missing work and invalid references. |
| Scheduling kernel | Indexed occupancy profiles, capacity and rate calendars, resumable/non-interruptible work, retained resources, simultaneous requirements, pre/post activities, sequence-dependent transitions and terminal cleanup. |
| Fast Planner | Dependency-ready dispatch, load-based mode choice, constructive placement and complete final validation. |
| Trainer | Six initial dispatch strategies followed by seeded dispatch-weight search; best valid lexicographic score, with reproducible strategy/seed/weights. No RL or external solver. |
| Scenario service | Durable imports and artifacts, revisions, independent forks, typed patches, freeze dimensions, reconstruction-based repair, comparison and pagination. |
| Customization | Typed hard windows and attribute objectives; an attribute objective supplies a matching dispatch signal. Native Rust decorators lower domain policy into the canonical problem. |
| Agent integration | 21 MCP tools over stdio, official SDK verification, bounded responses, schema discovery, local artifact references and project-local Codex configuration. |
| Workbench | JSON upload, synthetic demo, Fast Planner/trainer, scenario controls, KPIs, paginated Gantt/occupancy, concurrent pool lanes, retained-resource outlines and task inspection. |

The new executable has no Python/Cython runtime dependency and does not use `_dir_init`. Optional conversion and benchmark scripts may use Python. The legacy source was preserved when this report was written and has since been removed; its [source hash manifest](legacy-baseline.json) remains historical evidence. See the [legacy assessment and removal note](legacy-baseline.md) and [operating guide](../implementation.md).

## Verification results

The final `scripts/build.ps1` run passed formatting, Clippy with warnings denied, all **37 Rust tests**, and the release build. The tests comprise 28 scheduling/validation tests, one native customization test and eight service tests.

Additional checks passed:

- Official MCP SDK lifecycle, discovery of all 21 tools, schema discovery, complete scenario workflows, error envelopes and a 100,000-task chunk import.
- Browser workflow in headless Edge: demo, task inspection, scenario fork, filter/pagination without losing the active fork, patch, repair, comparison, objective change, validation and file import. No browser script errors were observed; the 800-pixel viewport had no horizontal page overflow.
- A separate CLI process validated every result in the richer fixture benchmark.
- All 161 legacy source hashes still match the captured baseline; the generated JSON Schema matches the built executable, and the links in the new entry-point documentation resolve locally.

Evidence: [Rust core tests](../../tests/core.rs), [service tests](../../tests/service.rs), [customization test](../../tests/customization.rs), [SDK test](../../tests/mcp-smoke.mjs), [SDK results](agent-workflows.json), [browser test](../../tests/viewer-smoke.mjs), [browser results](viewer-tests.json), [workbench screenshot](viewer.png).

### Operational coverage

Case IDs refer to the broader [migration contract](../architecture/core-semantics.md). A similarly named test does not imply that every variation of that contract is implemented.

| Cases | Evidence and boundary |
| --- | --- |
| C01-C04 | Breaks, non-interruptible placement, changing rates and machine retention with worker release are tested. Calendar interruptions are supported; arbitrary preemption and dynamic identity replacement within a phase are not. |
| C05 | Explicit phases can change staffing and include a handover phase. The test exercises different phase requirements. Automatic qualification matching and worker handover on every shift boundary remain outside this version. |
| C06 | Processing availability and retention permission are separate. Tests cover retention-forbidden downtime. |
| C07-C08 | Conditionals have independent modes/calendars/resources; product readiness distinguishes post-inspection from resource-only cleaning. |
| C09-C10 | Sequence reconstruction recalculates neighbor-dependent previous post-work, initial transitions and terminal cleanup. Tests cover changed order on a resource. An incremental cross-resource move equivalence suite is still needed. |
| C11-C12 | Resource, mode, start, relative-order and consecutive-block locks are implemented. Tests cover resource-only freedom, block interleaving rejection and phase continuity. An order block permits time gaps; it does not implicitly mean no-wait. |
| C13-C14 | Freeze membership is selected from an immutable baseline by start before a boundary, optionally for a resource. Locks survive scenario changes and conflicts are reported. Multiple calls can express different resource boundaries; there is no general overlapping-zone precedence language. |
| C15-C16 | Fixed actual work, explicit remaining work, retained occupancy until resumption, outstanding post-work and explicit restart work are tested. Lost work must be supplied through the execution/restart input. |
| C17 | Full construction and validation enforce capacity, inventory and temporal dependencies. There is no incremental evaluator yet, so incremental/full equivalence is not claimed. |
| C18 | Missing processing time, invalid fixation targets, cycles, unresolved transitions and unsupported fields produce errors. The converter must explicitly declare interruption and estimation assumptions. |

Corrupted-output tests separately alter work, reservations, metrics, assignment coverage and custom-rule compliance. The validator checks the supplied assignments rather than accepting a planner success flag. It shares typed domain definitions and transition resolution with the engine; this is not a formally independent implementation or a proof of correctness.

## Runtime measurements

Measured on Windows x64, Intel Core Ultra 9 275HX, 24 logical processors, approximately 63.4 GiB physical memory, Rust 1.98.1, optimized release build. Planning candidates run sequentially. See [environment metadata](environment.json).

The table reports the median of three repeats on a deterministic, deliberately simple dataset: independent tasks, two alternative modes, four machines, no breaks or conditionals. The engine timer includes compilation/readiness, construction, decoding and independent validation. It excludes JSON serialization, disk I/O and process startup. Trainer runs use 12 evaluations and a 60-second budget checked between evaluations.

| Tasks | Fast Planner median | Trainer median, 12 evaluations | Reduction in weighted tardiness |
| ---: | ---: | ---: | ---: |
| 100 | 0.459 ms | 5.422 ms | 0.00% |
| 1,000 | 4.927 ms | 56.286 ms | 19.57% |
| 10,000 | 63.180 ms | 814.730 ms | 19.89% |
| 100,000 | 1,045.607 ms | 13,077.393 ms | 23.09% |

All 24 measured results were valid. At 100,000 tasks, Fast Planner ranged from 1,045.524 to 1,047.553 ms; trainer ranged from 12,895.522 to 13,097.144 ms. Improvement is relative to the Fast Planner's default ordering on the same input, not to an optimal schedule. Three repetitions of one synthetic family do not establish industrial performance or statistical generalization. No equivalent v2 benchmark was run, so no speedup over v2 is asserted. Peak memory was not measured.

Reproduce the [raw results](runtime-raw.json):

```text
target/release/apex bench --sizes 100,1000,10000,100000 --iterations 12 --repeats 3 --out docs/reports/runtime-raw.json
```

### Richer shift-factory scenario

The [16-task fixture](../../examples/shift-factory.json) contains cut/assembly dependencies, alternative cutters, worker and tool capacity, a break, a later rate change, preparation, post-inspection, terminal cleaning and material flows. This is a small semantic scenario, not a large industrial benchmark.

| Measure | Fast Planner | Trainer, 12 evaluations |
| --- | ---: | ---: |
| Median engine time | 0.245 ms | 1.879 ms |
| Median CLI wall time | 10.838 ms | 13.312 ms |
| Weighted tardiness | 7,950 | 2,320 |
| Makespan, seconds | 5,250 | 4,890 |
| Transition work, normalized seconds | 1,080 | 720 |

The trainer selected the shortest-work strategy and reduced weighted tardiness by 70.82% for this fixture. Each result passed a separate CLI validation call. The CLI wall timer includes process startup and JSON input/output, and excludes that separate validation command. Sub-millisecond engine measurements should be interpreted cautiously.

Reproduce the [fixture measurements](shift-runtime.json):

```text
python scripts/benchmark_fixture.py --repeats 3 --iterations 12
```

## What an agent can do now

These are tool-level integration results driven by a deterministic SDK client. They demonstrate executable workflows available to an agent, not an evaluation of a model's ability to interpret arbitrary business prose.

| Planner request | Exercised workflow and observed result |
| --- | --- |
| "Create a plan and keep the early resource assignments/order." | Import the shift factory, create a schedule and freeze selected dimensions from its baseline. Time remains adjustable when only resource/order are frozen. |
| "CUT0 is unavailable for the first 900 seconds. Show the impact." | Fork, apply downtime with retention forbidden, repair with six candidates, compare and validate. All 16 tasks changed; makespan rose from 5,250 to 6,338 seconds and weighted tardiness from 7,950 to 17,491. The disruption's cost is visible. |
| "Prefer completion of high-urgency tasks." | Add a typed attribute objective and train again. The selected strategy became `objective`; the named metric was computed as 131,881. Makespan was 6,113 seconds, but weighted tardiness increased to 24,866. An objective change can improve a different preference at the expense of the previous KPI; the earlier schedule's urgency metric was not measured in this workflow. |
| "Load 100,000 operations without filling the chat." | A local generator writes 50 chunks sequentially through a reused file path, 2,000 tasks per chunk; the server persists each accepted chunk. An idempotent retry, finalization and a three-task page were verified. See the batching measurement below. |
| "Add a hard policy to this customer's scheduler." | The native decorator test adds a release window and objective, schedules and trains the lowered problem, and rejects a deliberately corrupted assignment. A coding agent can follow the same documented workflow and rerun the checks. |
| "Change the schedule using stale information." | A patch with an outdated revision is rejected. Failed multi-patch requests do not partially update the scenario. |

Weighted tardiness is currently a **task-level** metric: `sum(priority * max(0, ready - due))`. Order-level completion aggregation must be mapped explicitly, for example by attaching the due date to the order's final task. Objective priorities are lexicographic, not an implicit weighted compromise.

### Batching and context size

The official SDK import transferred **100,000 tasks / 62,677,830 bytes** through artifact paths in **3,449.529 ms**. This wall time includes local synthetic generation, chunk file writes, tool calls, persistence, global finalization and the final page query. It does not include planning or LLM inference. The largest append response's structured payload was **130 bytes**; this excludes JSON-RPC framing and the text copy of structured content.

The agent needs a sample and schema information to generate a converter; bulk records can remain in files. Idempotent chunks avoid repeating accepted work. Finalization performs global reference/count/readiness checks, so chunking does not bypass cross-chunk validation. Responses above 64 KiB are retained as explicitly referenced artifacts, and diagnostics are pageable.

Batching bounds tool transport and chat context. It does **not** imply constant-memory scheduling: the final canonical problem is loaded in memory, and immutable problem snapshots are stored with results. Long-term storage, memory and many-concurrent-user benchmarks remain open.

## Customization development contract

The [dummy customization knowledge base](../../customizations/dummy_customer/KNOWLEDGE.md) records the meaning of rules, objective units, counterexamples and the development checks. A supplied attribute objective currently creates both its metric and dispatch signal. Existing typed policies can be changed through tools without a rebuild.

For a genuinely new rule or objective, a coding agent must implement the typed representation, readiness checks, evaluation, dispatch/search behavior and independent final validation together, then add positive, negative and interaction tests. Native decorators currently transform the model before scheduling; they are not arbitrary callbacks at every scheduling decision. New engine semantics require rebuilding and restarting the tool process. Markdown is knowledge for the agent, not executable policy.

## Remaining migration work

- Demonstrate parity on a broader synthetic translation of the intended v2 use cases, including cross-resource conditional changes and stronger combinations of freezes, materials and maximum lags. A general v2-input migration converter is not included.
- Add incremental repair/local neighborhoods, explicit plan-stability objectives and quality benchmarks on constrained large instances. Current repair reconstructs the whole candidate and can move many tasks.
- Extend the executable model where needed for whole-route alternatives, lot splitting/production batches, qualification selection, dynamic worker replacement and explicit order-level KPIs. The current input is already normalized to tasks and modes.
- Add objective-specific heuristics for new objective families and measure them. There is no general automatic derivation of an effective heuristic from arbitrary code or natural language.
- Implement a real optional solver adapter. `solver.solve` currently returns `UNSUPPORTED_BACKEND`; export/import semantics and backend encodings still need work.
- Add a database/multi-user deployment model, asynchronous jobs and hard cancellation, crash-injection tests and memory/storage measurements before operational deployment. The HTTP viewer is a loopback development service.

Heuristic construction can miss a feasible schedule; failure is not an infeasibility proof. The trainer searches alternatives and improves the tested score, but does not certify optimality or robustness. Budgets are checked between candidates and can be exceeded by one candidate. Existing licensing documents remain unchanged; this implementation does not publish the repository or resolve the separate publication audit.
