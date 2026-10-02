# Qunevo Demo MES

A deliberately fictional valve factory with a small MES web application and a separate Excel production plan. All names, orders, people, quantities and times are synthetic.

The application uses Qunevo's logo, colors and locally bundled website fonts.
[Asset sources and font licenses](mes/web/assets/SOURCES.md) are recorded with the
assets. Northstar Valve Works remains the fictional factory represented by the
MES and planning workbook.

## Start the complete showcase

Install Docker with Linux containers and Docker Compose 2.20.3 or newer. From the repository root, run the local starter in Bash (Git Bash on Windows):

```bash
bash demo/scripts/start-demo.sh
```

It builds and starts the containers in the background, waits for readiness, then opens the shared working workbook in the host's spreadsheet application and a simple English setup page in the browser. No second terminal or manual token lookup is needed. On the page, choose **Open MES** or **Copy setup instructions** for a local chat agent. The instructions include the existing MCP connection, MES address and host workbook path; the client may need to reconnect to load newly configured tools. The generated `.local/start.html` contains the agent token: keep it private. It is ignored by Git and is outside the directory shared with the containers.

The starter can be run again without resetting edits. Use `--no-build` to reuse existing images or `--no-open` for a terminal-only start. It respects Compose environment settings and `.env`, including configured host ports. If the browser or spreadsheet application cannot open automatically, it prints the paths for manual opening. The host needs Docker and Bash; no host Python or Node installation is required.

Direct Compose startup remains available from this directory, without automatically opening host applications:

```bash
docker compose up --build
```

Open **http://127.0.0.1:8788** for the MES. APEX is available at **http://127.0.0.1:8780/mcp**. Startup shows the MCP address and ready-to-copy agent authorization header. The demo enables `APEX_SHOW_ACCESS=1`; set it to `0` in a local `.env` to keep the token out of startup logs. The first build downloads images/dependencies; initial MES generation can take about a minute. `docker compose up --build -d --wait` waits for readiness in the background and leaves the terminal free.

Keep the stack running when connecting a chat. With attached logs, open a second terminal in this directory. Run `docker compose exec apex apex-container connect` to display the connection details again, or `docker compose exec apex apex-container access` for both agent and desktop viewer tokens. Multiple local demo clients can reuse the existing agent token; connecting another client needs no container restart. The demo customization is selected by the deployment configuration, not by that token.

[compose.yaml](compose.yaml) includes the normal application stack and [apex.compose.yaml](apex.compose.yaml) selects its demo customization. PostgreSQL stores APEX state in project-scoped named volumes; the MES database and editable Excel working file live in **`.local/container/`**. Open `.local/container/production-planning.xlsx` in Excel to edit the actual mounted workbook. The versioned `planning/production-planning.xlsx` remains the reset baseline. The original native `.local/` working state is separate.

The APEX container receives that same working directory at `/sources/demo` read-only and the deployment's [sources.json](sources.json) inside the demo customization's adapter directory. The MES is reachable there as `http://mes:8788`. These prepare source access; the live adapter and factory-model mapping remain unimplemented, so starting the stack does not import MES or Excel data or optimize the factory automatically.

`docker compose down` retains both database volumes and local MES/Excel files. **Reset demo** resets only MES state and its working workbook, not APEX scenarios. `docker compose down -v` deletes the Compose project's APEX database and credentials; the bind-mounted `.local/container/` is retained. See the [application container guide](../app/docs/containers.md) for connection and lifecycle details.

The standalone app and this showcase use different Compose projects but the same default APEX port. Stop one before starting the other, or set `APEX_HTTP_PORT`. Set `DEMO_MES_PORT` to change the MES host port. On native Linux, run `mkdir -p .local/container` before the first start so the host user owns the parent directories; users whose UID/GID differ from 1000 can set `DEMO_UID` and `DEMO_GID` to their host IDs for editable workbook ownership. Windows/macOS use Docker Desktop's file sharing.

The MES container listens on its internal network interface but publishes only a host loopback port. Exact allowed Host values include the internal service and configured local port; cross-origin browser requests remain rejected. Desktop remains an optional separate client.

## MES-only Python start

Requires Python 3.10 or newer. The MES uses only the Python standard library; no package installation, scheduler build, database service or account is required.

From the repository root:

```bash
bash demo/scripts/start.sh
```

