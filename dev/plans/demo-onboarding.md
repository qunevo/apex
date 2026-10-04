# Local demo startup and shared Excel editing

> Historical implementation record. Paths and feature status below describe that
> batch, not the current product. See [current documentation](../../demo/README.md).

Status: implemented on the selected `codex/integration-cleanup` branch. The user
approved the local launcher and explicitly required resetting the shared Excel
file together with the MES, including an already used container installation.

Approved extension: the starter offers **Resume the last state** as the default
and **Reinitialize demo** to reset MES/Excel plus all APEX planning data. The user
explicitly selected retaining the current access tokens. Add explicit `--resume`
and `--reset` modes for scripted starts; noninteractive starts retain state by
default. Verify volume ownership and exclusive use before reset, reject an open
workbook before deleting APEX data, then recreate only PostgreSQL storage and
refresh the setup page. Exercise retained state, reset, failed reset and retained
credentials in a disposable Compose project. No reset of the user's active demo
is needed to implement or verify this extension.

Approved reopening extension: replace the MES workbook download with **Open in
Excel**. The local starter manages a background helper that opens the fixed
shared workbook through the host file association. MES requests cannot specify
paths or commands. Heartbeats and session generations detect stopped or replaced
helpers. Show failures and the host path in the MES. No automatic MES/Excel
synchronization, new network listener, registry handler or host Python/Node
dependency is added. Test repeat opening, stale helpers, request boundaries and
Windows background launch, plus the existing container reset/persistence checks.

## Current behavior

- `demo/compose.yaml` mounts `.local/container/` into the MES at `/var/lib/demo`.
  `demo/apex.compose.yaml` mounts the same directory into APEX at `/sources/demo`
  read-only. The editable workbook already exists on the host and in both
  containers; the MES download creates an independent copy.
- `app/scripts/container.sh` creates identity files only when they are absent.
  Restarting, rebuilding or recreating containers with the same named volumes
  preserves tokens. A new Compose project with new volumes, or deletion of the
  identity volume, creates a new identity. Existing container checks verify token
  and workbook persistence across service recreation.
- The central MCP catalog does not expose the MES or workbook. The demo adapter
  is reserved but unimplemented. A container mount alone does not grant an MCP
  client access to those sources. Local agents with filesystem and HTTP tools
  can access the host workbook and MES independently.

## Implemented user workflow

1. Run `bash demo/scripts/start-demo.sh` from the repository checkout. The helper
   resolves its own location, starts Compose with `up --build -d --wait`, and
   returns the terminal after startup.
2. The launcher opens the existing `.local/container/production-planning.xlsx`
   with the host's associated spreadsheet application and opens a generated
   local HTML onboarding page. Saving in Excel updates the shared working file.
   Opening or relaunching the demo never resets the workbook.
3. The English onboarding page offers **Open MES**, **Copy connection details**,
   **Copy setup instructions**, and **Copy workbook path**. It explains that
   changes must be saved to this working file. The setup text includes the
   current MCP URL, agent authorization, MES URL and absolute host workbook path.
   It targets a local agent capable of configuring its client and reading files;
   it does not promise that pasting text alone installs tools in every chat host.
4. MCP configuration is a one-time client operation. Reuse the existing token and
   connection on later starts. If a client needs a reconnect to load its new
   configuration, show that as an explicit setup step.
5. Close Excel and choose **Reset demo** in the MES. Restore the baseline workbook
   and MES records together; reopen the workbook afterward. Normal starts retain
   edits. Reset does not rotate credentials or delete APEX scenarios. An Excel
   owner file or failed file replacement rejects the reset and preserves MES
   changes. A database failure restores the prior workbook from a temporary
   backup; failed recovery retains that backup and reports the need to restore it.
   This coordinates ordinary failures, not a filesystem/SQLite crash transaction.

## Implementation order and boundaries

- Add the Bash launcher under `demo/scripts/`, with Windows/Git Bash, macOS and
  Linux open commands, quoted paths, configured Compose ports and clear startup
  failures. Use Docker and existing container runtimes for any rendering helper;
  do not require a new host Python/Node installation. The approved desktop opener
  uses a local Bash helper managed automatically by the starter.
  Keep `scripts/start-demo.sh` as the single demo launcher. The user approved
  removing the redundant MES-only launcher; direct Python startup remains
  documented for development.
