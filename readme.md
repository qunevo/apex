# APEX

[![CI](https://github.com/qunevo/apex/actions/workflows/ci.yml/badge.svg)](https://github.com/qunevo/apex/actions/workflows/ci.yml)

APEX is an experimental Rust scheduler for discrete production, built for agent tool use. It supports alternative workplans, conditional activity graphs, quantity/order expansion, existing-supply material allocation, native customization hooks and independent schedule validation. XG construction, XH hypersearch, XT tree search and XE schedule evolution share the same scheduling rules. A central middleware serves agents over MCP/HTTP and presents results through an MCP App or optional desktop client.

The central server owns scenarios, background runs, approvals and persistent state. The separate compatibility executable retains its file-backed tools and browser viewer. Neither scheduling path needs a model API key or external solver.

## Repository areas

- [app/](app/README.md) is the independent customer source distribution, with its own manifests, lockfiles, license, product skills, deployment helpers and tests. Build and run it without the parent repository. Keep local data and generated files out of distributed copies.
- [demo/](demo/README.md) composes the fictional MES/Excel showcase around the application.
- [dev/](dev/docs/repository-layout.md) contains contributor documentation, fixture generation, wiki/publication tooling and standalone verification.
- [.agents/skills/](.agents/skills) contains repository development workflows.
- [.github/](.github) contains CI and contribution infrastructure.
- [benchmark/](benchmark/README.md) contains the unchanged frozen reproduction package.

## Start APEX

From the repository root, enter `app/`. Install Docker with Linux containers and Docker Compose 2.20.3 or newer. No host Rust, Python or Node.js installation is needed.

```bash
cd app
docker compose up --build
```

This builds APEX and starts PostgreSQL, initialization and migrations. Connect a supporting chat host to `http://127.0.0.1:8780/mcp`. Keep the stack running and use a second terminal to display the connection details:

```bash
docker compose exec apex apex-container connect
```

The MCP App is displayed inside the chat. The optional desktop is a separate display client. The base stack does not activate a customization or start source systems. Read the [container guide](app/docs/containers.md) for retained state, ports, configuration and shutdown, and [control platform](app/docs/control-platform.md) for tools and clients.

For native Rust builds, direct scheduling commands and the retained file-backed viewer, use the [operating guide](app/docs/implementation.md). The compatibility viewer at port 8765 has its own state and tool catalog; Compose does not start it.

For the complete MES/Excel showcase, start from the repository's `demo/` directory instead:

```bash
cd demo
docker compose up --build
```

The demo includes the same APEX stack and adds its MES at `http://127.0.0.1:8788`. Start one stack at a time with the default ports. Source-system mapping is still pending; see the [demo guide](demo/README.md).

## Documentation

Start with the [documentation index](app/docs/README.md). The main references are:

- [Operating guide](app/docs/implementation.md): build, run, verify and understand capability limits.
- [Executable data model](app/docs/data-model.md): fields, units, relationships, commitments and KPIs.
- [Agent workflows](app/docs/agent-workflows.md) and [integration](app/docs/agent-integration.md): chat-led planning and tool access.
- [Material preparation](app/docs/material-dispatch.md), [search](app/docs/search-and-parity.md) and [direct evolution](app/docs/direct-schedule-evolution.md): current behavior and boundaries.
- [Architecture](app/docs/architecture/README.md): current modules, evaluation flow, state boundaries and change map for coding agents.
- [Migration audit](dev/docs/migration-audit.md): v2 comparison, implemented coverage and remaining migration boundaries.
- [Benchmark of 24 September 2026](benchmark/README.md): frozen sources, scientific instances, results and standalone reproduction instructions in one ZIP.

The [factory showcase](demo/README.md) combines a fictional MES with Excel planning. Its application integration belongs to [customization/demo](app/customization/demo/model/KNOWLEDGE.md); the live adapter is still pending. Technical regression inputs live under `app/tests/fixtures`.

The executable schema is `apex.v3.4`; canonical `apex.v3.1`, `apex.v3.2` and `apex.v3.3` inputs remain accepted by the same Rust runtime. Rust is the sole scheduling implementation; the [migration audit](dev/docs/migration-audit.md) records the v2 source removal and historical comparisons. Tested coverage does not establish complete legacy parity, optimality or production readiness. Repository documentation and examples use English, and all fixtures are deliberately synthetic.

## Contributing and support

Read the [contribution guide](CONTRIBUTING.md) before starting a change. Use [Discussions](https://github.com/qunevo/apex/discussions) for questions and [issue forms](https://github.com/qunevo/apex/issues/new/choose) for reproducible bugs and feature requests. Keep all shared examples deliberately synthetic.

The [support guide](SUPPORT.md), [Code of Conduct](CODE_OF_CONDUCT.md), [security policy](SECURITY.md) and [repository maintenance guide](dev/docs/repository-maintenance.md) describe the community channels and review process. Report vulnerabilities privately. Upstream contribution rights require a separate agreement; posting a pull request does not accept one.

## License

APEX is distributed under the [APEX Source Available License 1.1](LICENSE). The root license contains the complete public grant, eligibility and pricing rules. It is source available, not OSI-approved open source.

See the [licensing overview](LICENSING.md) and [pricing and eligibility](app/docs/legal/pricing.md). There is no per-operation, per-user, per-site or per-run metering. Commercial and contributor agreements require separate acceptance; [the legal documentation](app/docs/legal/README.md) explains the published document roles. Third-party components retain their own licenses. [Publication preparation](dev/docs/publication.md) records the release checks.