Or run `python -B -m demo.mes.server`. Open **http://127.0.0.1:8788**. Use `--port 8789` for a different local port. The native start binds to loopback by default. Stop it with Ctrl+C.

The first start generates the synthetic data and conventional planning baseline, then creates `demo/.local/mes.sqlite`. Initial generation can take about a minute depending on the host. Later starts reuse the local database. The reviewed [Excel baseline](planning/production-planning.xlsx) is copied to the local working directory and can be downloaded from **Excel planning** in the MES.

## Explore

1. Open **Production**. Search and filter 120 customer orders, 600 lots and 3,800 operations. An order shows its running operations with actual machines, operators and start times. Open an operation directly, or choose **View production lots** and follow the clickable material flow.
2. Edit the shared workbook opened by the starter, or open `.local/container/production-planning.xlsx` directly, and save changes in Excel. **Excel planning > Download a copy** exports a separate file; editing that download does not change the shared workbook. Review the dispatch sequence, machine/person assignments, fixed decisions and qualification matrix.
3. Add an urgent order, select an approved workplan, enter total quantity and pieces per lot. Saving releases its lots and frozen instructions together, including a remainder lot. Existing Excel assignments are unchanged.
4. Open **Articles & workplans > Workplans**. Compare combined machining with the separate roughing/drilling route. Create a draft revision, open a step, and edit its machine alternatives, times or component requirements. **Check & release** locks that revision. Approve it under **Article workplans** before selecting it on an order.
5. Open an operation to inspect its released alternatives and components. **Record progress** captures cumulative good/scrap quantities, operator, machine and actual start/finish. A scrap reason is required. The preceding step must be complete; only its good pieces can proceed. Confirmation history and material issues remain visible in separate tables.
6. Edit an inbound delivery, schedule equipment unavailability or a personnel absence, or place a production lot on **Quality hold** with a reason. The snapshot button advances the demo clock; it does not automatically execute the plan.
7. Close the shared workbook in Excel, then use **Reset demo**, entering the displayed confirmation text, to restore the original MES and shared workbook even after prolonged use or container recreation. Reopen the workbook afterward. Do not save an old open copy over the restored file. If the workbook is open or cannot be replaced, the reset is rejected and MES changes are rolled back. Separately downloaded Excel files, APEX scenarios and access tokens are unaffected.

Use **DE / EN** in the top-right header to switch the entire interface, including the showcase, forms and validation messages. The browser remembers the selection; the initial language follows the browser language. UI templates and known synthetic display labels live in `mes/web/locales/en.js` and `de.js`. IDs, API enum values, customer/person names, custom source prose and the Excel/CSV source files stay unchanged. Language changes never write to the MES database.

Open **Filters** or a column's filter button. Text columns offer substring matching and searchable checkbox lists; priorities, statuses and locations offer multiple selections. Numeric fields accept inclusive minimum/maximum bounds. Date bounds include whole local calendar days; identical bounds select one day. Columns combine with AND, selected values within a column with OR. Filters apply before sorting and pagination, and the CSV export uses the same full result. Chips identify active filters and remove them individually. A linked order/lot scope combines with the column filters; switching tables clears the current filters.

**In progress** on an order or lot does not imply an operation is currently running. **Running on** on a lot is populated only from a running operation's execution booking. Waiting operations show no actual machine, even if the Excel baseline assigns one. Operation details expose actual machine/operator/start/finish and links back to their lot and customer order. **Confirmations** is the production data capture history (German: **Betriebsdatenerfassung**).

For equipment, **Schedule unavailability** records a required start, end and reason. Open an equipment row to review, edit or cancel its periods. Its status is **Available** or **Unavailable** at the current factory snapshot, with the active or next block shown alongside it. Intervals include their start and exclude their end; overlapping or adjoining periods form one continuous block. Maintenance is a reason, not a third status. Use **Permanently unavailable** only for equipment out of service indefinitely; clearing it leaves dated periods intact. These exceptions describe equipment blocks, independently of shift working hours.

The **Showcase** area presents the factory in six chapters with a five-minute speaking budget: factory, products, material flow, decisions, Monday handover and the APEX handoff. Use **Present** to hide MES navigation, **Previous / Next** or the arrow keys to move through chapters, and **Escape** to exit the presentation layout. Chapters have direct URLs such as `#showcase/flow`; the former `#guide` link opens the introduction. Links into production, equipment and Excel return to ordinary MES use, and **Showcase** resumes the last chapter. The presentation describes the initial synthetic baseline, independently of later MES edits, and identifies APEX integration as future work. The detailed [factory description](factory.md) defines the synthetic assumptions and baseline limits.

