# APEX

APEX 0.6 is an experimental Rust scheduler for discrete production, built for agent tool use. It supports alternative workplans, conditional activity graphs, quantity/order expansion, existing-supply material allocation, native customization hooks and independent schedule validation. Fast Planner, evolutionary Q-policy training, UCT prefix search and direct schedule evolution share the same scheduling rules. A lean browser viewer displays saved plans.

The same 32 tools are available over MCP stdio, MCP Streamable HTTP and a JSON HTTP API with OpenAPI discovery. No model API key or external solver is required by the scheduler.

## Run locally

```text
cargo build --release
target/release/apex serve
```

On Windows use `target/release/apex.exe`, then open `http://127.0.0.1:8765`. The default viewer shows the schedule, KPIs, commitments and operation details. Planning changes run through your agent chat; `?mode=workbench` exposes development controls.

For the repository's Codex setup, run `scripts/setup-mcp.ps1` after building on Windows, then reconnect MCP in a trusted project. Other clients can use the [MCP configuration template](examples/codex-mcp.toml) and [agent integration guide](docs/agent-integration.md).

Use `schedule.create` for quick planning and `schedule.improve` for improvement under one shared budget. Improvement defaults to Trainer and Plus; an agent can explicitly enable the optional direct GA. See [combined improvement](docs/architecture/combined-improvement.md) for semantics and limits.

## Documentation

Start with the [documentation index](docs/README.md). The main references are:

- [Operating guide](docs/implementation.md): build, run, verify and understand capability limits.
- [Executable data model](docs/data-model.md): fields, units, relationships, commitments and KPIs.
- [Agent workflows](docs/agent-workflows.md) and [integration](docs/agent-integration.md): chat-led planning and tool access.
- [Material preparation](docs/material-dispatch.md), [search](docs/search-and-parity.md) and [direct evolution](docs/direct-schedule-evolution.md): current behavior and boundaries.
- [Architecture](docs/architecture/README.md): implemented contracts, design decisions and open proposals.
- [Migration audit](docs/migration-audit.md): v2 comparison, implemented coverage and remaining migration boundaries.

Runnable synthetic inputs include [production orders](examples/production-orders.json), [shift and material constraints](examples/shift-factory.json), [material chains](examples/chain-routing.json) and [dispatch policies](examples/dispatch-campaign.json). The [customization knowledge bundle](customizations/dummy_customer/KNOWLEDGE.md) describes the extension workflow.

The executable schema is `apex.v3.4`; canonical `apex.v3.1`, `apex.v3.2` and `apex.v3.3` inputs remain accepted. The earlier `3.0-draft.1` design is not executable input. Rust is the sole scheduling implementation; the [migration audit](docs/migration-audit.md) records the v2 source removal and historical comparisons. Tested coverage does not establish complete legacy parity, optimality or production readiness. Repository documentation and examples use English, and all fixtures are deliberately synthetic.

## License

APEX is distributed under the [APEX Source Available License 1.1](LICENSE). The root license contains the complete public grant, eligibility and pricing rules. It is source available, not OSI-approved open source.

See the [licensing overview](LICENSING.md) and [pricing and eligibility](docs/legal/pricing.md). There is no per-operation, per-user, per-site or per-run metering. Commercial and contributor agreements require separate acceptance; [the legal documentation](docs/legal/README.md) explains the published document roles. Third-party components retain their own licenses. [Publication preparation](docs/publication.md) records the release checks.
