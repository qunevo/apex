# Cargo workspace and initial control platform

Approved scope: turn `app/` into a Cargo workspace with independently usable
engine, control platform and desktop UI crates. The existing `apex` executable
remains the engine-only product (CLI, MCP, HTTP, viewer) with unchanged behavior.

1. `app/Cargo.toml` becomes the workspace root and keeps the `apex-scheduler`
   package (`server/`, `data/`, `ui/`, `customization/`). `app/core` becomes the
   `apex-engine` crate without I/O dependencies. The root library re-exports it so
   existing `apex::…` paths keep working.
2. `app/control` (`apex-control`): tenants, actors and roles, scenarios with
   immutable revisions and optimistic checks, planning intent, runs, result
   provenance, approval and publication. It defines `Store` and `EngineAdapter`
   and has no database, HTTP or engine dependency. Includes an in-memory store.
3. `app/engine-adapter` (`apex-engine-adapter`): `EngineAdapter` for
   `apex-engine` with capabilities, preparation diagnostics, run, independent
   validation and a common schedule view.
4. `app/control-postgres` (`apex-control-postgres`): PostgreSQL store, embedded
   migrations, tenant ownership with forced row-level security and a job table
   claimed with `FOR UPDATE SKIP LOCKED` through a narrow claim function.
5. `app/control-server` (`apex-control-server`, executable `apex-control`): HTTP
   API over the control operations, bounded background workers and static
   token-to-actor authentication read from the environment.
6. `app/desktop` (`apex-desktop`): minimal GPUI Kit client that lists scenarios,
   runs and results through the HTTP API. It is excluded from default workspace
   members, so ordinary builds and CI do not require GPUI platform dependencies.

Acceptance: fmt, Clippy, full workspace tests and release build; existing
MCP/HTTP/viewer integration and the standalone check unchanged in behavior;
control-platform tests for stale revisions, immutable run snapshots, provenance,
approval before publication, refusal to publish results of superseded revisions,
and cross-tenant isolation in both the application and PostgreSQL RLS. PostgreSQL
tests run when `APEX_TEST_DATABASE_URL` is set.

Added on request during implementation: an MCP endpoint and server-sent change
events, so local agents operate the platform and the desktop client only displays
and follows it; a Gantt-first desktop layout with German and English.

Out of scope: a control-platform CLI, connectors, a login provider, compiling
planning intent into engine input (preparation reports unsupported declarations),
browser/WASM UI and Gantt editing.
