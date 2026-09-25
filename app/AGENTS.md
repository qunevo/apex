# Application conventions

This directory is the complete, independent APEX source distribution. All commands and source paths below are relative to this directory, whether it is used alone or under the development repository's `app/`.

- Use English for source, documentation, examples and user-facing messages.
- Read [architecture](docs/architecture/README.md) before implementation. The executable model is `apex.v3.4`; canonical v3.1-v3.3 inputs use the same Rust runtime in `src/`.
- Keep the scheduling core independent of real customers. Optional domain behavior belongs in customization modules. Commit only deliberately synthetic examples, never customer exports or derived private data.
- For scheduling changes update typed input, readiness checks, evaluation, objectives/heuristics and independent validation together. Include semantic, negative and corrupted-output tests; never disable hard rules to improve a score.
- Run `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked` and `cargo build --locked --release`. Rebuild before MCP/browser verification. Product integration tests use the local npm manifests.
- Read [LICENSE](LICENSE), [LICENSING.md](LICENSING.md) and [pricing](docs/legal/pricing.md). Do not change the public grant, invent prices or thresholds, or claim OSI-approved open-source status. In the full development repository, the root LICENSE is canonical and this copy must remain byte-identical.
- Keep runtime `.apex` data, `.codex` configuration, `.env` files, generated reports and compiled output out of distribution. Read credentials and deployment-specific values from the environment.
- Use Bash for shell helpers, with LF endings, executable permissions and quoted paths. Build/runtime assets must resolve inside this application; do not depend on a parent checkout.
- Product workflows are in `skills/`. Report measured evidence separately from goals, optimality claims and historical migration parity.
