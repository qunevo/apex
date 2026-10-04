# Demo MES and Excel adapter

This optional Python adapter reads the public MES API and the actual saved Excel
working file. It produces the current canonical APEX `Problem` JSON. All source
extraction, workbook parsing and factory mapping live in this package. It has no
imports from the parent development repository and changes no scheduling core,
MES records or workbook cells. The shared HTTP server exposes a configurable
request-size limit so deployments can admit complete factory snapshots.

## Run

Requires Python 3.10 or newer and the package's optional dependencies. From the
standalone application directory:

```bash
python -m pip install -r customization/demo/adapter/requirements.txt
python -B customization/demo/adapter/import_demo.py \
  --mes-url http://127.0.0.1:8788 \
  --workbook /path/to/production-planning.xlsx \
  --output .apex/demo-import-001 \
  --apex target/release/apex
```

Use `apex.exe` on Windows. `--apex` is optional and runs the existing canonical
input/readiness checks in a disposable workspace. It does not schedule or change
the central server. Without it, the report explicitly says `engine_input:
not_run`; successful mapping alone is not a feasibility or validation claim.

Alternatively pass `--config /path/to/sources.json` containing `mes_url` and
`planning_workbook`. Relative workbook paths resolve against that config file.
Explicit flags or `DEMO_MES_URL` / `DEMO_PLANNING_WORKBOOK` override config values.
The command works from any working directory. Each export requires a **new output
directory** so earlier snapshots remain intact. A failed import leaves no new
output directory and prints a structured diagnostic to stderr.

The repository showcase supplies a separate opt-in `adapter` Compose profile.
From its `demo/` directory, after starting the MES:

```bash
mkdir -p .local/adapter
docker compose --profile adapter run --build --rm adapter \
  --config /config/sources.json --output /outputs/import-001
```

The host output is `.local/adapter/import-001`. The one-shot adapter receives the
saved workbook read-only and connects to `http://mes:8788`. Ordinary demo startup
does not import automatically. The standalone APEX image needs no Python runtime.

## Outputs and middleware handoff

| File | Purpose |
| --- | --- |
| `problem.json` | Canonical scheduling input; accepted by the existing APEX CLI and as control `content.facts` |
| `scenario.json` | Request body for `POST /v1/scenarios` / `scenarios.create`, selecting the `demo` package |
| `report.json` | Source hashes/revision, assumptions, counts, warnings, closed history and Excel proposals |
| `mes-snapshot.json` | Exact extracted source records, including confirmations and material issues |
| `planning-source.xlsx` | Exact saved workbook bytes used by this import |

`scenario.json` also contains bounded `content.source_summary`: source fingerprints,
record counts, remaining operations without Excel proposals and sampled notices.
It is separate from engine facts and enables the [Source details](../ui/README.md)
dashboard through `views.get` immediately after import. Full source records and
the full report stay in this output directory. Evidence is adapter-reported and
must be refreshed with a new import when the source changes.

All outputs are runtime data: keep them in `.apex` or another ignored/private
directory. No URL credentials are supported or copied into provenance. Source
hashes and the problem hash identify the evidence, not separate schema versions.

To create a scenario, submit `scenario.json` through the existing authenticated
HTTP/MCP interface. To refresh an existing scenario, use `scenarios.revise` with
its current `expected_revision` and `content.facts` from the new `problem.json`.
Then start a run explicitly. This command itself performs no server mutations,
optimization, approval, publication or MES/Excel writeback.

JSON is written compactly. The current factory expands more than 200,000
machine/person modes and its canonical input is approximately 74 MB. The shared
server retains its 2 MiB default; the demo override sets
`APEX_CONTROL_MAX_REQUEST_BYTES=134217728` (128 MiB). Configure that setting or
`--max-request-bytes` explicitly on other deployments before submitting a large
snapshot. HTTP 413 means the configured limit was exceeded. Use file/HTTP transfer
for these artifacts instead of putting the full input into a chat context.

## Parsing and growing sources

- Read every page of the required public MES tables. Check the MES revision,
  factory metadata and counts before/after extraction; also compare saved workbook
  bytes before/after. Retry changed sources up to three times, then fail. This is
  an optimistic consistency check, not a cross-system transaction.
- Read `Dispatch plan` and `Skills` by normalized column headings (trimmed,
  case-insensitive, repeated whitespace collapsed). Columns may move; headers
  may move down. IDs remain case-sensitive.
- Scan all saved worksheet rows, including hidden/filtered rows, blank gaps and
  rows beyond a stale Excel table/dimension range. No fixed row count is used.
  New MES operations do not need an Excel row. New Excel rows need an existing
  MES operation ID; the adapter does not create MES work from a spreadsheet.
- Duplicate IDs, mismatched lot/order IDs, unknown references, missing/duplicate
  required headings, invalid values and incomplete fixed decisions fail with a
  source location. Additional columns are ignored; new semantic columns require
  an explicit mapping. This is deterministic parsing, not guessed business logic.
