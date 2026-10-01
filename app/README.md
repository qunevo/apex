# APEX

[![CI](https://github.com/qunevo/apex/actions/workflows/ci.yml/badge.svg)](https://github.com/qunevo/apex/actions/workflows/ci.yml)

APEX is an experimental Rust scheduler for discrete production, built for agent tool use. It supports alternative workplans, conditional activity graphs, quantity/order expansion, existing-supply material allocation, native customization hooks and independent schedule validation. XG construction, XH hypersearch, XT tree search and XE schedule evolution share the same scheduling rules. A central middleware serves agents over MCP/HTTP and presents results through an MCP App or optional desktop client.

The central server owns scenarios, background runs, approvals and persistent state. The separate compatibility executable retains its file-backed tools and browser viewer. Neither scheduling path needs a model API key or external solver.

## Start APEX

This directory is an independent source distribution. Copy its source files alone or enter `app/` in the full repository. Install Docker with Linux containers and Docker Compose 2.20.3 or newer. No host Rust, Python or Node.js installation is needed.

```bash
docker compose up --build
```

This builds APEX and starts PostgreSQL, initialization and migrations. Connect a supporting chat host to `http://127.0.0.1:8780/mcp`. Retrieve access tokens with:

```bash
docker compose exec apex apex-container access
```

The MCP App is displayed inside the chat. The optional desktop is a separate display client. The base stack does not activate a customization or start source systems. Read the [container guide](docs/containers.md) for retained state, ports, configuration and shutdown, and [control platform](docs/control-platform.md) for tools and clients.

For native Rust builds, direct scheduling commands and the retained file-backed viewer, use the [operating guide](docs/implementation.md). The compatibility viewer at port 8765 has its own state and tool catalog; Compose does not start it.

## Product skills

The three product workflows are in [skills/](skills): planning, data intake and native customization. Configure your agent to load the relevant `SKILL.md` from this installation, or open it as workflow guidance. Paths inside skills refer to this application directory. The full repository's contributor skill is separate.

## Documentation

Start with the [documentation index](docs/README.md). The main references are:

- [Operating guide](docs/implementation.md): build, run, verify and understand capability limits.
- [Executable data model](docs/data-model.md): fields, units, relationships, commitments and KPIs.
- [Agent workflows](docs/agent-workflows.md) and [integration](docs/agent-integration.md): chat-led planning and tool access.
- [Material preparation](docs/material-dispatch.md), [search](docs/search-and-parity.md) and [direct evolution](docs/direct-schedule-evolution.md): current behavior and boundaries.
- [Architecture](docs/architecture/README.md): current modules, evaluation flow, state boundaries and change map for coding agents.
- [Migration audit](https://github.com/qunevo/apex/blob/main/dev/docs/migration-audit.md): v2 comparison, implemented coverage and remaining migration boundaries.
- [Benchmark of 24 September 2026](https://github.com/qunevo/apex/blob/main/benchmark/README.md): frozen sources, scientific instances, results and standalone reproduction instructions in one ZIP.

The [factory showcase](https://github.com/qunevo/apex/tree/main/demo) combines a fictional MES with Excel planning. Its application integration belongs to [customization/demo](customization/demo/model/KNOWLEDGE.md); the live adapter is still pending. Technical regression inputs live under `tests/fixtures`.

The executable schema is `apex.v3.4`; canonical `apex.v3.1`, `apex.v3.2` and `apex.v3.3` inputs remain accepted by the same Rust runtime. Rust is the sole scheduling implementation; the [migration audit](https://github.com/qunevo/apex/blob/main/dev/docs/migration-audit.md) records the v2 source removal and historical comparisons. Tested coverage does not establish complete legacy parity, optimality or production readiness. Repository documentation and examples use English, and all fixtures are deliberately synthetic.

## Contributing and support

Read the [contribution guide](https://github.com/qunevo/apex/blob/main/CONTRIBUTING.md) before starting a change. Use [Discussions](https://github.com/qunevo/apex/discussions) for questions and [issue forms](https://github.com/qunevo/apex/issues/new/choose) for reproducible bugs and feature requests. Keep all shared examples deliberately synthetic.

The [support guide](https://github.com/qunevo/apex/blob/main/SUPPORT.md), [Code of Conduct](https://github.com/qunevo/apex/blob/main/CODE_OF_CONDUCT.md), [security policy](https://github.com/qunevo/apex/blob/main/SECURITY.md) and [repository maintenance guide](https://github.com/qunevo/apex/blob/main/dev/docs/repository-maintenance.md) describe the community channels and review process. Report vulnerabilities privately. Upstream contribution rights require a separate agreement; posting a pull request does not accept one.

## License

APEX is distributed under the [APEX Source Available License 1.1](LICENSE). The included license contains the complete public grant, eligibility and pricing rules. It is source available, not OSI-approved open source.

See the [licensing overview](LICENSING.md) and [pricing and eligibility](docs/legal/pricing.md). There is no per-operation, per-user, per-site or per-run metering. Commercial and contributor agreements require separate acceptance; [the legal documentation](docs/legal/README.md) explains the published document roles. Third-party components retain their own licenses. [Publication preparation](https://github.com/qunevo/apex/blob/main/dev/docs/publication.md) records the release checks.

## Application packages

The application is organized into `core/`, `middleware/`, `ui/`, `skills/`
and `customization/`. See [server configuration](docs/server-configuration.md)
for an enabled-package list, default selection and per-request customization.

The [control platform](docs/control-platform.md) adds tenants, versioned scenarios,
background runs, approval and publication on PostgreSQL, served over HTTP and MCP by
the `apex-control` executable, and a desktop display client. It uses the same
scheduling engine crate and leaves the `apex` executable unchanged.
