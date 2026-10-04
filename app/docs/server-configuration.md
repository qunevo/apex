# Server configuration and customization packages

The central `apex-control` service and the compatibility `apex` executable use
the same configuration file and package manifests. Both accept `--config FILE`
or `APEX_CONFIG`. Core algorithms remain independent of transport and persistent
state. For the central service, pass `apex-control serve --config FILE` with your configuration;
see [control platform](control-platform.md) for scenario-bound selection and
PostgreSQL storage. The rest of this page documents the file-backed compatibility
interface:

```bash
apex serve --config /opt/apex/apex.config.json --workspace /var/lib/apex
```

The same `--config` option works with `mcp` (including `--with-viewer`) and
`tool`. `APEX_CONFIG` is the environment equivalent; an explicit flag wins.
Without either, the existing flat workspace/store behavior is unchanged.
For a [local MCP client](agent-integration.md#local-mcp), add `--config` and its
file path to the executable's `args` when package routing is wanted.

The base container stack leaves `APEX_CONFIG` empty. An optional package-specific [configuration](../customization/demo/apex.config.json) illustrates selecting the bundled synthetic package:

```json
{
  "customization_root": "..",
  "default_customization": "demo",
  "enabled_customizations": ["demo"]
}
```

Use this optional file with `--config customization/demo/apex.config.json` in a native build, or select its container path through a deployment override. See [containers](containers.md#configuration-and-retained-state).

The package root resolves relative to the configuration file, independent of
the server's working directory. Each enabled folder must contain `package.json`
with its matching `id` and a version token (lowercase letters, digits, hyphens or
underscores, at most 64 characters). Invalid, duplicate, unknown or escaping
folders/manifests fail startup. Configuration is read at startup, not hot-loaded.

Every tool accepts an optional top-level `customization` folder ID:

```json
{"scenario_id": "saved-scenario-id", "customization": "demo"}
```

An omitted ID uses the configured default. An explicit ID must be enabled.
All follow-up calls, including chunk imports, artifact paging and comparisons,
use the same selection. Calls never change a global active package. Capabilities
report the enabled IDs, default and selected package identity/version. The
viewer carries that selection in its URL and requests.

Configured storage uses `<store>/customization/<id>/<version>/`; input file
paths are relative to `<workspace>/inputs/<id>/` and cannot escape that folder.
The default store is `<workspace>/.apex`. Scenarios, import sessions and saved
schedules also retain the package identity/version. Different packages and
versions cannot read each other's artifact IDs through the tools. Changing the
default does not move existing data. A new manifest version starts a separate
store; migrating old state requires an explicit future migration procedure.

Native model selection is separate: `Problem.customization` still selects a
compiled, versioned scheduling extension. Enabling the `demo` package
does not impose its optional technical policy (`demo@1`) on every problem.
The package also contains an optional [MES/Excel source adapter](../customization/demo/adapter/README.md).
Run it explicitly to export input JSON; server package selection does not execute
it or synchronize source systems.
Folders and Markdown never execute code or automatically install agent skills.

The server remains a shared-trust workspace service. Package namespaces are
not per-user authorization or tenant isolation: clients with server access can
select any enabled package. Use the existing authenticated HTTP deployment
contract. The central `apex-control` service supplies active-plan lifecycle,
database storage, tenant authorization and MCP Apps resources. Its UUID-based
records are separate; file artifacts are not migrated automatically.