## Source ownership

| Source | Owns |
| --- | --- |
| MES | Orders, articles, released work instructions, equipment, attendance, shift calendars, material supply, downtime and booked progress |
| Excel | Proposed operation sequence, workplace/person assignments, planned starts, setup/run allowances, fixed decisions, planner notes and qualifications |
| APEX | Future integration: executable scheduling model and validated alternative plans |

The browser workbook views are previews of the original seed, not a live spreadsheet editor. Excel changes are not imported automatically. A changed MES does not silently overwrite the planner's file. Item master edits apply to future orders; existing released work instructions remain snapshots. **On hand** equals opening stock plus received deliveries available at the snapshot, minus posted component issues. Confirmed or delayed deliveries are future supply, not usable stock. Confirmation consumes components for newly processed input pieces, including scrap. It never consumes the same pieces twice.

## Repository and local state

- `data/factory.json`: initial clock, reproducible seed and dataset identity.
- `mes/`: web application, HTTP API, SQLite persistence, generation and tests.
- `planning/production-planning.xlsx`: intentionally versioned, entirely synthetic source fixture representing the planner's starting workbook.
- `planning/build-workbook.mjs`: maintainer authoring recipe using `@oai/artifact-tool` from the Codex bundled runtime. It reads `.local/seed.json`; that runtime is not required to run the MES or open the supplied Excel file.
- `.local/`: ignored database, seed cache, working Excel copy, previews and test outputs.
- `scripts/start.sh`: portable launcher, including Git Bash on Windows.
- `scripts/start-demo.sh`: complete Compose startup with a private local onboarding page and shared workbook opening.

No parent dependency is added to `app/`. The container showcase composes the existing APEX server; the MES-only Python start still runs independently. Frozen paper evidence is unchanged. The APEX package boundary is in `app/customization/demo/`; the live adapter remains a subsequent integration step.

To regenerate after changing seed code, stop the server, run `python -B -m demo.mes.seed`, rebuild the source workbook with the authoring runtime, and then use **Reset demo** after restarting. This is an intentional maintainer operation; do not reset someone else's active demo.

## API

The same API backs the browser and a future adapter; Compose also makes it available on the private container network:

| Method and path | Contract |
| --- | --- |
| `GET /api/meta` | Factory clock, field contracts, record counts and MES revision |
| `GET /api/tables/{entity}` | Paged records; `q`, `offset`, `limit` (max 200), `sort`, `direction`, exact `filter_field`/`filter_value`, JSON `filters`, optional `facets=1` |
| `GET /api/tables/{entity}?format=csv` | Full filtered source export with headers |
| `POST /api/tables/{entity}` | `{ "data": { ... } }`; creates an allowed master record or an order and its lots/operations |
| `PATCH /api/tables/{entity}/{id}` | `{ "expected_version": 1, "data": { ... } }`; editable fields only |
| `POST /api/progress/{operation_id}/report` | `expected_version`, cumulative `completed_quantity` and `scrap_quantity`, released `resource_id`, qualified `person_id`, `actual_start`, `actual_end` (only when complete), `scrap_reason` |
| `POST /api/workplans/{id}/revise` | `expected_version`, new `id` (max 24 characters), `revision`, optional `name`; copies all child rows into a draft |
| `POST /api/workplans/{id}/release` | `expected_version`; validates the complete draft, then locks it |
| `POST /api/clock` | `expected_as_of`, later `as_of`; advances the local demo snapshot |
| `GET /api/plan`, `GET /api/skills` | Original Excel seed data with source and current MES revisions |
| `GET /api/audit` | Latest 50 persisted changes |
| `GET /downloads/production-planning.xlsx` | Local working copy of the source workbook |
| `POST /api/reset` | `{ "confirmation": "RESET DEMO" }`; restores MES and shared workbook, returns `reset` and `workbook_reset`; HTTP 409 when Excel is open or the workbook cannot be restored |

`filters` is an object keyed by a non-JSON catalog field. Text fields accept `{"contains":"OEM", "values":["OEM 01","OEM 02"]}`; numeric fields accept `{"min":25,"max":100}`; datetime fields accept calendar dates such as `{"min":"2026-10-07","max":"2026-10-07"}`. Empty bounds and an empty selection impose no restriction. Unknown fields/operators, invalid dates, non-finite numeric bounds and reversed ranges return HTTP 400. `facets=1` returns distinct values across the complete parent scope, independently of pagination and active column filters. Operation rows expose a computed `order_id`; lot rows expose computed `current_operation` and `resource_id` (running only). These read-only projections do not change stored records or revisions.

