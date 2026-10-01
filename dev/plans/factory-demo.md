# Factory demo

Approved in the task: build a reviewable fictional MES and Excel planning environment.

## Scope

- Add a standalone, loopback-only Python/SQLite MES in `demo/`, with a table-based web UI and a documented JSON API.
- Seed 120 customer orders and 600 production lots for a fictional valve factory, including workplans, material, personnel, calendars, progress and a conventional planning baseline.
- Provide a real editable Excel baseline with operation sequencing, resource/person assignments, commitments, skills and source snapshots.
- Document the factory and source ownership. Reset only demo-owned local state. Do not add a scenarios directory.
- Keep APEX integration, solver changes, existing examples and paper evidence outside this implementation.

## Verification

Check seed relationships and baseline resource/precedence consistency; test API edits, stale revisions, invalid references and progress. Exercise the web app in a browser, inspect every worksheet, recalculate formulas and check the exported workbook. Verify the independent application distribution because a new root component is added. Keep generated previews, state and reports local.

## Delivery

Retain the feature branch for review, start the local preview and open the MES and workbook. No merge or publication requested.

## Implementation evidence

Implemented the source environment with 120 orders, 600 lots, 3,800 operations and 24 people. Seven integration tests pass. The complete baseline passes independent material, precedence, calendar and resource-occupancy checks. Browser acceptance covers editing, new-order lot expansion, search, filtering, pagination, lot drilldown, workbook previews and reset. All five workbook sheets were rendered and inspected; exported values and 19,026 formulas passed readback checks, including a duration-change recalculation check during authoring. Native Excel execution was not tested.

The standalone distribution check, publication guard, JavaScript syntax and launcher syntax checks pass. Local state and screenshots remain ignored. APEX model integration and Excel synchronization remain outside this delivery.

## Equipment availability follow-up

Requested: replace global Available/Maintenance/Unavailable editing with dated unavailability by default and an explicit permanent exception. Reuse the equipment downtime records, expose active/next periods at the factory snapshot, support editing/cancelling periods and derive only Available/Unavailable. Preserve existing local data during upgrade. Verify interval boundaries, overlaps, cancellation, permanent exceptions, production booking and the browser workflow. Continue the existing demo branch without creating a new worktree.

Implemented and verified: all 14 demo tests pass. Browser checks in a separate data copy cover creating a timed block, derived-status filtering, permanent exception on/off and cancellation. The live preview was upgraded without resetting business data and the equipment dialog was visually checked. Documentation and publication checks pass. The workbook and APEX integration are unchanged.
