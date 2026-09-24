# Rust implementation and operating guide

APEX **0.6.0**, 24 September 2026. The executable model is **`apex.v3.4`**, with canonical v3.1/v3.2/v3.3 input compatibility. The bounded [declarative planning language](architecture/declarative-scheduling.md) adds shared dispatch policies, exact placement probes and replayable explanations. The [architecture map](architecture/README.md) describes current modules and control flow. The Rust runtime is independent of the removed legacy Python/Cython implementation; [migration audit](migration-audit.md) remains available.

The [executable model](data-model.md) covers route- and mode-aware material allocation, upstream chain urgency, conditional choices and grouped/resource/stage KPIs. [Direct schedule evolution](direct-schedule-evolution.md) adds priority chromosomes, adaptive operators and declared native conditional alternatives. The implementation includes route selection, mode-dependent conditional activity graphs, quantity/order expansion, native customization hooks, a heuristic planner/trainer, independent validation, durable scenario tools and a browser workbench. This is experimental software; tested feature coverage does not establish universal equivalence with every v2 customization or production readiness.

## Build and run

Install stable Rust and its native linker. Windows needs Visual Studio C++ Build Tools for the MSVC target. This checkout was tested with Rust 1.98.1. The executable needs neither Python nor an external solver.

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release

target/release/apex expand examples/production-orders.json --out .apex/production-expanded.json
target/release/apex plan .apex/production-expanded.json --out .apex/schedule.json
target/release/apex train .apex/production-expanded.json --iterations 128 --workers 4 --out .apex/trained.json
target/release/apex validate .apex/production-expanded.json .apex/trained.json
target/release/apex serve --port 8765
```

Create `.apex` before writing CLI artifacts. On Windows use `apex.exe`. Open `http://127.0.0.1:8765`. The executable embeds the frontend, so rebuild and restart after editing `web/index.html`; stop the owned process before replacing its executable on Windows.

### Bash helpers

The repository helpers use Bash on Linux/macOS or Git Bash on Windows; PowerShell is not required. They resolve the checkout from their own location, so they also work when invoked from another directory.

```bash
bash scripts/build.sh
bash scripts/start-viewer.sh
# Optional port (default: 8765):
bash scripts/start-viewer.sh 8766
# Configure this checkout for a local MCP client:
bash scripts/setup-mcp.sh
```

[build.sh](../scripts/build.sh) runs formatting, linting, tests and the release build in order, stopping on failure. It finds Cargo on `PATH`, then under `CARGO_HOME` or `$HOME/.cargo`. [start-viewer.sh](../scripts/start-viewer.sh) selects the platform's release executable and runs the server in the foreground; use Ctrl+C to stop it.

[setup-mcp.sh](../scripts/setup-mcp.sh) appends the APEX entry to the ignored `.codex/config.toml`, preserves other settings and leaves an existing APEX entry unchanged. It accepts an optional checkout path: `bash scripts/setup-mcp.sh /path/to/apex`. Under Git Bash it writes native Windows paths for the MCP host. Build for the environment that runs the client: Git Bash uses the Windows binary; WSL uses a Linux binary and Linux paths for a client running inside WSL.

## Model and UI

Read [the executable data model](data-model.md) for exact field and unit semantics, relationship choices, conditional DAGs, sequence penalties, material overrides, freeze policies and explicit boundaries. The workbench also exposes search limits, workers, goal editing, Pareto candidates and distinct freeze/lock markers. Regenerate schemas with `apex schema --out PATH` and `apex schema --model production --out PATH`.

The workbench offers a production example, canonical/production JSON import, resource occupancy with separate work and reservations, task inspection, jobs/orders, alternative workplan selection, rules/locks, KPI comparisons and a structured resource-outage scenario form. The inspector distinguishes processing end, product readiness, selected mode and quantity. Route changes are actual model decisions followed by replanning. Views and individual tasks can be linked from an agent chat.

## Agent access and large input

[Agent integration](agent-integration.md) documents MCP stdio, Streamable HTTP, plain HTTP tools, generated OpenAPI and deployment configuration. All transports use the same 32 tools. No model API key is required by APEX; the conversational host supplies its own agent.

An optional source adapter can be a connector, an agent-created conversion script or a ready canonical artifact. Read samples and mapping metadata into context, then convert complete source artifacts outside the model. Durable task imports accept at most 5,000 tasks and 4 MiB per chunk. Finalization validates cross-chunk references. Oversized responses become artifacts inspectable remotely through `artifact.read`. Batching bounds transport and context; compilation and planning still hold the problem in memory.

Missing processing work and invalid references produce structured diagnostics with source references. The core does not invent estimates. Record deliberate assumptions in the input. Imported data is not executable code.

