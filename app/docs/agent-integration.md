# Agent integration

APEX exposes the same 32 tools through MCP stdio, MCP Streamable HTTP and a plain JSON HTTP API. The scheduler does not depend on a model provider. A chat host must support one of these tool transports, or supply a small bridge. A chat without tool access cannot invoke APEX merely by receiving its URL.

## Local MCP

Configure the host to launch the release executable:

```json
{
  "command": "/absolute/path/to/apex/target/release/apex",
  "args": ["mcp", "--with-viewer", "--workspace", "/absolute/path/to/apex"]
}
```

Use `apex.exe` on Windows. This is a command/arguments example; the surrounding configuration format belongs to the host. `--with-viewer` also starts the browser workbench on port 8765. An occupied viewer port does not terminate stdio tools. Select another `--port` if necessary.

For the existing Codex project setup, `bash deploy/setup-mcp.sh` writes the ignored project configuration. Run it in Bash, or Git Bash for a native Windows host, after building the release executable. It preserves other settings and leaves an existing APEX entry unchanged. Restart the host's MCP connection after changing the executable. See the [Bash helper guide](implementation.md#bash-helpers) and [configuration template](../examples/codex-mcp.toml).

## HTTP MCP and function tools

Run `apex serve --port 8765 --workspace /absolute/path/to/apex`. Endpoints:

| Endpoint | Purpose |
| --- | --- |
| `POST /mcp` | MCP Streamable HTTP, stateless JSON responses |
| `GET /api/tools` | Tool names, descriptions and argument schemas |
| `POST /api/tools/{name}` | Direct function invocation; request body is the argument object |
| `GET /openapi.json` | OpenAPI 3.1 description for function/action adapters |
| `GET /` | Plan viewer (optional `?mode=workbench`) |

For example, POST `{ "profile": "production" }` to `/api/tools/demo.create`, then pass the returned `scenario_id` in the JSON body of a POST to `/api/tools/schedule.create`. A result contains metrics and a `viewer_url`. HTTP tool failures return status 422 with structured diagnostics. MCP callers receive `isError` and the same diagnostic data.

The HTTP implementation uses the JSON response option in the [MCP transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports). Notifications receive 202; GET/DELETE on `/mcp` return 405 because this server does not use SSE or sessions. Both MCP transports are exercised with the official JavaScript SDK. These tests establish protocol interoperability, not certification of every commercial chat product.

## Hosted agents and authentication

`127.0.0.1` is reachable only on the computer running APEX. A cloud agent needs an endpoint reachable from its execution environment. Configure these environment variables for a deployment:

| Variable | Meaning |
| --- | --- |
| `APEX_BIND` | Listening interface; default `127.0.0.1` |
| `APEX_API_TOKEN` | Bearer token; required with at least 24 characters for a non-loopback binding |
| `APEX_PUBLIC_URL` | Externally reachable base URL used for links, allowed origin and OpenAPI server URL |

Provide HTTPS through the deployment's reverse proxy and pass `Authorization: Bearer ...`. Configure the proxy to preserve the configured public Host and Origin. Keep secrets out of URLs and repository files. The UI's **Agent connection** panel accepts the configured token in browser session storage and displays the relevant endpoints.

This is a single-workspace service with a shared bearer token. It has no built-in TLS, OAuth, user identities, tenant isolation or asynchronous job queue. Hosts that require OAuth need a compatible gateway. Deployment to an external service is separate from this local implementation.

## Roles and viewer

The portable repository skills under `skills/` cover data intake, planning and code extensions. A single agent can use all three. The default viewer keeps scenario controls out of the planning flow; `?mode=workbench` exposes development controls. See [chat-led workflows](agent-workflows.md).

## Suggested agent workflow

1. `capabilities` and `schema.get`: discover supported semantics. Request a named definition rather than the entire schema.
2. Import a canonical problem with `problem.import`, or orders and workplan templates with `production.import`.
3. When existing supply needs allocation, use `material.prepare` to create a separate prepared scenario and page its report. See [the material contract](material-dispatch.md). Inspect diagnostics; missing processing work is an error, never silently replaced with zero. Preserve source references and record explicit estimates in `assumptions`.
4. `schedule.create` produces a quickly constructed, independently validated plan. `queues.inspect` exposes goal/proxy mapping. `schedule.improve` combines XH and XT, with an optional XE, under one total budget. Pass `schedule_id` only for an incumbent from the same scenario identity and revision. Individual XH/XT tools remain available for diagnostics; `schedule.evolve` refines a same-revision saved plan with its own requested budget. Search exposes phases, improvement curves and replay metadata; see [search settings](search-and-parity.md).
5. `scenario.fork`, `scenario.patch` and `scenario.freeze` define a proposed change. `schedule.repair` reconstructs a plan under those constraints; `scenario.compare` explains changed, added and removed operations and KPI differences.
6. `schedule.validate` replays hard constraints and custom evaluation against the saved input. Return the `viewer_url` to the planner.

Use `model.page` for routes, jobs, orders, dependencies, rules and locks; `task.inspect` for one operation and its active relationships. Deep links accept `schedule`, `scenario`, `view=jobs|routes|rules|search|schedule`, `task` and `baseline`. Links open an independent web UI; they do not require an embedded chat widget.

## Large inputs and bounded context

An optional source adapter can be an agent-generated converter. Read source samples and metadata, run the converter against the complete artifact, and keep complete row sets outside the language-model context.

Canonical task import supports `import.begin`, `import.append`, `import.finalize`. Each append permits at most 5,000 tasks and 4 MiB. Stable chunk IDs make retries idempotent. Finalization checks references across chunks before making the scenario available. Supply the non-task model header to `import.begin`; arbitrarily large relationship collections need a file-based import/converter rather than one enormous prompt. Production-template expansion is currently a whole-input operation; for large exports, expand to canonical JSON outside the model and use staged task import.

File paths refer to the server workspace, not the remote client's filesystem. Remote clients can send bounded inline chunks; shared files require an explicit transfer or mounted workspace. Tool requests are limited to 8 MiB.

Tool responses over 64 KiB are retained as artifacts. `artifact.read` takes the returned artifact ID and a JSON Pointer, pages object keys/array items and bounds long strings. Remote agents therefore do not need local filesystem access to inspect retained output. Ordinary pagination and targeted reads should remain the default. Batching limits transport and context size; the current engine still holds the compiled problem in memory.

## Verification

After `cargo build --release` and `npm ci`:

```text
node tests/mcp-smoke.mjs --large
node tests/http-mcp.mjs
node tests/viewer-smoke.mjs
node tests/viewer-plan-smoke.mjs
node tests/material-workflow.mjs
```

The viewer test expects a server on port 8765. The HTTP test starts and stops its own authenticated instance on port 18765. Set `APEX_BROWSER` if the default local Chromium/Edge executable is unavailable.