Dates use `YYYY-MM-DDTHH:MM` in the named plant timezone. The October seed stays within CEST. Monetary costs and time-zone transitions are outside this initial case. Row updates reject stale versions with HTTP 409. Invalid values and references return HTTP 400. There is no deletion or automatic source-system writeback. This local mock has no multi-user authentication and is not a deployment-ready MES.

Equipment rows expose read-only `status`, `unavailable_from`, `unavailable_until` and `unavailability_reason`, calculated at `factory.as_of`. Edit `downtime` records for dated blocks (`cancelled: true` withdraws a period), or explicitly set the Boolean `permanently_unavailable`. Production booking checks actual execution intervals against equipment calendars, dated blocks, operator shifts/breaks, absences, the original Excel qualification snapshot and existing execution bookings. Existing local databases are upgraded without resetting edits; legacy global Maintenance/Unavailable values become permanent exceptions because no end date was recorded. The Excel baseline remains a separate snapshot after any availability change.

## Verify

```bash
python -B -m unittest discover -s demo/mes/tests -v
node --check demo/mes/web/app.js
node --check demo/mes/web/details.js
node --check demo/mes/web/showcase.js
node --check demo/mes/web/filters.js
node --check demo/mes/web/production.js
node demo/mes/tests/i18n.mjs
```

Tests cover relationships, resource and employee occupancy, calendar placement, material readiness, revisions, atomic order creation, progress rules, composed filters, date boundaries, CSV/pagination parity, actual-machine projections and HTTP validation. Locale checks cover template parity, parameters, API error translation and unchanged source values. Workbook authoring performs formula recalculation, an input-change check and per-sheet rendering; previews remain local.

## Workplan and execution contracts

- A workplan ID identifies one revision. Draft child rows are editable; release locks the header, steps, machine alternatives and components. Use a new draft revision for changes. Inactive rows stay visible; records are not physically deleted.
- Step numbers define a linear, whole-lot sequence. Machine alternatives are explicit per step, body size and body material. Setup and unit times may differ by machine. The first alternative supplies the default displayed lot allowance; all alternatives and their rates remain available in the operation snapshot. The Excel baseline retains its original rounded allowances.
- An article can approve multiple released revisions. A new article automatically approves its default route. Alternative links must match its family and cover every step for its size/material. The order selects one route for all its lots. No mid-production rerouting occurs.
- Each released operation freezes its instructions, output description, eligible machines, times, attendance and component quantities. Changes to article or workplan masters apply to future releases only.
- A confirmation reports cumulative quantities, with append-only deltas in the confirmation and issue tables. Good plus scrap cannot exceed input. If all input is scrapped, following operations are marked **Skipped**; the lot closes with zero finished good pieces. **Complete** means execution is accounted for, not that the original shipping quantity was achieved. Lot/order good and scrap quantities expose that difference. Replacement demand is an explicit new order.
- Actual start, machine and operator are fixed after the first timed confirmation. Finish is required when all input is accounted for and must not precede an earlier open confirmation. Actual execution fits one equipment shift window; operator attendance fits one shift segment. CNC attendance covers setup only. Pauses, cross-shift execution and operator handovers are not modeled.
- A quality hold blocks new confirmations; releasing it is a recorded master edit. Scrap is recorded with a reason. Test measurements, rework routes, formal inspection dispositions and serial/batch genealogy are outside this demo.
- Stock checks are transactional and include historical issue times. Receipt or opening-stock edits cannot invalidate already posted issues. Material reservations, warehouse transfers, finished-goods inventory and issue reversals are not implemented.
- Qualifications remain owned by Excel. The MES validates against the original supplied qualification snapshot; editing a downloaded workbook does not update it. Newly added people therefore have no imported qualifications yet.

Existing local databases receive an idempotent upgrade without reset. Original seeded releases recover article attributes from the seed; other legacy releases use the then-current article because earlier snapshots were not stored. Legacy bookings retain quantities; actual times are filled only when the booking matches the known synthetic baseline. Otherwise the confirmation identifies missing historical execution times. The SQLite database and audit history remain local.
