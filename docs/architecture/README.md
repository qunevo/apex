# Architecture and design

Use the [operating guide](../implementation.md) and [executable data model](../data-model.md) for the current runtime. The documents below separate implemented contracts from the broader migration and research targets.

## Implemented contracts

| Document | Scope |
| --- | --- |
| [Declarative scheduling](declarative-scheduling.md) | Typed constraints, shared dispatch policies, probes and replay |
| [Combined improvement](combined-improvement.md) | Trainer/Plus portfolio and optional direct GA under one budget; open research items are labeled separately |
| [Direct schedule evolution](../direct-schedule-evolution.md) | Current genetic search, operator selection and native extension contracts |

## Decisions and proposals

| Document | Status |
| --- | --- |
| [ADR 0001: Rust core](decisions/0001-rust-core.md) | Accepted architectural direction; some target capabilities remain unimplemented |
| [Agentic scheduler](agentic-scheduler.md) | Original target architecture and research rationale, including future incremental evaluation and solver integration |
| [Core semantics](core-semantics.md) | Migration requirements and synthetic acceptance cases; not a current feature checklist |
| [Draft input model](input-model.md) | Earlier `3.0-draft.1` proposal and [design example](input-model-example.json); not executable input |
| [Dispatch-policy plan](dispatch-policy-plan.md) | Original pre-0.4 design; shared filtering is implemented, while indexed witness queries and general rollback remain proposals |

The [architecture gallery](diagrams.html), [draft input-model page](input-model.html) and [dispatch-policy diagrams](dispatch-policy.html) accompany those design proposals. Read their scope alongside the current contracts above; the galleries are not runtime documentation.

Historical v2 source paths identify removed files. Preserve the distinction between intended semantics, tested coverage and proposed implementation. See the [migration audit](../migration-audit.md) for evidence.
