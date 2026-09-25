# APEX

[![CI](https://github.com/qunevo/apex/actions/workflows/ci.yml/badge.svg)](https://github.com/qunevo/apex/actions/workflows/ci.yml)

APEX is an experimental Rust scheduler for discrete production, built for agent tool use. It supports alternative workplans, conditional activity graphs, quantity/order expansion, existing-supply material allocation, native customization hooks and independent schedule validation. XG construction, XH hypersearch, XT tree search and XE schedule evolution share the same scheduling rules. A lean browser viewer displays saved plans.

The same 32 tools are available over MCP stdio, MCP Streamable HTTP and a JSON HTTP API with OpenAPI discovery. No model API key or external solver is required by the scheduler.

## Run locally

This directory is an independent source distribution. Copy its source files alone; no parent repository is needed. Run the commands below here (`cd app` when using the full checkout). Rust and a native linker are build prerequisites. Do not copy local runtime data or installed/build output.

```text
cargo build --release
target/release/apex serve
```

On Windows use `target/release/apex.exe`, then open `http://127.0.0.1:8765`. The default viewer shows the schedule, KPIs, commitments and operation details. Planning changes run through your agent chat; `?mode=workbench` exposes development controls.

The [Bash helpers](docs/implementation.md#bash-helpers) cover build/checks, viewer startup and local MCP setup on Linux, macOS and Git Bash on Windows. For this installation's Codex setup, run `bash deploy/setup-mcp.sh` after building, then reconnect MCP in a trusted project. Other clients can use the [MCP configuration template](examples/codex-mcp.toml) and [agent integration guide](docs/agent-integration.md).

Use `schedule.create` for quick planning and `schedule.improve` for improvement under one shared budget. Improvement defaults to XH and XT; an agent can explicitly enable the optional XE phase. See [combined improvement](docs/architecture/combined-improvement.md) for semantics and limits.

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

Runnable synthetic inputs include [production orders](examples/production-orders.json), [shift and material constraints](examples/shift-factory.json), [material chains](examples/chain-routing.json) and [dispatch policies](examples/dispatch-campaign.json). The [customization knowledge bundle](customizations/dummy_customer/KNOWLEDGE.md) describes the extension workflow.

The executable schema is `apex.v3.4`; canonical `apex.v3.1`, `apex.v3.2` and `apex.v3.3` inputs remain accepted by the same Rust runtime. Rust is the sole scheduling implementation; the [migration audit](https://github.com/qunevo/apex/blob/main/dev/docs/migration-audit.md) records the v2 source removal and historical comparisons. Tested coverage does not establish complete legacy parity, optimality or production readiness. Repository documentation and examples use English, and all fixtures are deliberately synthetic.

## Contributing and support

Read the [contribution guide](https://github.com/qunevo/apex/blob/main/CONTRIBUTING.md) before starting a change. Use [Discussions](https://github.com/qunevo/apex/discussions) for questions and [issue forms](https://github.com/qunevo/apex/issues/new/choose) for reproducible bugs and feature requests. Keep all shared examples deliberately synthetic.

The [support guide](https://github.com/qunevo/apex/blob/main/SUPPORT.md), [Code of Conduct](https://github.com/qunevo/apex/blob/main/CODE_OF_CONDUCT.md), [security policy](https://github.com/qunevo/apex/blob/main/SECURITY.md) and [repository maintenance guide](https://github.com/qunevo/apex/blob/main/dev/docs/repository-maintenance.md) describe the community channels and review process. Report vulnerabilities privately. Upstream contribution rights require a separate agreement; posting a pull request does not accept one.

## License

APEX is distributed under the [APEX Source Available License 1.1](LICENSE). The included license contains the complete public grant, eligibility and pricing rules. It is source available, not OSI-approved open source.

See the [licensing overview](LICENSING.md) and [pricing and eligibility](docs/legal/pricing.md). There is no per-operation, per-user, per-site or per-run metering. Commercial and contributor agreements require separate acceptance; [the legal documentation](docs/legal/README.md) explains the published document roles. Third-party components retain their own licenses. [Publication preparation](https://github.com/qunevo/apex/blob/main/dev/docs/publication.md) records the release checks.