- Add a small maintained English HTML template and rendering helper in `demo/`.
  Retrieve the existing agent connection once after readiness and render a local
  ignored page outside the directory shared with the MES. Escape all dynamic
  values, keep credentials out of tracked files and URLs, and do not copy the
  database credentials or viewer token into the page. Refresh it on every start.
  Do not expose an unauthenticated token-discovery endpoint in the APEX server.
- Keep the workbook's current host path and directory mount. In the MES, label
  the workbook action as **Open in Excel** and reopen the shared working file
  through the local helper. Remove the copy-download action and endpoint. Keep
  the original-seed preview clearly identified; do not imply that it reflects
  saved workbook edits. Update both MES locales.
- Update `demo/README.md` and relevant startup guidance. Keep direct Compose
  startup supported; only the local launcher opens host applications. No
  application-to-parent dependency, scheduler change, automatic MES import,
  spreadsheet writeback by APEX, or native scheduling adapter is part of this
  onboarding change.

## Acceptance and verification

- One launch starts the stack, opens the onboarding page and shared workbook,
  and requires no second terminal or manual token extraction. Exercise the real
  Windows host open/save workflow; other platform behavior is reported according
  to the checks actually available.
- Paths containing spaces, changed host ports, an already running stack,
  existing workbook edits and unavailable desktop applications are handled.
  If an application cannot open automatically, show the exact path/URL and a
  clear message without failing a healthy container stack.
- Save a deliberate synthetic workbook change using desktop Excel; verify the
  saved value from the host and the file bytes visible to both containers. After
  recreation, the changed workbook and same agent token remain available.
- Verify page escaping and copy text, absence of credentials in tracked source,
  truthful source-access instructions, MES locale parity and the renamed download
  label. Run relevant MES tests, documentation checks and container smoke checks
  in disposable projects without resetting the user's active demo.
- Stop after a reviewed local implementation on `codex/integration-cleanup`.
  Publication and integration into `dev` require their own existing or subsequent
  authorization.

## Verification evidence

- 54 MES/onboarding tests passed, including repeated HTTP reset, open-workbook
  rejection, failed replacement, database rollback, retained recovery backups
  and cleanup failures. Launcher checks cover paths with spaces, custom ports,
  retained edits, private page permissions and mocked successful/failed Windows,
  macOS and Linux application open commands.
- Starter mode checks cover resume, explicit reset, conflicting options, foreign
  and shared volume rejection, and an open workbook leaving APEX data untouched.
  An interactive terminal check verified menu rendering, invalid-input retry and
  Enter selecting resume by default.
- 52 contributor tooling tests passed. JavaScript syntax and 728 matching locale
  templates passed.
- The disposable Docker demo check passed: local starter, private setup page,
  authenticated planning, retained tokens and workbook edits across recreation,
  reset after reuse, identical baseline bytes on the host and in both containers,
  and preservation of APEX plans and credentials during MES reset.
- Full starter reinitialization also passed in Docker: an open workbook rejects
  the operation before APEX data removal; successful reset empties the scenario
  list, restores MES/Excel, refreshes the page and retains both access tokens.
  The retained agent token can create a new scenario in the fresh database.
- A Chromium browser check passed for setup copy controls/fallback, narrow-screen
  layout and reset instructions. A later check exercised the MES **Open in Excel**
  button through the actual Windows background helper and confirmed the separate
  synthetic test workbook appeared in Excel. The download link and endpoint were
  removed. Helper tests cover repeated requests, fixed-file handling, stale
  sessions, rate limits, cross-origin rejection and retirement on starter reuse.
- No actual Excel save is claimed; shared file visibility/reset is verified in
  Docker. The user stopped Computer Use with Escape during cleanup of the test
  workbook, and further UI actions were stopped. The owned helper/container and
  local test server were stopped separately. macOS/Linux desktop apps were not
  available for an interactive check.

## Alternative for remotely hosted workbooks

An **Open in Excel** Office URI can launch desktop Excel for an HTTP document,
but saving back requires a server-side authoring protocol such as WebDAV,
including file-lock handling. A download URL alone is insufficient. This adds a
service and Office integration testing. The user selected the local desktop
helper with the existing shared file instead; no WebDAV service was added.

References: [Docker bind mounts](https://docs.docker.com/engine/storage/bind-mounts/),
[Office URI schemes](https://learn.microsoft.com/en-us/office/client-developer/office-uri-schemes),
[Office web-server authoring](https://learn.microsoft.com/en-us/openspecs/office_protocols/ms-ocproto/2d268a23-420b-4339-8c56-9e4f8a924171),
[MCP client configuration](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).
