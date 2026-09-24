# ADR 0001: Rust core with explicit extension and data boundaries

> Historical source references: the v2 implementation was removed on 24 September 2026. Paths below identify former source files; see the [migration audit](../../migration-audit.md).

Status: accepted architectural direction, 23 September 2026. The [current implementation](../../implementation.md) is documented separately, with explicitly documented coverage and limitations. The decision below remains the target, not a claim that every architectural capability has been completed.

## Decision and scope

Use Rust for the new scheduling domain model, contract compilation, calendar/resource evaluation, move evaluation, validation and heuristic search. The core must run without customer customization, a chat client, Python or an external solver. Optional adapters, user interfaces and solver workers may use other languages.

Rust's compiler-checked [ownership rules](https://doc.rust-lang.org/book/ch04-01-what-is-ownership.html) support memory safety. This helps development, including code produced by agents; it does not establish scheduling correctness or application security. Prefer safe Rust in first-party core code. Keep any required native integration behind a small, reviewed boundary.

The [mandatory operational semantics](../core-semantics.md) are a replacement gate: preserve intended constraints and decision logic before replacing v2. Maintain a migration inventory mapping each legacy behavior and customization point to its new representation, evaluation, validation and synthetic cases. The existing 18 acceptance specifications are a starting set, not an exhaustive audit of all legacy behavior. Record intentional corrections and stronger semantics explicitly.

## Historical v2 solver use: verified source paths

The inspected v2 standard pipeline uses the Cython fast decoder. This is a source-level finding, not a claim about every historical deployment or external customization.

| Evidence | Finding |
| --- | --- |
| Fastplanner decoding (`src/v2/scheduler/fastplanner/decode_output_helper.py`, removed) | Explicitly constructs and calls `Fastdecoding`. |
| Strategy pipeline (`src/v2/strategy/strategy_default.py`, removed) | Supplies `Fastdecoding` to refinement/deep search and final decoding. |
| Trainer evaluation (`src/v2/trainer/eval.py`, removed) | Calls the standard pipeline with candidate queue weights and disables nested training, refinement and deep search. |
| Decoder dispatch (`src/v2/decoder/schedule_decoding.py`, removed) | Contains both the fast path and a separate `CpDecoding` branch. The reviewed standard callers select the fast path. |
| CP decoder (`src/v2/decoder/cp_decoder_default.py`, removed) | Contained an actual OR-Tools `Solve` call in the removed v2 source. |
| CP model builder (`src/v2/decoder/cp_model_builder_default.py`, removed) | Has an early return before objective construction and a commented-out assignment of decoded intervals to the solution cache. It is not an established complete backend. |
| Dependency container (`src/v2/customization/di_container.py`, removed), base classes (`src/v2/customization/di_base.py`, removed) and requirements (`src/v2/requirements.txt`, removed) | Import and construct CP components unconditionally; OR-Tools remains a legacy installation dependency even when solving uses the fast path. |

The new base Rust build must not inherit this unconditional solver dependency. Add backend dependencies only with an explicitly selected optional integration.

## Customization is a first-class contract

Keep a generic default implementation for each extension point. Use versioned configuration and typed rule data for ordinary variations; use Rust traits and composed implementations when custom executable logic is necessary. A decorator can wrap a default implementation through composition. Source-level Rust extensions are built and tested with the engine; runtime Python-style monkey-patching is not the proposed contract.

Hooks have defined stages and outputs:

| Stage | Permitted contribution | Required checks |
| --- | --- | --- |
| Source mapping | Convert files/API records into canonical entities; enrich them from declared sources or estimation rules | Mapping version, source references, units, completeness and assumptions |
| Problem compilation | Generate modes, phases, conditional rules, dependencies and domain-specific constraints | Stable IDs, schema validation, references, cycles and capability coverage |
| Candidate generation/ranking | Suggest moves, rank eligible tasks or change heuristic weights | Existing hard rules and locks still apply; rankings do not grant feasibility |
| Transition/phase evaluation | Determine conditional work, duration, retained resources, handover or restart effects from typed context | Declared dependencies; all generated work has schedulable resources and temporal semantics |
| Hard-constraint evaluation | Return candidate violations or additional restrictions | Independent checking of the corresponding final assignments |
| Objective evaluation | Return named cost components and explanations | Shared units, priorities, normalization and consistent full/delta evaluation |
| Output and explanation | Supply KPI details, diagnostic explanations and viewer metadata | Read only a versioned result; do not mutate the validated schedule |

Every executable scheduling rule declares its ID/version, required fields, parameters, scope, dependencies, hardness, evaluation support and validation support. Solver support is a separate declaration. Pin the active bundle to each scenario and run.

Evaluation context must expose the information required by the rule: relevant task/mode/phase, resource neighbors and state, calendars, execution progress, locks, dependencies and material state. Use typed read-only views and explicit proposed effects, not unrestricted mutation of shared scenario state. Effects pass through the common transaction and propagation mechanism.

