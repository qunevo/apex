# Control platform

The control platform is the shared application layer for teams and hosted use. It
keeps planning scenarios, runs, results and their approval in one place and exposes
the same typed operations over HTTP and MCP. Agents, scripts and other clients
change state through these operations; display clients such as the desktop
application follow the resulting changes. The engine-only `apex` executable
described in the [operating guide](implementation.md) is unchanged and remains
usable on its own.

## Crates

| Crate | Directory | Responsibility |
| --- | --- | --- |
| `apex-engine` | `core/` | Scheduling model, algorithms, evaluation and independent validation. No I/O. |
| `apex-control` | `middleware/control/` | Tenants, roles, scenarios, immutable revisions, runs, approval and publication. Defines `Store` and `EngineAdapter`; no database or transport dependency. The optional `apex` feature supplies the built-in engine implementation in `src/apex.rs`. |
| `apex-control-postgres` | `middleware/data/` | PostgreSQL store with embedded migrations, tenant row-level security and a worker claim function. `files.rs` retains the separate compatibility file store. |
| `apex-control-server` | `middleware/api/` | `apex-control` executable: HTTP, MCP, UI resources, events and background workers. The existing `apex` CLI/transport also lives here and retains its public commands. |
| MCP App assets | `ui/mcp-app/` | Read-only chat view served as an MCP UI resource; `compat.html` preserves old file-backed viewer links. |
| `apex-desktop` | `ui/desktop/` | GPUI Kit display client. Excluded from default workspace members; build it with `cargo build -p apex-desktop`. |

## Concepts

- **Scenario and revision.** A scenario names a planning problem for one engine. Every
  edit appends an immutable revision containing facts (the engine's model, e.g. an
  `apex.v3.4` problem) and planning intent (declarations). Revising requires the
  current `expected_revision`; a stale value fails with `CONFLICT`.
- **Planning intent.** Declarations are kept separate from imported facts. The engine
  adapter lists the declaration kinds it can represent; others are rejected with
  `UNSUPPORTED_DECLARATION` rather than ignored. The APEX adapter currently supports
  none; commitments are expressed in the facts.
- **Run.** A background optimization of one revision. Edits made while it runs do
  not change its input. Workers claim runs under a lease; a run abandoned by a
  crashed worker is retried after the lease expires and fails after repeated
  attempts. Each tenant has a limit on concurrently running runs.
- **Result.** Only a schedule that passes the engine's independent validation becomes
  a result. It records provenance: engine and extension versions, options, seed and
  the revision's content hash. It carries an engine-neutral schedule view for Gantt
  charts besides the native schedule.
- **Approval and publication.** Approving requires the approver role; planners cannot
  approve their own runs unless they also hold it. Publishing requires an approved
  result of the scenario's current revision; a result of a superseded revision
  cannot be published. A newly published result supersedes the previous one.
- **Roles.** `viewer` reads; `planner` edits scenarios and starts or cancels runs;
  `approver` approves, rejects and publishes; `admin` may do everything.

## Start with containers

From the standalone application directory, run `docker compose up --build`.
Initialization, database roles and migrations are automatic; state and initial
access tokens survive normal shutdown. Follow the [container guide](containers.md).
No source system or customization is activated by the base Compose file.

## Run locally

```text
cargo build --release
target/release/apex-control init
target/release/apex-control serve
```

`init` writes `.apex-control/auth.json` with one tenant and prints two tokens once: an
admin token for agents and a viewer token for display clients. Only token hashes are
stored. Add tokens with `apex-control token --actor NAME --roles planner,approver`.

Without a database the server uses an in-memory store and loses its state on exit.
For durable state, start PostgreSQL and migrate as the owning role:

```text
docker run -d --name apex-postgres -e POSTGRES_PASSWORD=apex -p 5432:5432 postgres:17-alpine
target/release/apex-control migrate --database postgres://postgres:apex@127.0.0.1:5432/postgres
target/release/apex-control serve --database postgres://postgres:apex@127.0.0.1:5432/postgres
```

Settings can also come from `APEX_CONTROL_AUTH`, `APEX_CONTROL_DATABASE_URL`,
`APEX_CONTROL_BIND` (default `127.0.0.1:8780`) and `APEX_CONTROL_WORKERS` (default 2).
`--config FILE` or `APEX_CONFIG` enables customization packages using the same
[manifest contract](server-configuration.md) as the compatibility CLI.

Select a package by optional top-level `customization` on `scenarios.create`
(or `POST /v1/scenarios`); omission uses the configured default. The server pins
the package ID and version in revision content, its hash and result provenance.
Revisions keep that package even when the server default changes. A new package
or version requires a new scenario. New revisions, runs and worker claims reject
an unavailable package version; historical reads and result decisions remain
available under the same tenant permissions. Configuration is loaded at startup.
Without configuration, explicit package selection is rejected and ordinary
unconfigured scenarios retain their previous behavior.

