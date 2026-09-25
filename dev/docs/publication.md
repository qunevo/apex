# Source distribution

The repository contains the Rust core, viewer, tests, synthetic examples and customization, JSON schemas, portable agent skills, build/setup scripts, documentation and licensing files. Source publication does not certify production readiness or complete migration parity.

## Included and local-only files

`app/Cargo.lock` and `app/package-lock.json` pin dependencies and belong to the source distribution. The [migration audit](migration-audit.md) is the consolidated record of the former v2 implementation and its comparison with Rust. Historical per-release reports remain in Git history.

Credentials and environment files, `.apex` state, `.codex` configuration, installed dependencies, compiled output, caches, temporary files and runtime locks remain outside version control. `app/docs/reports/` is a local output directory for test and measurement artifacts; its only tracked file is the ignore policy that keeps the directory available in fresh checkouts.

The [benchmark publication](../../benchmark/README.md) is distributed as one ZIP beside its reader's guide. It contains the frozen source archive, public scientific instances, completed experiment results, analysis and reproduction scripts, dependency locks and provenance. The original publication manifest and result bytes are preserved inside the ZIP. Local environments, executables, temporary runs and intermediate work remain excluded, as do the separate experiment plan and historical benchmark/report helper scripts.

The publication scanner accepts `benchmark/apex-benchmark-2026-09-24.zip` only at the reviewed SHA-256 digest pinned in the scanner, with a 100 MiB size limit. This binds every member, including the nested algorithm ZIP, to the audited package without extracting or executing it during CI. A changed archive requires content review and a new digest; unrelated ZIPs still require review. The scanner retains its bounded frozen-source inspection for auditing the expanded benchmark layout. Follow the reader's guide to extract into a separate directory and run the independent dataset, result and schedule audit.

## Release checks

1. Inspect the intended release contents, including any generated archive, for customer data, credentials and local artifacts. Run `python dev/scripts/check_publication.py` against the distribution directory; its file/content checks do not prove that all data is synthetic.
2. Distribute the complete [APEX Source Available License 1.1](../../LICENSE), preserve required third-party notices and verify authority over the actual release contents. The [legal document guide](../../app/docs/legal/README.md) identifies the operative license and summaries; commercial and contributor agreements are handled separately.
3. Build the Rust release and run the required Rust, MCP/HTTP and viewer checks for that release. Keep architecture proposals distinct from implemented capabilities and measured results distinct from design goals.
4. Audit history, earlier releases and external archives separately when required. Deleting a file from the current branch does not remove it from Git history or revoke an exposed credential.
