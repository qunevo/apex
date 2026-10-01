# Northstar factory demo

A deliberately fictional valve factory with a small MES web application and a separate Excel production plan. All names, orders, people, quantities and times are synthetic.

## Start

Requires Python 3.10 or newer. The MES uses only the Python standard library; no package installation, scheduler build, database service or account is required.

From the repository root:

```bash
bash demo/scripts/start.sh
```

Or run `python -B -m demo.mes.server`. Open **http://127.0.0.1:8788**. Use `--port 8789` for a different local port. The server binds to loopback only. Stop it with Ctrl+C.

The first start generates the synthetic data and conventional planning baseline, then creates `demo/.local/mes.sqlite`. Initial generation can take about a minute depending on the host. Later starts reuse the local database. The reviewed [Excel baseline](planning/production-planning.xlsx) is copied to the local working directory and can be downloaded from **Excel planning** in the MES.

## Explore

1. Open **Production**. Search and filter 120 customer orders, 600 lots and 3,800 operations. Click an order, then **View production lots** to follow its material flow.
2. Open **Excel planning** and download the actual `.xlsx` file. Review the dispatch sequence, machine/person assignments, fixed decisions and qualification matrix.
3. Add an urgent order. Its lots and work instructions are created together. Existing Excel assignments are intentionally unchanged.
4. Edit an inbound delivery, record machine downtime or mark a person absent. A workbook review notice appears after MES changes.
5. Open an operation and **Record progress**. Book cumulative good quantity on an eligible machine. Predecessors must be complete. Lot and order states update with it.
6. Use **Reset demo**, entering the displayed confirmation text, to restore the original MES and local workbook copy. Separately downloaded Excel files are unaffected.

For equipment, **Schedule unavailability** records a required start, end and reason. Open an equipment row to review, edit or cancel its periods. Its status is **Available** or **Unavailable** at the fixed factory snapshot, with the active or next block shown alongside it. Intervals include their start and exclude their end; overlapping or adjoining periods form one continuous block. Maintenance is a reason, not a third status. Use **Permanently unavailable** only for equipment out of service indefinitely; clearing it leaves dated periods intact. These exceptions describe equipment blocks, independently of shift working hours.

The **Factory guide** explains the products and source ownership. The detailed [factory description](factory.md) defines the synthetic assumptions and baseline limits.

## Source ownership

| Source | Owns |
| --- | --- |
| MES | Orders, articles, released work instructions, equipment, attendance, shift calendars, material supply, downtime and booked progress |
| Excel | Proposed operation sequence, workplace/person assignments, planned starts, setup/run allowances, fixed decisions, planner notes and qualifications |
| APEX | Future integration: executable scheduling model and validated alternative plans |

The browser workbook views are previews of the original seed, not a live spreadsheet editor. Excel changes are not imported automatically. A changed MES does not silently overwrite the planner's file. Item master edits apply to future orders; existing released work instructions remain snapshots. Material figures are **opening balances at the planning-period start**, not live stock accounting.

## Repository and local state

- `data/factory.json`: fixed clock, reproducible seed and dataset identity.
- `mes/`: web application, HTTP API, SQLite persistence, generation and tests.
- `planning/production-planning.xlsx`: intentionally versioned, entirely synthetic source fixture representing the planner's starting workbook.
- `planning/build-workbook.mjs`: maintainer authoring recipe using `@oai/artifact-tool` from the Codex bundled runtime. It reads `.local/seed.json`; that runtime is not required to run the MES or open the supplied Excel file.
- `.local/`: ignored database, seed cache, working Excel copy, previews and test outputs.
- `scripts/start.sh`: portable launcher, including Git Bash on Windows.

No parent dependency is added to `app/`. This demo does not require or change the APEX executable, the bundled technical examples or frozen paper evidence. The APEX adapter/customization is a subsequent integration step.

To regenerate after changing seed code, stop the server, run `python -B -m demo.mes.seed`, rebuild the source workbook with the authoring runtime, and then use **Reset demo** after restarting. This is an intentional maintainer operation; do not reset someone else's active demo.

## API

The same loopback API backs the browser and a future adapter:

| Method and path | Contract |
| --- | --- |
| `GET /api/meta` | Factory clock, field contracts, record counts and MES revision |
| `GET /api/tables/{entity}` | Paged records; `q`, `offset`, `limit` (max 200), `sort`, `direction`, exact `filter_field`/`filter_value` |
| `GET /api/tables/{entity}?format=csv` | Full filtered source export with headers |
| `POST /api/tables/{entity}` | `{ "data": { ... } }`; creates an allowed master record or an order and its lots/operations |
| `PATCH /api/tables/{entity}/{id}` | `{ "expected_version": 1, "data": { ... } }`; editable fields only |
| `POST /api/progress/{operation_id}/report` | `expected_version`, cumulative `completed_quantity`, eligible `resource_id` |
| `GET /api/plan`, `GET /api/skills` | Original Excel seed data with source and current MES revisions |
| `GET /api/audit` | Latest 50 persisted changes |
| `GET /downloads/production-planning.xlsx` | Local working copy of the source workbook |
| `POST /api/reset` | `{ "confirmation": "RESET DEMO" }`; demo-owned local state only |

Dates use `YYYY-MM-DDTHH:MM` in the named plant timezone. The October seed stays within CEST. Monetary costs and time-zone transitions are outside this initial case. Row updates reject stale versions with HTTP 409. Invalid values and references return HTTP 400. There is no deletion or automatic source-system writeback. This local mock has no multi-user authentication and is not a deployment-ready MES.

Equipment rows expose read-only `status`, `unavailable_from`, `unavailable_until` and `unavailability_reason`, calculated at `factory.as_of`. Edit `downtime` records for dated blocks (`cancelled: true` withdraws a period), or explicitly set the Boolean `permanently_unavailable`. Production booking checks availability at that same snapshot. Existing local databases are upgraded without resetting edits; legacy global Maintenance/Unavailable values become permanent exceptions because no end date was recorded. The Excel baseline remains a separate snapshot after any availability change.

## Verify

```bash
python -B -m unittest discover -s demo/mes/tests -v
node --check demo/mes/web/app.js
```

Tests cover relationships, resource and employee occupancy, calendar placement, material readiness, revisions, atomic order creation, progress rules and HTTP validation. Workbook authoring performs formula recalculation, an input-change check and per-sheet rendering; previews remain local.
