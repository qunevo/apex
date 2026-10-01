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

## Practical MES follow-up

Approved by the user: extend the demo into a realistic, approachable source MES. Continue the existing branch and preserve local business records.

- Introduce draft/released workplan revisions, editable steps, explicit machine alternatives with setup and unit times, and material requirements at the consuming step. Link multiple released workplans to an article and select the workplan when releasing an order.
- Seed a second route with separate rough machining and finish drilling, alongside the existing combined machining route. Retain all original order, lot and operation IDs and the Excel baseline.
- Freeze machine alternatives, requirements and instructions in released operations. Master-data edits must never change the eligibility of a released lot.
- Record cumulative good/scrap quantities, actual times, operator and scrap reason; propagate usable quantities through whole-lot precedence. Show confirmations and material issues, with received supply and nonnegative stock checks. Keep the qualification source in Excel.
- Add dated personnel absences and a controllable demo clock. Retain timed equipment unavailability and the explicit permanent exception.
- Verify revision locking, invalid route release, immutable snapshots, quantity conservation, stock, temporal/resource constraints, rollback, upgrade preservation and browser workflows in a separate data copy.

This remains a single-plant demo without purchasing, costing, serial genealogy, automatic rework routing, electronic signatures, ERP interfaces or Excel/APEX synchronization. No scheduling-core or distribution-boundary change is intended.

Implemented: six released workplans, 41 master steps, 900 machine/configuration alternatives, 24 step-component rows and 36 article approvals. The original 120 orders, 600 lots and 3,800 operation IDs are retained. Released operations contain their own eligibility, rate and component snapshots. Confirmations now cover good/scrap, actual execution, qualified operators, quality holds and automatic component issues; shift, absence, equipment and historical stock checks are transactional.

Verification: all 29 demo tests pass, including negative cases and atomic rollback. JavaScript syntax, maintained documentation and publication checks pass. Browser acceptance in a separate database copy covers copying and editing a workplan, releasing the revision, creating a 43-piece order on the alternative route (three lots), and confirming 24 good pieces plus one scrap piece on a running 25-piece lot. The live SQLite database was backed up and upgraded; comparison confirms all original business fields, identifiers, audit rows and MES revision are preserved. The live detail view was visually checked. Excel remains the unchanged initial planning fixture, and the Rust application is unaffected.

## Qunevo branding and presentation

Requested and name confirmed by the user: rename the application to **Qunevo Demo MES** and align its appearance with qunevo.com. Use the original Qunevo mark, blue/violet/cyan brand colors, locally bundled website fonts with their licenses, clearer table contrast, consistent form spacing and responsive layouts. Keep Northstar as the fictional factory and preserve all business data. Check the document title, local asset responses, production tables, dialogs and compact layouts in the browser. Continue the existing feature branch; no new worktree or integration is requested.

Implemented the name in the page title, application header and dialogs, with the original SVG mark as the sidebar logo and favicon. Refined navigation, table density, status contrast, form spacing and short-window navigation. Bundled Outfit and Bricolage Grotesque with their upstream font licenses and asset provenance. Verification: JavaScript syntax, maintained documentation and publication checks pass; the live HTML, SVG and both fonts return HTTP 200 with the expected content types and byte-identical local assets. Desktop production lots, a lot detail dialog and a 600-pixel compact layout were inspected in the browser; the temporary viewport override was reset. The live database SHA-256 is unchanged. No scheduling or business logic changed.