## Quick planning and combined improvement

Use `schedule.create` for a quick plan and `schedule.improve` for Trainer followed by Plus and an optional direct schedule GA under one shared budget. An optional current-revision incumbent is independently checked and retained. See the [combined-search contract](architecture/combined-improvement.md) for budget and incumbent semantics.

## Fast Planner, trainer and repair

Fast Planner selects a route per choice, constructs a dependency-compatible dispatch order, chooses eligible execution modes and places phases and conditional graphs using indexed occupancy profiles. It scores and independently validates the complete result before returning it.

Trainer now evolves per-stage Q weight maps, normalization, unpinned route choices and conditional-resource modes with mutation, crossover and elitism. Selection can use weighted priority levels or Pareto fronts/crowding. Fast Planner Plus maintains an alternative-prefix tree with UCT selection, parallel complete rollouts and revisits. Read [the detailed search contract](search-and-parity.md) for limits, defaults, gene operators, proxy mapping and reproducibility. Both return replayable options and search evidence; metrics always come from complete validated schedules.

Fast Planner, Trainer, Plus and direct GA enforce the same hard constraints. Trainer searches for better candidates; it does not provide a stronger correctness certificate than the independent validator already used by Fast Planner. Time budgets are checked between complete evaluations, so one evaluation can exceed the remaining budget.

Repair currently reconstructs candidates in full under the changed model and locks. It is not incremental delta simulation. Construction can miss feasible or better plans because of route/mode choice, dispatch order, tight maximum lags, resource retention or material allocation. A failed search is not proof of mathematical infeasibility.

## Customization

[The synthetic knowledge bundle](../customizations/dummy_customer/KNOWLEDGE.md) documents the development contract. Existing typed attribute objectives create both an evaluation metric and a dispatch signal. Native `Customization` hooks cover model lowering, complete resource-sequence context, activity/penalty decoration, dispatch ranking, objective metrics and hard validation. Independent validation regenerates decorations and checks custom scores.

The linked `dummy_customer@1` example is selected by ID/version. New native logic requires source changes, registration, semantic/negative/corruption tests and a rebuild. Markdown captures requirements and counterexamples; it is not interpreted as a hard rule. Arbitrary objectives do not automatically yield effective heuristics without implementation and quality evaluation. A coding agent can perform that development workflow in the customer's checkout.

Mandatory candidate filtering is implemented in the shared dispatch path. Exact campaign/idle/urgency policies inspect prospective placements and retain replayable explanations. See the [current language contract](architecture/declarative-scheduling.md); the earlier audit describes the pre-0.4 gap.

## Persistence and deployment boundaries

The `.apex` store uses a process-shared filesystem lock, temporary writes and atomic replacement. Scenario patches require `expected_revision`; failed multi-patch requests do not partially update state. Forks are independent. Saved schedules pin their input/revision, so later edits do not alter historical validation. Chunk IDs/content hashes support idempotent retries and persisted imports can resume after restart.

The service remains synchronous, single-writer and filesystem-based, with whole-state loading and an input copy per saved schedule. It has no database, per-user identity, tenant isolation, asynchronous cancellation or crash-injection coverage. Remote HTTP requires configured authentication and network deployment; hosted agents cannot reach another computer's loopback address.

No external solver is connected: `solver.solve` returns `UNSUPPORTED_BACKEND`. A complete solver export/import contract is future work. Automatic generation of missing production orders, dynamic named-worker replacement within one phase, arbitrary preemption, simultaneous production batches, unrestricted cross-task internal activity graphs and stochastic simulation are not implemented. Optional route-aware allocation of existing material supply is available through `material.prepare`; see [material preparation](material-dispatch.md) for its limits.

## Verification

The [migration audit](migration-audit.md) records the selected synthetic v2 comparisons, intentional corrections and remaining migration work. The Rust semantic and regression tests remain runnable; the removed v2 execution harnesses do not. Historical comparisons do not establish complete parity or a speedup over v2.

After `npm ci`, run `npm run test:mcp`, `npm run test:http` and, with a server running, `npm run test:viewer`. `node tests/mcp-smoke.mjs --large` exercises 100,000-task import. On other systems install a Playwright Chromium browser or set `APEX_BROWSER`. Benchmarks must use a freshly built release executable and describe their data complexity.

## Chat-led viewer and material preparation

The default browser is a lean plan viewer. Scenario and search controls remain in the optional advanced workbench. Three repository skills define data intake, planning and implementation roles; see [agent workflows](agent-workflows.md). The `material.prepare` tool allocates existing supply into a new scenario, preserving free workplans and differing-material main modes and reporting actual allocations per saved schedule. Its limits and differences from v2 are explicit in [material preparation](material-dispatch.md).
