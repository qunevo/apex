# Material preparation: scheduling, not MRP

APEX separates quantity/lot expansion, material allocation and finite-capacity scheduling. The `material.prepare` tool allocates **existing** usable stock, confirmed receipts and outputs of supplied production operations. It never generates procurement proposals, forecasts, replenishment quantities or missing manufacturing orders.

## Mapping

- In `ProductionInput`, workplan task and mode material maps are **per unit**. Expansion multiplies by lot quantity and template task quantity. `$job` gives job-local intermediate item IDs when needed.
- In canonical `Problem`, `consume` and `produce` contain **total quantities for that operation**. Modes may override them. Do not infer yield/output from processing time or automatically treat a job's item label as a supply quantity.
- Map released-order BOM requirements onto the consuming operations. Map actual outputs onto their producing operations. A multilevel BOM is represented by successive output/input relationships among the supplied orders. Material-only master BOMs without released production orders do not cause order creation.
- `inventory` must be usable opening stock after external reservations. `receipts` are confirmed available quantities/times, not hypothetical purchase proposals. Already allocated external production output must be represented as unavailable/net supply by the source adapter; there is no separate external-reservation object yet.
- Missing processing times remain blocking input diagnostics. Missing material causes diagnostics in the preparation preview or blocks a fully selected allocation. Units must be normalized before import.

## Tool workflow

1. Import canonical tasks or expand production templates.
2. Leave route and differing-material main-mode alternatives free, or supply `options.route_choices` and `options.mode_choices` to pin them. Fixed and started choices cannot be overridden.
3. Call `material.prepare` with `scenario_id`, `expected_revision` and optional `options`.
4. The result gives a **new scenario**, allocation count, added dependency count and `report_id`. For flexible routes or material modes, this report is a preview; inspect `allocation_preview` and `preview_diagnostics`, then page actual saved allocations through `model.page` / `materials`. The original revision is unchanged. Page `artifact.read` with `pointer: "/allocations"` to inspect consumer, item, quantity, stock/receipt/producer source, receipt index and timing.
5. Use `schedule.create` or `schedule.improve` on the prepared scenario; validate and compare as usual. Flexible models reallocate supply after each actual route and material-mode selection, while material-identical machine modes retain their resource choices. New internal tokens ensure another dispatch order cannot steal a quantity reserved to a particular consumer.

When all routes and material-mode decisions are fixed, preparation produces a materialized snapshot in ordinary canonical JSON: physical material maps plus internal `@apex/pegging/…` balances and precedence edges. Both decoder and independent validator enforce them. These internal names are not new physical items. Preparing an already prepared scenario is rejected. Material/routing edits to a materialized snapshot require preparing the **original** input again; never manually edit the internal tokens. Flexible models carry `material_policy: "reallocate_routes"` and reallocate during evaluation after typed scenario changes. Goals and calendars may be varied on either kind of prepared scenario.

## Policy and limits

Readiness considers explicit predecessors and currently allocatable material. Ready tasks are examined by derived chain due date, then descending chain priority, then stable input order. Pending production is exhausted before falling back to late receipts. Multiple sources may supply one consumer and one producer may supply several consumers. No unbounded recursive BOM walk is used; cyclic and blocked networks produce diagnostics. Due dates are not silently rewritten upstream.

The policy is deterministic and conservative. It is not an optimal global pegging algorithm and may require another allocation/routing choice for a difficult instance. `MATERIAL_UNRESOLVED` reports shortages or blocked/cyclic supply; it does not prove global infeasibility. Preparation is a whole-snapshot operation with a 10-million candidate-check guard, not a streaming optimizer. Large imports still use files/chunks and bounded reports, while the model remains in server memory.

Running operation input is treated as consumed before the snapshot, matching current core semantics. Its output becomes available only after remaining work and required post-work. Partial output already in stock must be represented by the adapter without double-counting the future output.

## Migration boundary

The [migration audit](https://github.com/qunevo/apex/blob/main/dev/docs/migration-audit.md) records the comparison with the former v2 material dispatcher, intentional semantic differences and remaining acceptance work. Existing-supply allocation does not establish complete legacy preprocessing or search equivalence.
