# APEX documentation

This index describes the runnable Rust implementation, version **0.6.0**, with executable schema **`apex.v3.4`**. Canonical v3.1, v3.2 and v3.3 inputs remain accepted. Runtime capabilities and generated schemas are authoritative for the installed build; the migration audit records the scope of historical v2 comparisons.

## Use and integrate APEX

| Guide | Read it for |
| --- | --- |
| [Operating guide](implementation.md) | Build, run, verify and understand runtime limits |
| [Agent workflows](agent-workflows.md) | Data intake, planner conversations and extension responsibilities |
| [Agent integration](agent-integration.md) | MCP, HTTP, authentication, pagination and large imports |
| [Executable data model](data-model.md) | Units, routes, orders, conditional activities, locks, objectives and KPIs |
| [Material preparation](material-dispatch.md) | Existing-supply allocation, flexible routes and material modes |
| [Search and objectives](search-and-parity.md) | Q policies, Trainer, Plus, replay and search limits |
| [Combined improvement](architecture/combined-improvement.md) | One shared budget for Trainer, Plus and optional direct GA |
| [Direct schedule evolution](direct-schedule-evolution.md) | Priority chromosomes, genetic operators and extension contracts |
| [Declarative scheduling](architecture/declarative-scheduling.md) | Typed constraints, mandatory dispatch policies and explanations |
| [Synthetic customization](../customizations/dummy_customer/KNOWLEDGE.md) | Domain knowledge, native hooks and validation requirements |

## Schemas and examples

The current generated schemas cover the [canonical problem](../schemas/apex.v3.4.json), [production templates](../schemas/production.v3.4.json) and [planning options](../schemas/options.v3.4.json). Only these current snapshots are kept in the repository. The CLI and `schema.get` generate schemas from Rust types; acceptance of older canonical inputs is enforced by the runtime and does not depend on historical schema files.

| Synthetic example | Focus |
| --- | --- |
| [Demo](../examples/demo.json) | Basic canonical scheduling input |
| [Production orders](../examples/production-orders.json) | Quantities, lot expansion and selectable workplans |
| [Shift factory](../examples/shift-factory.json) | Calendars, conditional work and material |
| [Material chains](../examples/chain-routing.json) | Flexible routing, allocation and downstream urgency |
| [Dispatch campaign](../examples/dispatch-campaign.json) | Mandatory construction policies |
| [Release times](../examples/improve-release-times.json) | Combined-improvement scenario |

## Work on the repository

[Current architecture](architecture/README.md) maps modules, the shared evaluation flow, service state and customization boundaries. Start there before changing implementation; its change map links responsibilities to tests.

## Evidence and release

- [Migration audit](migration-audit.md) consolidates v2 comparison evidence, implemented coverage and remaining boundaries.
- [Benchmark guide](../benchmark/README.md) and [experiment plan](benchmark-experiment-plan.md) cover the separate benchmark workstream.
- [Licensing documents](legal/README.md) explain the canonical public license, pricing and eligibility.
- [Publication preparation](publication.md) lists release checks.

The [data-model reference](data-model.md) covers the current format and accepted canonical input versions. The removed v2 source and execution harnesses cannot be run from this checkout; see [migration audit](migration-audit.md).