Define deterministic composition: additive constraints must all hold; contributions to named objectives combine according to their documented policy; competing replacements of the same decision need an explicitly selected provider. Reject ambiguous overrides and dependency cycles. Extension failure cannot silently disable a hard rule.

Invalidation follows each rule's declared dependencies. A rule that reads global state needs global invalidation unless it provides a correct narrower dependency contract. In-process hooks must be trusted and cannot be treated as a sandbox. Keep external I/O and language-model calls outside the search/evaluation loop. A separate process boundary can be introduced if isolated extension execution is required.

Migration must include the existing service injection (`src/v2/customization/di_container_create.py`, removed), queue hooks (`src/v2/scheduler/fastplanner/fastplanner_default.py`, removed), sequence resolver (`src/v2/decoder/sequence_resolver_default.py`, removed), preprocessing, constraint handling and KPI behavior. Coverage includes interactions with pauses, conditionals, resource retention and locks, not merely whether the hook can be called.

## Large inputs and bounded agent context

Separate three budgets: source import memory, scheduling state memory and agent context. Batching one does not automatically bound the other two. Tens or hundreds of thousands of source rows are a design workload to measure, not an already demonstrated capacity.

The agent should inspect source structure and bounded samples, define a mapping, then run a converter/import tool over the full source outside conversation context. The converter may be generated by the agent. Existing files can be passed by artifact reference; they need not be copied into a tool request as a giant inline JSON string. An agent can also generate canonical records directly into staged artifacts or chunks.

Proposed import lifecycle:

1. `import.begin`: create a durable session with source snapshot, mapping/schema versions, expected partitions/counts where known, and explicit completeness criteria.
2. `import.append`: stage bounded chunks with stable entity IDs, source locations, chunk ID and content hash. Repeating the same ID/hash is idempotent; the same ID with different content is a conflict unless an explicit replacement operation is used.
3. `import.status` / `import.diagnostics`: return counts, readiness, unresolved references and paginated details. Persist progress and resume cursors outside chat history.
4. `import.finalize`: verify all declared chunks, global uniqueness, cross-chunk references, required fields, units and rule coverage. Atomically publish an immutable problem revision only when finalization succeeds. Partial or failed imports remain staging data.

Set configurable row and byte limits, bounded buffering and backpressure. Prefer complete entities or related groups within chunks; unusually large groups use explicit cross-chunk references. Input-format adapters must document their actual memory behavior; a chunked API alone does not make a spreadsheet parser stream efficiently.

Missing processing times or required conditional/phase parameters produce durable diagnostics with entity and source references. Explicit estimation policies retain warnings and provenance. Local chunk checks are followed by global checks, including extension-supplied checks. Never infer completeness from a sample or silently ignore rows rejected during conversion.

Chat and viewer tools return summaries, stable result IDs and bounded pages. Include total issue counts and an explicit cursor when details are omitted. Use queries such as affected orders, one resource's time window or a particular source range instead of returning the whole dataset repeatedly.

Import chunks are transport/storage units. Tasks from different chunks still compete for the same resources, material and time. Scheduling decomposition requires a separate algorithmic decision with explicit coupling constraints. The initial core may hold its compiled problem in memory; measure that footprint independently of import buffering.

## Optional solver port

Define a backend-neutral request/result boundary; do not build a solver integration as a prerequisite for the first heuristic engine. A future Python worker can call native OR-Tools, or a C++ worker can expose another backend through the same contract. OR-Tools documents its supported language interfaces in the [official getting-started guide](https://developers.google.com/optimization/introduction/get_started).

The port must cover:

- Capability discovery against the exact active rules, including custom constraints, interruptions, conditionals and locks.
- Export of a pinned problem/scenario revision, objective definition, time resolution, budget, seed and optional incumbent or neighborhood.
- Backend model construction, execution, progress/cancellation and explicit termination status.
- Import of candidate assignments, conditional activities, segments and reservations, plus mapping back to canonical IDs.
- Independent core validation of the reconstructed candidate before it becomes a valid scenario result.

Model conversion preserves semantics, not just JSON field names. Arbitrary Rust hook code cannot automatically become solver constraints. A custom hard rule needs an equivalent encoding for that backend; otherwise reject that solver request as unsupported. Post-validation alone does not make an incomplete solver model equivalent to the original problem.

Keep infeasible, unsupported, failed, cancelled and timed-out-without-an-incumbent outcomes distinct. A backend optimality claim applies only to the exact encoded problem and declared objectives. Record serialization, model-build, solve and validation time separately before considering a tighter native bridge.

## Delivery implications

First establish the migration inventory, typed contract, rule lifecycle and synthetic reference cases. Then implement the Rust evaluator and validator, followed by fast construction and bounded move/repair operations using the same semantics. Add durable import sessions and a synthetic customization demonstrating both a heuristic hook and an independently validated hard rule.

Acceptance must include resumable imports, retry/conflict handling, missing values spanning chunks, cross-chunk references, bounded tool responses, extension invalidation and rule-version changes. Performance checks must report import memory, compiled-state memory and evaluation latency separately. A solver worker and advanced search remain subsequent optional work.
