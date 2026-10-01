# Application packages and customization selection

Approved scope: restructure the application into core, server, data, UI, skills
and customization packages on the existing demo feature branch and PR.

1. Move cohesive algorithm modules to `app/core`, server entry points and tool
   orchestration to `app/server`, and the existing viewer to `app/ui`. Preserve
   the public Rust re-exports, executable names, algorithms and input schemas.
2. Extract file persistence and stored records to `app/data`. Retain revision
   checks and immutable saved inputs. Active-plan lifecycle remains future work.
3. Group each synthetic customization under `app/customization/<id>`, with
   adapter, model, skills, tests and UI responsibilities. Keep the demo source
   systems outside the independent application.
4. Add explicit JSON server configuration: package root, enabled folder IDs and
   a default. Validate manifests at startup; route an optional API
   `customization` argument to a separate artifact store and input workspace.
   Pin package identity/version in stored records and viewer links. Do not
   dynamically execute package code or change native model selection semantics.
5. Keep unconfigured CLI/MCP/HTTP behavior and old flat stores compatible.
   Update deployment, architecture, links and publication checks.

Acceptance: Rust fmt, Clippy, full tests and release build; configuration and
cross-package negative tests; MCP/HTTP and viewer-context checks; documentation
and contributor checks; detached standalone application verification. No new
UI, live MES adapter, native rule, database backend or automatic writeback is
part of this structural batch.

Implemented on `codex/demo`, PR #24. All 25 algorithm source modules retain
identical content apart from filesystem line endings. Package configuration is
explicit (`--config` or `APEX_CONFIG`); unconfigured stores remain compatible.

Validation: 146 Rust tests passed (two report generators remain intentionally
ignored), fmt/Clippy passed, rebuilt release passed MCP/HTTP and configured
viewer integration, 51 contributor tests passed, source/wiki/publication checks
passed, and the detached application export built and validated all five
planning methods plus three additional fixtures. The local integration binary
was built under `.apex/package-build` because running processes held the default
Windows executable open. No running deployment was replaced.