- Required input columns must contain values, not formulas or spreadsheet errors.
  Formula caches cannot prove freshness. Calculated finish, lateness, due and
  priority columns are ignored; current MES dates/priorities remain authoritative.
  Save changes in Excel before importing. Unsaved edits cannot be read.

The required headings are declared in [workbook.py](demo_adapter/workbook.py).
Blank optional assignments/allowances are supported on nonfixed rows; `Fixed`
must explicitly contain `Yes` or `No`. The qualification matrix uses the six
released demo skill names with `Yes` / `No` values. A person missing from that
matrix has no eligible modes and appears in the report.

## Mapping contract

- Preserve operation, lot, order, machine and person IDs. Use released per-operation
  machine/time/material snapshots, never current workplan master data or seed
  internals. Existing MES lots are not split or expanded again.
- Derive weekday resource calendars from MES shifts, subtract breaks, absences
  and equipment blocks, and exclude permanently unavailable equipment/absent
  staff from new modes. Translate local plant dates through the source timezone
  into integer seconds. Reject ambiguous/nonexistent local times without offsets.
- Combine released machine alternatives with Excel qualifications. CNC setup-only
  attendance becomes a staffed setup phase and an unstaffed machine run phase;
  continuous attendance requires the person throughout. Phases stay contiguous.
- Excel setup/run allowances apply to the row's proposed machine. Other machines
  retain their released rates. Run allowances scale with remaining usable input
  quantity after booked upstream scrap; setup occurs once. Attendance allowances
  must agree with the released attendance contract. A supplied run allowance must
  be positive; leave it blank to use the MES rate. Zero setup is allowed.
- Proposed assignments, sequence and starts stay in the report. `Fixed=Yes`
  creates a mode lock (machine/person), a start lock and, where supplied, relative
  sequence among fixed operations on that machine. Fixed decisions never disappear
  to make a schedule possible. No technical `demo@1` native policy is activated.
- Completed/skipped operations are excluded from future work and retained in the
  report/source snapshot. Completed quantities feed the next operation. Current
  scrap reduces projected downstream quantities; no additional scrap is forecast.
  Jobs/orders represent remaining work, not historical completion KPIs.
- Running operations keep their actual machine, operator and start. The first
  implementation estimates uninterrupted setup-then-run progress from elapsed
  time and the duration allowance. This assumption is explicit; the MES does not
  supply phase-level actual work. Exhausted estimates, incompatible assignments
  or missing execution data fail instead of silently restarting work. A separate
  remaining-work field would be needed to model arbitrary pauses accurately.
- MES `on_hand` already includes received deliveries and posted material issues.
  Add only future receipts. Exclude unreceived overdue deliveries with a warning.
  Reserve the unissued balance of running operations from current stock because
  core execution inputs are treated as already consumed. Waiting operations
  consume their full input quantity when scheduled. Insufficient running stock
  fails; it is not supplied from a future receipt.
- An unresolved quality hold blocks import with a diagnostic. No release time is
  invented. The adapter does not add setup matrices, batch formation or other
  unstated factory policies.

The source horizon is preserved. Use `--horizon-end 2026-10-30T22:00` only when
deliberately extending it. The demo's Excel baseline can finish beyond its target
period. `PROPOSAL_HORIZON` reports proposed starts outside the imported horizon;
it neither extends the horizon nor turns proposals into fixed decisions.

## Reproducible full-factory showcase

For the shipped synthetic factory, explicitly import with
`--horizon-end 2026-10-30T22:00`, then select the existing `release` construction
strategy. It chooses machines and qualified people freely; it does not replay
Excel assignments or ordering. From the standalone application directory:

```bash
target/release/apex plan .apex/demo-import-001/problem.json \
  --strategy release --out .apex/demo-import-001/schedule.json
target/release/apex validate .apex/demo-import-001/problem.json \
  .apex/demo-import-001/schedule.json
```

After importing `scenario.json`, the equivalent `runs.start` MCP arguments are:

```json
{
  "scenario_id": "<returned scenario ID>",
  "options": {"method": "create", "options": {"strategy": "release"}}
}
```

Wait for a successful run, check `results.get` validation, then open `views.get`
with the scenario and result IDs. The complete acceptance case covers 3,705
unfinished operations, six running operations and 65 locks. A local release-build
measurement completed free construction in about 2.3 seconds; this is a feasible
starting plan, not an optimization or delivery-performance claim. The original
October 16 horizon and the default `queues` strategy did not produce complete
plans in that check. Failed greedy construction does not prove infeasibility.
New or edited source data always requires a fresh run and validation.

## Tests

From the application directory:

```bash
python -B -m unittest discover -s customization/demo/tests -p 'test_adapter.py' -v
APEX_TEST_BINARY="$PWD/target/release/apex" \
  python -B -m unittest discover -s customization/demo/tests -p 'test_adapter.py' -v
```

The optional executable test checks input readiness, planning, independent
validation and corrupted-result rejection. Fixtures are deliberately synthetic
and independent of the development-only MES package.
