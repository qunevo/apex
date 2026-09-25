# Synthetic scheduling policies

All examples are fictional. `rules.json` can be supplied through a `scenario.patch` of kind `rules` for `examples/demo.json`.

## Urgency completion

Every task has a nonnegative `urgency` attribute. Minimize the sum of `urgency * product_ready_time` at objective priority zero. This favors completing high-urgency work earlier. It does not convert a soft due date into a deadline.

The engine compiles the same declaration into a named objective metric and an automatic `attribute:ID` Q using urgency / estimated work. The trainer evaluates this signal alongside its other strategies and selects by the complete lexicographic score. A signal is a heuristic, not a guarantee of optimality.

## Planned release

Task `T000000` may start no earlier than second 120 and must release its product by second 86400. The evaluator and independent validator enforce both bounds.

## Developing a new rule with an agent

1. Write the requirement, quantities, time basis, hard/soft meaning and counterexamples here.
2. Prefer existing typed rules, activities, modes, calendars, dependencies and locks.
3. If a new executable rule is needed, add its typed representation to `src/model.rs`, readiness checks to `compile.rs`, evaluation and dispatch semantics to `rules.rs`/`xg.rs`, and final-assignment checks to `validate.rs`.
4. Add positive, negative, interaction and corrupted-output tests. For a new objective, test that reported scores and training selection use the same definition; compare the matching heuristic against actual objectives on varied instances and a held-out set.
5. Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`. Rebuild with `cargo build --release` and restart the local tool process.
6. Fork a scenario, activate the rule, plan and compare. Keep the previous code/input version available for reproduction.

## Native extension example

[`policy.rs`](policy.rs) implements the linked `dummy_customer@1` policy. Select it through `Problem.customization` or a scenario patch of kind `customization`. It penalizes returning to a family after intervening work, adds an urgency dispatch adjustment and reports a custom weighted completion metric. All data and logic are synthetic.

The `Customization` trait provides `compile`, `sequence`, `dispatch_rank`, `objectives`, `queue_definitions`, `queue_value`, `metrics` and `validate`. Sequence hooks receive the complete selected primary-resource sequence and return task decorations with pre/post activities and separate penalties. Native metric hooks declare value, weight and objective priority. Hard validation runs for every proposed result. Independent revalidation rebuilds sequence decorations and recomputes custom scores; it must reject altered activities or metrics. See [customization tests](../../tests/customization.rs) and [parity tests](../../tests/parity.rs).

Hooks must be deterministic for the pinned input, sequence and extension version. Do not depend on wall-clock time, mutable external state or network responses during evaluation. Give all added activities stable IDs, declare occupancy explicitly and return diagnostics for unsupported semantics. If a new hard validator rejects a candidate, the trainer may try another candidate; it must never relax the rule to obtain a score.

Register a new version in `src/extensions.rs` and rebuild after code changes. The registry is statically linked: source text from chat or Markdown is never executed by runtime tools. Use an existing typed rule for changes that do not need native code. A new objective needs a matching dispatch signal or search neighborhood and quality tests; only the already supported attribute objective derives its signal automatically.

Native hooks implement `Send + Sync`. Declare selected metric names through `objectives`; JSON cannot register an evaluator by spelling a new name. Declare explicit Q proxies through `queue_definitions` and calculate them in `queue_value`. Record quality measurements and counterexamples before changing defaults. See [search semantics](../../docs/search-and-parity.md).

## Declarative campaign example (implemented)

[`dispatch-campaign.json`](../../examples/dispatch-campaign.json) keeps family A for 21,600 productive main-operation seconds on M0 when a matching feasible continuation remains. Initial historical credit is zero, start-fixed work is explicitly exempt, and no-match permits a switch. Pauses and cleaning do not count as productive time. The example is a construction policy, not a universal final-schedule campaign invariant. Inspect the actual KPI trade-off against a scenario without the policy.

Native `filter_candidates` hooks and the required prefix-decoration contract are documented in the [language reference](../../docs/architecture/declarative-scheduling.md); the existing synthetic sequence decorator is declared prefix-safe. Tests in `tests/dispatch.rs` exercise all strategies, both searches, replay corruption and placement interactions.

## Genetic proposals and conditional catalogs

APEX 0.6 adds `xe_operators` / `evolve` for deterministic mutation or crossover proposals and `conditional_modes` for advance names of sequence-generated alternatives. See [the contract](../../docs/direct-schedule-evolution.md). The synthetic acceptance examples are in `tests/xe.rs`; this knowledge note does not register another runtime operator. Explicit locks remain protected and independent validation remains authoritative.
