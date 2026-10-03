# Repository layout and distribution

The repository contains an independently usable application, contributor tooling and a frozen scientific benchmark.

| Location | Responsibility |
| --- | --- |
| [app/](../../app) | Complete customer source: Rust package and lockfile, embedded viewer, schemas, public customization, product skills, deployment helpers, documentation and product tests with synthetic fixtures |
| [dev/](..) | Fixture generation, wiki/publication checks, detached application verification, contributor documentation and tests of these tools |
| [demo/](../../demo) | Standalone fictional factory: local MES web app, synthetic business records and a separate Excel planning baseline; optional and not required by the application |
| [dev/skills/](../skills) | Canonical contributor workflows for planning, development, integration, releases and maintenance |
| [.agents/skills/](../../.agents/skills) | Generated discovery entries linking to the contributor skills |
| [.github/](../../.github) | CI, dependency updates and contribution infrastructure |
| [benchmark/](../../benchmark/README.md) | Unchanged frozen reproduction package; it does not use the current application |

## Independent application

Inside `app/`, `core/` owns algorithms. `middleware/` groups API entry points,
control operations and data persistence. `ui/` contains the MCP App and desktop.
`skills/` holds general workflows; `customization/<id>/` groups each domain's
adapter, model, skills, tests and optional views. `app/Cargo.toml` is also the
workspace root for the `apex-engine` crate in `core/` and the control-platform
crates (`middleware/control/`, `middleware/data/`, `middleware/api/`,
`ui/desktop/`); see [control platform](../../app/docs/control-platform.md). The
desktop client is not a default member, so ordinary builds do not need GPUI.
See [server configuration](../../app/docs/server-configuration.md) for package
allowlists, defaults and request-specific selection.

`app/` owns its Cargo workspace, `Cargo.lock`, test-tooling manifests and release profile. There is no parent Cargo workspace or root lockfile. Enter `app/` before running Cargo, npm or product commands. Product tests stay next to the application they verify. The scheduler does not require Node.js or Python; those tools are used for development and integration tests.

All compile-time assets and runtime helpers resolve inside the application. `app/scripts/` contains the build helper and the Docker container entrypoint. The build helper locates the application relative to its own script, regardless of the caller's current directory. Local MCP clients configure the executable and data workspace directly; see [agent integration](../../app/docs/agent-integration.md#local-mcp). Local `.apex` state, `.codex` settings, installed dependencies, generated reports and build output are excluded from distribution.

The root [LICENSE](../../LICENSE) is canonical. [app/LICENSE](../../app/LICENSE) is its byte-identical distribution copy. Keep both synchronized when the canonical license changes; `check_standalone.py` rejects drift. Product documentation links to the local copy. Links from product documentation to contribution processes and historical evidence are optional online references, not build or runtime dependencies.

The module architecture remains [with the application source](../../app/docs/architecture/README.md), because customers implementing native customizations need that contract too. Contributor processes and historical migration evidence live in `dev/docs`.

See [developer workflow](developer-workflow.md) for the dev/main branch model and agent authorization, [versioning](versioning.md) for product releases, and [documentation maintenance](documentation-maintenance.md) for drift and context audits.

## Container deployment boundaries

`app/compose.yaml` and its Docker build context work from an application-only
copy. Initialization and migration services prepare PostgreSQL and access tokens.
`demo/compose.yaml` includes that base and supplies its own APEX override, MES
image and host workbook mount. No application deployment file refers to the
parent demo. The desktop remains an optional native client.
See the [container contract](../../app/docs/containers.md).

The optional `demo/scripts/start-demo.sh` starts the showcase and opens its shared
host workbook plus a private local setup page. It offers resume or complete demo
reinitialization; the latter also recreates the planning database while retaining
access tokens. MES reset restores the workbook and MES baseline together; it
preserves the APEX data and identity volumes. See
the [demo workflow](../../demo/README.md) for Excel locking and recovery behavior.

Run `python -B dev/scripts/check_containers.py` with a Linux Docker engine to
verify both deployments in temporary source exports. It builds the app with no
parent demo present, plans through MCP, checks the UI resource and database role,
then recreates services to verify credentials, plans and MES/Excel persistence.
The demo check also exercises the local starter and workbook reset after reuse.
The test only removes its uniquely named projects and temporary files.

## Verification

From the repository root:

```bash
cd app
bash scripts/build.sh
npm ci --ignore-scripts
npm run test:mcp
npm run test:http
cd ..
python -B -m unittest discover -s dev/tests -p 'test_*.py' -v
python -B dev/scripts/build_wiki.py
python -B dev/scripts/check_standalone.py
```

The standalone check exports only Git source files under `app/` to a fresh temporary directory with spaces in its path, outside the checkout. It checks licensing and Bash syntax, builds with the lockfile, expands a production example, runs all five scheduling methods, independently validates the results, and exercises embedded HTTP assets with a separate data workspace. It removes the temporary copy on completion. CI requires this check in addition to the Rust and integration checks.

Copy or archive source files, not a developer's working directory containing local data. A binary release is a separately assembled artifact with the application, applicable product skills, startup instructions and license; public executable distribution follows the existing licensing terms.
