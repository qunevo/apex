# Agent workflows, viewer and material-dispatch review

> Historical review of the pre-0.4 viewer and material workflow. Tool counts and limitations below describe that revision; see the [current operating guide](../implementation.md) and [material contract](../material-dispatch.md).

This follow-up makes routine planning chat-led, adds explicit existing-supply pegging and corrects two interactions between frozen schedules and conditional work. It supersedes the earlier UI description and the statement that no Rust material dispatcher exists. It does not assert full-system v2 parity.

## Delivered behavior

- Normal viewer: four KPI cards, three views (schedule, jobs/orders, commitments/rules), operation inspector and baseline comparison. Freeze shading, independent lock dimensions, sequence/block markers and running work remain visible. Scenario mutation, import, goal editing and search controls require the explicitly opened advanced workbench.
- Context handoff: the viewer can copy scenario/revision/schedule/baseline/operation references for a connected chat. It does not send messages or embed an LLM. Any tool-capable host can use the same 28 MCP/HTTP tools.
- Repository skills: `apex-data-intake`, `apex-planning`, `apex-extension`. One agent can switch roles. The coding skill requires exact objective evaluation, suitable proxies, held-out evidence and independent validation; it does not promise automatic universal metric-to-Q generation. External writeback remains deferred.
- `material.prepare`: a separate, revision-bound scenario with source-level quantity allocation, stock and receipt timing, producer/consumer links, started-work semantics and a paged audit report. Missing supply is reported; no replenishment or missing orders are created.

## Bugs found by the new viewer scenario

1. Order locks were inserted into the same temporal graph as product dependencies. A fixed sequence could therefore wait for an independent inspection even after the machine was free. Construction ordering and physical dependencies are now separate. Explicit production/material dependencies retain product-release timing.
2. The material ledger's monotone consumption timestamp delayed preparation on other resources. Reconstructing two frozen operations with equal main-start times in a different order could invalidate an otherwise feasible baseline. The frontier now applies at main start, which is when the ledger consumes material.

The production baseline can now be frozen in start/resource/sequence dimensions and reconstructed with Fast Planner. Both cases have regression tests; the sequence/inspection test also covers Trainer, Plus and rejection of an invalid reversed sequence. These changes can improve runtime KPIs relative to earlier recorded schedules; no new optimality claim follows.

## Verification

All checks below passed on the rebuilt release service:

| Check | Evidence |
| --- | --- |
| Rust | 74 passing tests; `cargo fmt --check`, Clippy with warnings denied; one explicitly ignored benchmark remains outside the normal suite |
| Material semantics | Seven tests covering multilevel BOM flows, stock/receipt/production splits, late receipts, reservations against competing consumers, cycles/shortages, route/mode ambiguity and started work |
| Independent material validation | A schedule valid without pegging is rejected against the pegged snapshot when it uses another consumer's stock |
| Service contract | Revision mismatch rejection, source scenario immutability, paged report and scheduling/validation after preparation |
| MCP | Official SDK tests passed through both stdio and Streamable HTTP, including search, custom goals and fixed decisions |
| Bulk intake | 100,000 tasks, 62.68 MB JSON, 4.12 seconds; largest chunk acknowledgement 130 bytes. This measures import, not scheduling |
| Normal viewer | [Browser report](viewer-plan-tests.json): no mutation controls, read-only inspection/navigation, agent context, commitments, comparison, reload, 800 px viewport and explicit workbench toggle |
| Advanced workbench | [Browser report](viewer-tests.json): existing planning/search/editing/freeze workflows still pass |
| Skills | All three pass the skill-creator structural validator; this is not an independent LLM behavior evaluation |
| Legacy material dispatcher | [Two executed comparisons](material-legacy-comparison.json): source quantities match the unmodified v2 dispatcher exactly on the synthetic cases |
| Legacy decoder | [Six executed comparisons](v0.3-legacy-comparison.json): five exact interval matches; the previously documented shift-rate correction remains the sole intentional difference |
| Legacy preservation | All 161 recorded reference source hashes unchanged |

## Measurements

[Material workflow measurements](material-workflow.json) use the release HTTP service. Each preparation timing includes the request, validation, transformation, serialization and saving the new scenario. Three repeats; medians:

| Synthetic profile | Preparation time | Tool response |
| --- | ---: | ---: |
| 1,000 independent operations consuming opening stock | 12.93 ms | 326 bytes |
| 10,000 independent operations consuming opening stock | 75.47 ms | 328 bytes |

The allocation rows remain in a paged artifact. These are not worst-case cyclic/deep BOM networks or proof of 100,000-operation scheduling throughput. The preparation guard limits candidate checks to 10 million; compilation and the full input remain in memory.

On the three-operation multilevel material fixture, Fast Planner took 0.10 ms, Plus 1.73 ms and Trainer 1.78 ms in this run. Plus/Trainer used 16 evaluations and two workers. All returned a validated makespan of 30 seconds. This small forced chain tests tool integration and correctness; it is not evidence of optimization quality on a difficult factory.

## Remaining boundaries

See [material semantics and the v2 audit](../material-dispatch.md) for precise differences. Pegging currently fixes a whole-workplan choice; joint search over alternative workplans and reassigned supply remains open. Identical-material machine modes remain flexible. Allocation is deterministic and conservative, not a global optimum or infeasibility proof. Stock/output reservations belonging to external demand must be normalized by the intake adapter.

Skills describe a tested tool/code workflow. They do not turn Markdown directly into enforced rules, supply credentials to arbitrary source systems, or authorize external writeback. The scheduling kernel remains a bounded constructive/search implementation without an external solver.
