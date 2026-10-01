# Start APEX with Docker Compose

The application is self-contained. Copy `app/` alone or enter it in the full
repository. Install Docker with Linux containers and Docker Compose 2.20.3 or
newer. Rust, Python, Node.js and a desktop APEX installation are not needed on
the host. The first build downloads the base images and Rust dependencies.

```bash
docker compose up --build
```

Use `docker compose up --build -d --wait` to start in the background and wait
for health checks. APEX listens at `http://127.0.0.1:8780`; `/health` reports
readiness and `/mcp` is the authenticated MCP endpoint. The server provides an
MCP App resource, displayed by a supporting chat host after a tool result.
There is no standalone dashboard at `/` on this server.

## What starts

The [Compose file](../compose.yaml) builds the [server image](../Dockerfile)
containing middleware, the linked core, MCP App assets, customization packages,
general skills and the license. It starts:

- `setup`: initializes random database passwords and an APEX identity once.
- `postgres`: stores scenarios, revisions, runs, decisions and results.
- `migrate`: creates the restricted application role and applies migrations.
- `apex`: serves HTTP, MCP and events, and runs planning workers.

The server runs as an unprivileged container user and connects to PostgreSQL as
a non-owner role without superuser or `BYPASSRLS` rights. Owner credentials are
mounted only into initialization/database services. PostgreSQL has no published
host port. HTTP is published on loopback for local use; remote deployments need
their own HTTPS endpoint and access configuration.

The base stack has no dependency on a parent checkout or source system. It does
not activate a customization by default. The optional native desktop is installed
and started separately; see [desktop connection](control-platform.md#desktop-display-client).

## Connect a chat

After startup, retrieve the initial access tokens explicitly:

```bash
docker compose exec apex apex-container access
```

Use the agent token as the bearer credential for `http://127.0.0.1:8780/mcp`.
Use the viewer token for the optional desktop client. Token hashes are in the
server's auth file; the bootstrap keeps the initial plaintext tokens in the
private `apex-state` volume so this command works after restart. They are not
printed by normal startup or stored in the repository. A hosted chat needs a
reachable server address instead of this machine's loopback address.

The tool contract and a client configuration snippet are in
[control platform](control-platform.md#agents-mcp). Skills are bundled source
files; the chat host must load the applicable skills separately. Container
startup does not install a chat connector or agent skills automatically.

## Configuration and retained state

Optional environment settings (or an ignored local `.env` beside the Compose file):

| Setting | Default | Purpose |
| --- | --- | --- |
| `APEX_HTTP_PORT` | `8780` | Host loopback port |
| `APEX_CONTROL_WORKERS` | `2` | Concurrent background workers |
| `APEX_CONFIG` | empty | Customization configuration path **inside the container** |
| `APEX_IMAGE` | `apex-local:dev` | Locally built image tag |

Mount a deployment-specific configuration and optional package files using a
local Compose override, then set `APEX_CONFIG` to the mounted path. Native Rust
extensions require registration and rebuilding; folder mounts do not load code.
See [package configuration](server-configuration.md) for selection and versioning.

Compose uses project-scoped `postgres-data`, `apex-state` and `bootstrap` volumes.
Ordinary `docker compose down`, restart and image rebuild preserve them. Keep all
three together when backing up or restoring this deployment. `docker compose down
-v` intentionally removes all three, including database records and credentials.
The next start creates a new installation and new tokens.

```bash
docker compose ps
docker compose logs apex
docker compose down
```

Do not change a project's storage identity or remove only its credential volume
while keeping its database. If ports are occupied, stop the other installation or
set a different `APEX_HTTP_PORT` before starting.

## Repository showcase

The full repository's separate `demo/` deployment includes this Compose file and
adds its MES, workbook mount and package selection. Its
[start guide](https://github.com/qunevo/apex/tree/main/demo) owns source-system
addresses, synthetic data and reset behavior. The application image/build context
never includes that directory. The standalone application does not require it.

Compose [include](https://docs.docker.com/compose/how-tos/multiple-compose-files/include/)
keeps paths relative to the included application, and
[dependency conditions](https://docs.docker.com/compose/how-tos/startup-order/)
wait for initialization, database readiness and migration completion.
