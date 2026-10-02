# APEX documentation

This index describes the runnable Rust implementation and its current, unversioned input schemas, which ship with the application release. Runtime capabilities and generated schemas are authoritative for the installed build; the migration audit records the scope of historical v2 comparisons.

## Use and integrate APEX

| Guide | Read it for |
| --- | --- |
| [Container start](containers.md) | One-command server startup, credentials and retained state |
| [Operating guide](implementation.md) | Build, run, verify and understand runtime limits |
| [Agent workflows](agent-workflows.md) | Data intake, planner conversations and extension responsibilities |
| [Agent integration](agent-integration.md) | MCP, HTTP, authentication, pagination and large imports |
| [Server configuration](server-configuration.md) | Enabled packages, per-call selection and scoped state |
| [Control platform](control-platform.md) | Tenants, versioned scenarios, background runs, approval, PostgreSQL, MCP/HTTP and the desktop display client |
| [Executable data model](data-model.md) | Units, routes, orders, conditional activities, locks, objectives and KPIs |
| [Material preparation](material-dispatch.md) | Existing-supply allocation, flexible routes and material modes |
| [Search and objectives](search-and-parity.md) | Q policies, XH, XT, replay and search limits |
| [Combined improvement](architecture/combined-improvement.md) | One shared budget for XH, XT and optional XE |
| [Direct schedule evolution](direct-schedule-evolution.md) | Priority chromosomes, genetic operators and extension contracts |
| [Declarative scheduling](architecture/declarative-scheduling.md) | Typed constraints, mandatory dispatch policies and explanations |
| [Synthetic customization](../customization/demo/model/KNOWLEDGE.md) | Domain knowledge, native hooks and validation requirements |

## Schemas and demo

Start with the [data model and planning workflow](data-model.md#from-source-data-to-a-schedule) for the two import paths, optional production expansion, lot creation and planning options. [Material preparation](material-dispatch.md#when-allocation-runs) explains when supply is allocated. The [schema roles and tool releases](data-model.md#schema-roles-and-tool-releases) describe the three input contracts and release boundary; the [schema directory README](../schemas/README.md) maps generated files to Rust types and gives regeneration commands.

The [factory showcase](https://github.com/qunevo/apex/tree/main/demo) presents the MES/Excel workflow. Its [customization package](../customization/demo/model/KNOWLEDGE.md) defines the application integration boundary and optional technical policies. The live factory adapter is still pending. Small synthetic inputs used to verify individual engine capabilities remain [test fixtures](../tests/fixtures).

## Work on the repository

[Current architecture](architecture/README.md) maps modules, the shared evaluation flow, service state and customization boundaries. Start there before changing implementation; its change map links responsibilities to tests.

Read the [contribution guide](https://github.com/qunevo/apex/blob/main/CONTRIBUTING.md) for setup, review and contribution rights. [Repository maintenance](https://github.com/qunevo/apex/blob/main/dev/docs/repository-maintenance.md) describes CI, dependency updates, GitHub protections and the release process. Use the [support guide](https://github.com/qunevo/apex/blob/main/SUPPORT.md) for questions and the [security policy](https://github.com/qunevo/apex/blob/main/SECURITY.md) for private vulnerability reports.

## Evidence and release

- [Migration audit](https://github.com/qunevo/apex/blob/main/dev/docs/migration-audit.md) consolidates v2 comparison evidence, implemented coverage and remaining boundaries.
- The [24 September 2026 benchmark](https://github.com/qunevo/apex/blob/main/benchmark/README.md) provides a frozen reproduction ZIP, public scientific instances and the original results. Later algorithm development is outside this baseline.
- [Licensing documents](legal/README.md) explain the canonical public license, pricing and eligibility.
- [Publication preparation](https://github.com/qunevo/apex/blob/main/dev/docs/publication.md) lists release checks.
- [Wiki publication](https://github.com/qunevo/apex/blob/main/dev/docs/wiki-publication.md) explains how reviewed documentation is published automatically.

The [data-model reference](data-model.md) covers the current scheduling problem and its input contract. The removed v2 source and execution harnesses cannot be run from this checkout; see [migration audit](https://github.com/qunevo/apex/blob/main/dev/docs/migration-audit.md).
