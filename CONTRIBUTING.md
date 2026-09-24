# Contributing to APEX

Bug reports, documentation improvements and focused proposals are welcome. Start with the [README](readme.md), [architecture map](docs/architecture/README.md) and [repository conventions](AGENTS.md). Community participation follows our [Code of Conduct](CODE_OF_CONDUCT.md).

## License and contribution rights

APEX uses the [APEX Source Available License 1.1](LICENSE), not an OSI-approved open-source license. Source access does not grant unrestricted execution rights; running the scheduler or its tests requires an applicable entitlement under the license. The [licensing overview](LICENSING.md) explains the public grant.

Discuss substantial contributions with a maintainer before investing significant work. Upstream incorporation requires a separately accepted contributor agreement covering the necessary source and commercial licensing rights, as described in LICENSE section 5. Opening an issue, submitting a pull request or checking a template box does not accept such an agreement. Maintainers must resolve contribution rights before merging; no unfinished agreement template is supplied here.

## Choose the right channel

- Use [Discussions](https://github.com/qunevo/apex/discussions) for usage questions and early design ideas.
- Use the [issue forms](https://github.com/qunevo/apex/issues/new/choose) for reproducible bugs and concrete feature requests. Search existing issues first.
- Report vulnerabilities privately through the process in [SECURITY.md](SECURITY.md).

Include a minimal, deliberately synthetic example, the APEX version or commit, the operating system, the command or tool call, and the expected and actual result. Do not upload customer exports, production plans, credentials, personal information, private endpoints or local `.apex` stores. Redacting a customer export is not a substitute for making a synthetic example.

## Development setup

Install stable Rust and the native linker for your platform. Windows builds need Visual Studio C++ Build Tools. The integration tests also use Node.js 24 and Python 3. See the [operating guide](docs/implementation.md) for setup and runtime details.

Fork the repository, create a topic branch and keep each pull request focused. Maintainers can use a branch in this repository. From the checkout root:

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release
npm ci --ignore-scripts
npm run test:mcp
npm run test:http
```

These commands check formatting, run Clippy with warnings denied, run Rust tests and build the release executable using the lockfile. Rebuild before testing MCP, HTTP or viewer behavior. For viewer changes, install Chromium with `npx playwright install chromium`, start the rebuilt server with `target/release/apex serve` (`target/release/apex.exe serve` on Windows), and run `npm run test:viewer` and the relevant viewer tests from the architecture map.

## Change requirements

- Use English for code, documentation, examples and repository messages.
- Keep the scheduling core independent of real customers. Put optional adapters and domain behavior in customization modules.
- For scheduling semantics, update typed input, readiness checks, evaluation, objectives/heuristics and independent validation together. Include semantic, negative and corrupted-output cases. Never disable a hard rule to improve a score.
- Explain behavior changes in the relevant documentation. Separate measurements from goals and optimality claims.
- Keep generated output, local settings and secrets out of commits. Review the staged diff, not just the ignore rules.

## Pull requests and review

Describe the problem, resulting behavior, relevant tests and any limits. Link related issues and include synthetic reproduction steps when helpful. Documentation-only changes need link and factual checks; they do not need new scheduler tests.

CI checks formatting, linting, Rust tests/builds on Linux, Windows and macOS, MCP/HTTP/viewer integration on Linux, wiki export and documentation links, and the source publication scan. Keep checks passing and resolve review conversations. External fork workflows require maintainer approval before they run; approval to run CI is separate from approval to merge.

The default branch requires a pull request and an approving review from someone other than the author and most recent pusher. New reviewable commits dismiss earlier approvals. Administrators have no bypass entry. Maintainers also verify contribution rights and release scope before merging. See [repository maintenance](docs/repository-maintenance.md) for the configured checks and release process.
