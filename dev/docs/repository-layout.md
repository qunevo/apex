# Repository layout and distribution

The repository contains an independently usable application, contributor tooling and a frozen scientific benchmark.

| Location | Responsibility |
| --- | --- |
| [app/](../../app) | Complete customer source: Rust package and lockfile, embedded viewer, schemas, synthetic examples, public customization, product skills, deployment helpers, documentation and product tests |
| [dev/](..) | Fixture generation, wiki/publication checks, detached application verification, contributor documentation and tests of these tools |
| [dev/skills/](../skills) | Canonical contributor workflows for planning, development, integration, releases and maintenance |
| [.agents/skills/](../../.agents/skills) | Generated discovery entries linking to the contributor skills |
| [.github/](../../.github) | CI, dependency updates and contribution infrastructure |
| [benchmark/](../../benchmark/README.md) | Unchanged frozen reproduction package; it does not use the current application |

## Independent application

`app/` owns its `Cargo.toml`, `Cargo.lock`, test-tooling manifests and release profile. There is no parent Cargo workspace or root lockfile. Enter `app/` before running Cargo, npm or product commands. Product tests stay next to the application they verify. The scheduler does not require Node.js or Python; those tools are used for development and integration tests.

All compile-time assets and runtime setup helpers resolve inside the application. Deployment helpers locate the application relative to their own script, regardless of the caller's current directory. MCP setup can select a separate data workspace while retaining the binary in the application installation. Local `.apex` state, `.codex` settings, installed dependencies, generated reports and build output are excluded from distribution.

The root [LICENSE](../../LICENSE) is canonical. [app/LICENSE](../../app/LICENSE) is its byte-identical distribution copy. Keep both synchronized when the canonical license changes; `check_standalone.py` rejects drift. Product documentation links to the local copy. Links from product documentation to contribution processes and historical evidence are optional online references, not build or runtime dependencies.

The module architecture remains [with the application source](../../app/docs/architecture/README.md), because customers implementing native customizations need that contract too. Contributor processes and historical migration evidence live in `dev/docs`.

See [developer workflow](developer-workflow.md) for the dev/main branch model and agent authorization, [versioning](versioning.md) for product releases, and [documentation maintenance](documentation-maintenance.md) for drift and context audits.

## Verification

From the repository root:

```bash
cd app
bash deploy/build.sh
npm ci --ignore-scripts
npm run test:mcp
npm run test:http
cd ..
python -B -m unittest discover -s dev/tests -p 'test_*.py' -v
python -B dev/scripts/build_wiki.py
python -B dev/scripts/check_standalone.py
```

The standalone check exports only Git source files under `app/` to a fresh temporary directory with spaces in its path, outside the checkout. It checks licensing, builds with the lockfile, expands a production example, runs all five scheduling methods, independently validates the results, and exercises embedded HTTP assets and MCP setup. It removes the temporary copy on completion. CI requires this check in addition to the Rust and integration checks.

Copy or archive source files, not a developer's working directory containing local data. A binary release is a separately assembled artifact with the application, applicable product skills, startup instructions and license; public executable distribution follows the existing licensing terms.
