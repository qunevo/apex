# Generated input schemas

For schema roles, optional import steps and lot creation, read the [data model and planning workflow](../docs/data-model.md#from-source-data-to-a-schedule). [Material preparation](../docs/material-dispatch.md) explains allocation and when it runs. Both guides are maintained in `app/docs` and included in the public wiki.

## Files and sources

| File | Rust root type | Schema model selector |
| --- | --- | --- |
| [scheduling-problem.schema.json](scheduling-problem.schema.json) | [`Problem`](../core/model.rs) | `problem` (default) |
| [production-orders.schema.json](production-orders.schema.json) | [`ProductionInput`](../core/production.rs) | `production` |
| [planning-options.schema.json](planning-options.schema.json) | [`Options`](../core/model.rs) | `options` |

These are generated snapshots of the current input types. Edit the Rust types, then regenerate the affected files; do not maintain schema fields by hand. See the [application release contract](../docs/data-model.md#schema-roles-and-tool-releases) for versioning and compatibility.

## Regeneration

Run from the application root (`app/` in the development repository):

```text
cargo build --locked --release
target/release/apex schema --out schemas/scheduling-problem.schema.json
target/release/apex schema --model production --out schemas/production-orders.schema.json
target/release/apex schema --model options --out schemas/planning-options.schema.json
```

Use `target/release/apex.exe` on Windows. Review the generated diff and run the checks in the [architecture change map](../docs/architecture/README.md#change-map-for-coding-agents). The [input-contract tests](../tests/input_contract.rs) compare all three snapshots to the generated Rust schemas.

The `schema.get` tool exposes the same model selectors and can return a named definition for bounded agent discovery; see [agent integration](../docs/agent-integration.md).
