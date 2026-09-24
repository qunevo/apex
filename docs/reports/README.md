# Implementation and migration evidence

Reports describe the build, fixtures and measurements recorded in each document. Historical counts, timings, tool catalogs and limitations are retained as evidence; they are not current operating instructions. Use the [operating guide](../implementation.md) and [data model](../data-model.md) for current behavior.

## Versioned reports

| Report | Recorded scope |
| --- | --- |
| [0.6: direct evolution](v0.6-evolution.md) | Direct GA, operators, material-mode search and native conditional alternatives |
| [0.5: model additions](v0.5-migration.md) | Route-aware allocation, chain urgency, conditional choices and KPIs |
| [0.4.1: combined improvement](v0.4.1-combined-improvement.md) | Shared Trainer/Plus budget and measured comparisons |
| [0.4: declarative scheduling](v0.4-declarative-scheduling.md) | Shared dispatch policies, replay and acceptance results |
| [0.3: search and parity](v0.3-search-parity.md) | Q evolution, UCT search, parallel evaluation and selected legacy comparisons |
| [0.2: relationships and UI](v0.2-parity-and-ui.md) | Routes, conditional graphs, quantity expansion, customization and transports |
| [0.1: initial Rust implementation](implementation-report.md) | Initial executable model and measured baseline |

## Audits and supporting evidence

| Document | Recorded scope |
| --- | --- |
| [Legacy baseline and source removal](legacy-baseline.md) | Original source inventory and what can no longer be rerun |
| [v2 parity audit](v2-parity-audit.md) | Historical gaps, follow-up implementation and unresolved parity boundaries |
| [Dispatch-policy audit](dispatch-policy-audit.md) | Pre-0.4 construction-filter gap |
| [Agent and material review](agent-material-review.md) | Earlier chat-led viewer, supply pegging and frozen-plan interactions |

JSON measurements and screenshots belong to their reports but remain local in the [initial source-only Git snapshot](../publication.md#initial-source-only-git-snapshot). Links to those raw artifacts require the local evidence files. Historical migration evidence remains here even when an old execution harness has been removed. Source removal and selected synthetic comparisons do not establish complete v2 equivalence, universal speed improvements or production readiness.

The separate [benchmark workstream](../../benchmark/README.md) has its own experiment definitions and results.