Tenant isolation is enforced by every operation and, in PostgreSQL, by forced
row-level security. Row-level security is effective when the server connects as a
role without `BYPASSRLS` that does not own the tables: create such a role, run
`apex-control migrate --app-role ROLE` with the owning role, and serve with the
application role's URL. The worker claim function runs as its owner and is the only
cross-tenant query.

## Agents (MCP)

`POST /mcp` serves stateless MCP over streamable HTTP with bearer authentication.
For example, in a Codex configuration:

```toml
[mcp_servers.apex-control]
url = "http://127.0.0.1:8780/mcp"
bearer_token_env_var = "APEX_CONTROL_TOKEN"
```

Tools: `engines.list`, `scenarios.list|get|create|revise`, `revisions.get`,
`runs.start|list|get|cancel`, `results.list|get|approve|reject|publish`. For the APEX
engine, run options are `{"method": "create|hypersearch|treesearch|evolve|improve",
"options": <apex Options>}`. Without a strategy, the queue policy is used, matching
the engine-only tools.

## HTTP

| Method and path | Operation |
| --- | --- |
| `GET /v1/engines` | `engines.list` |
| `GET`, `POST /v1/scenarios` | `scenarios.list`, `scenarios.create` |
| `GET /v1/scenarios/{id}` | `scenarios.get` |
| `POST /v1/scenarios/{id}/revisions` | `scenarios.revise` |
| `GET /v1/scenarios/{id}/revisions/{number}` | `revisions.get` |
| `GET`, `POST /v1/scenarios/{id}/runs` | `runs.list`, `runs.start` |
| `GET /v1/scenarios/{id}/results` | `results.list` |
| `GET /v1/runs/{id}`, `POST /v1/runs/{id}/cancel` | `runs.get`, `runs.cancel` |
| `GET /v1/results/{id}[?include_schedule=true]` | `results.get` |
| `POST /v1/results/{id}/approve\|reject\|publish` | result decisions |
| `GET /v1/events` | Server-sent events for the caller's tenant |

Errors use `{code, message, diagnostics}` with `UNAUTHORIZED` (401), `FORBIDDEN`
(403), `NOT_FOUND` (404), `CONFLICT` (409), `INVALID` (422) and `STORE` (500).

Events name what changed (`scenario`, `run` or `result`, with IDs); clients then read
current state through the API. A `resync` event means the client missed events.

## MCP App

`results.get` advertises `ui://apex/plan.html` via `_meta.ui.resourceUri`. The
authenticated MCP endpoint implements `resources/list` and `resources/read` with
`text/html;profile=mcp-app`. The embedded page receives tool results, renders
resource lanes, operation filters, metrics, validation and provenance, and can
refresh the same result through the host's `tools/call` bridge. The page stores
no tokens and makes no direct network requests. Hosts without MCP Apps support
still receive normal text and structured tool results.

The bridge follows the [MCP Apps protocol](https://modelcontextprotocol.io/extensions/apps/overview).
Protocol behavior is tested in an opaque browser sandbox with a simulated host;
individual chat host integrations still require deployment verification.
The view uses the same `ScheduleView` as desktop; it is an overview of main
operations, not the full activity/reservation model. The compatibility viewer
keeps its detailed activity views until these are exposed by the control view.

## Desktop display client

```text
cargo build --release -p apex-desktop
APEX_CONTROL_URL=http://127.0.0.1:8780 APEX_CONTROL_TOKEN=<viewer token> target/release/apex-desktop
```

The window shows the selected scenario's published plan (otherwise its newest valid
proposal) as a Gantt chart, plan KPIs, and recent runs and results. It follows
`/v1/events` and changes no planning state. German or English follows the system
locale or `APEX_LANG` and can be switched in the window. On macOS, shaders are
compiled at runtime, so Xcode is not required.

## Current limits

- Events are distributed within one server process. Several server instances need a
  shared channel, such as PostgreSQL `LISTEN/NOTIFY`, before they share subscribers.
- The APEX engine cannot be interrupted during a computation. Cancelling a running
  run takes effect when the computation returns; `budget_ms` bounds search time.
- Planning intent is not yet compiled into engine input, and production templates
  must be expanded into a canonical problem before they are used as facts.
- Authentication uses static bearer tokens from a file. There is no login provider,
  SaaS provisioning or connector to external systems yet.
- The tenant concurrency limit is checked when a run is claimed; simultaneous
  claims by several workers can exceed it briefly.
