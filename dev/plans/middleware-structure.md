# Middleware and client boundaries

> Historical implementation record. Paths and feature status below describe that
> batch, not the current product. See [current documentation](../../app/docs/architecture/README.md).

Approved in the repository conversation after integrating PR #25. Retain its
control workflow, storage contracts, PostgreSQL implementation and desktop client.

1. Group the application into `core`, `middleware`, `ui`, `customization` and
   `skills`. Put transports in `middleware/api`, orchestration in
   `middleware/control`, and persistence in `middleware/data`. Move the desktop
   into `ui/desktop`. Keep independently testable storage and API crates.
2. Make the APEX engine adapter an optional internal control module instead of
   a separate package. Preserve scheduling behavior, public compatibility paths,
   CLI commands, saved artifacts, and the existing native extension registry.
3. Share the customization manifest/configuration contract with the control
   platform. Select an enabled package when creating a scenario and retain its
   version in immutable revisions and result provenance. Reject package changes
   within a scenario and unavailable versions for new work.
4. Establish `ui/mcp-app` as the chat client. Add actual MCP UI resource discovery
   and host communication to the middleware. Read the same control results used
   by desktop, with no separate authoritative UI state. Retain the old viewer
   only as a compatibility entry point for existing `apex` links and tools.
5. Align architecture, distribution paths, CI and standalone verification.

The existing file-backed `apex` interface remains a compatibility surface; this
batch does not silently migrate user files into PostgreSQL or remove working
commands. Customer adapters stay in customization packages. A general dynamic
plugin system, live MES connector and new scheduling semantics are out of scope.

Verification: Rust formatting, Clippy, workspace tests and release builds;
configuration/version rejection and immutable-context tests; MCP resource/host
integration and existing HTTP/MCP/viewer tests; PostgreSQL conformance where
available; contributor/documentation checks and standalone application export.

Implemented on `codex/middleware-structure` after merging PR #25 into `dev`.
The empty `codex/demo-integration` branch was deleted locally and remotely.
No worktree was created. The macOS post-merge CI failure was traced to colliding
temporary fixture directories; a process-local atomic counter now distinguishes
parallel config tests in addition to their process ID and timestamp.

Local verification passed: formatting, Clippy with warnings denied, default
workspace Rust tests, release builds, official SDK tests for compatibility and
control MCP, HTTP/config/viewer tests, MCP App host simulation, 51 contributor
tests, docs/skills/wiki checks, tracked-source publication scan and a standalone
export build. No scheduling-core files changed. The desktop dependency graph
does not include `apex-engine`. Local PostgreSQL execution and native desktop
build verification remain covered by the PR's Linux PostgreSQL/macOS jobs;
Docker is not running on this workstation. A real chat-host deployment remains
separate from the tested MCP Apps protocol bridge.
