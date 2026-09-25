# APEX

[![CI](https://github.com/qunevo/apex/actions/workflows/ci.yml/badge.svg)](https://github.com/qunevo/apex/actions/workflows/ci.yml)

APEX is an experimental Rust scheduler for discrete production, built for agent tool use. It supports alternative workplans, conditional activity graphs, quantity/order expansion, existing-supply material allocation, native customization hooks and independent schedule validation. XG construction, XH hypersearch, XT tree search and XE schedule evolution share the same scheduling rules. A lean browser viewer displays saved plans.

The same 32 tools are available over MCP stdio, MCP Streamable HTTP and a JSON HTTP API with OpenAPI discovery. No model API key or external solver is required by the scheduler.

## Repository areas

- [app/](app/README.md) is the independent customer source distribution, with its own manifests, lockfiles, license, product skills, deployment helpers, examples and tests. Build and run it without the parent repository. Keep local data and generated files out of distributed copies.
- [dev/](dev/docs/repository-layout.md) contains contributor documentation, fixture generation, wiki/publication tooling and standalone verification.
- [.agents/skills/](.agents/skills) contains repository development workflows.
- [.github/](.github) contains CI and contribution infrastructure.
- [benchmark/](benchmark/README.md) contains the unchanged frozen reproduction package.

## Run locally

```text
cd app
cargo build --release
target/release/apex serve
```

On Windows use `target/release/apex.exe`, then open `http://127.0.0.1:8765`. The default viewer shows the schedule, KPIs, commitments and operation details. Planning changes run through your agent chat; `?mode=workbench` exposes development controls.

The [Bash helpers](app/docs/implementation.md#bash-helpers) cover build/checks, viewer startup and local MCP setup on Linux, macOS and Git Bash on Windows. For the repository's Codex setup, run `bash deploy/setup-mcp.sh` after building, then reconnect MCP in a trusted project. Other clients can use the [MCP configuration template](app/examples/codex-mcp.toml) and [agent integration guide](app/docs/agent-integration.md).

Use `schedule.create` for quick planning and `schedule.improve` for improvement under one shared budget. Improvement defaults to XH and XT; an agent can explicitly enable the optional XE phase. See [combined improvement](app/docs/architecture/combined-improvement.md) for semantics and limits.

## Documentation

Start with the [documentation index](app/docs/README.md). The main references are:

- [Operating guide](app/docs/implementation.md): build, run, verify and understand capability limits.
- [Executable data model](app/docs/data-model.md): fields, units, relationships, commitments and KPIs.
- [Agent workflows](app/docs/agent-workflows.md) and [integration](app/docs/agent-integration.md): chat-led planning and tool access.
- [Material preparation](app/docs/material-dispatch.md), [search](app/docs/search-and-parity.md) and [direct evolution](app/docs/direct-schedule-evolution.md): current behavior and boundaries.
- [Architecture](app/docs/architecture/README.md): current modules, evaluation flow, state boundaries and change map for coding agents.
- [Migration audit](dev/docs/migration-audit.md): v2 comparison, implemented coverage and remaining migration boundaries.
- [Benchmark of 24 September 2026](benchmark/README.md): frozen sources, scientific instances, results and standalone reproduction instructions in one ZIP.

Runnable synthetic inputs include [production orders](app/examples/production-orders.json), [shift and material constraints](app/examples/shift-factory.json), [material chains](app/examples/chain-routing.json) and [dispatch policies](app/examples/dispatch-campaign.json). The [customization knowledge bundle](app/customizations/dummy_customer/KNOWLEDGE.md) describes the extension workflow.

The executable schema is `apex.v3.4`; canonical `apex.v3.1`, `apex.v3.2` and `apex.v3.3` inputs remain accepted by the same Rust runtime. Rust is the sole scheduling implementation; the [migration audit](dev/docs/migration-audit.md) records the v2 source removal and historical comparisons. Tested coverage does not establish complete legacy parity, optimality or production readiness. Repository documentation and examples use English, and all fixtures are deliberately synthetic.

## Contributing and support

Read the [contribution guide](CONTRIBUTING.md) before starting a change. Use [Discussions](https://github.com/qunevo/apex/discussions) for questions and [issue forms](https://github.com/qunevo/apex/issues/new/choose) for reproducible bugs and feature requests. Keep all shared examples deliberately synthetic.

The [support guide](SUPPORT.md), [Code of Conduct](CODE_OF_CONDUCT.md), [security policy](SECURITY.md) and [repository maintenance guide](dev/docs/repository-maintenance.md) describe the community channels and review process. Report vulnerabilities privately. Upstream contribution rights require a separate agreement; posting a pull request does not accept one.

## License

APEX is distributed under the [APEX Source Available License 1.1](LICENSE). The root license contains the complete public grant, eligibility and pricing rules. It is source available, not OSI-approved open source.

See the [licensing overview](LICENSING.md) and [pricing and eligibility](app/docs/legal/pricing.md). There is no per-operation, per-user, per-site or per-run metering. Commercial and contributor agreements require separate acceptance; [the legal documentation](app/docs/legal/README.md) explains the published document roles. Third-party components retain their own licenses. [Publication preparation](dev/docs/publication.md) records the release checks.
