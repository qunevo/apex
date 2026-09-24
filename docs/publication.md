# Source distribution

The repository contains the Rust core, viewer, tests, synthetic examples and customization, JSON schemas, portable agent skills, build/setup scripts, documentation and licensing files. Source publication does not certify production readiness or complete migration parity.

## Included and local-only files

`Cargo.lock` and `package-lock.json` pin dependencies and belong to the source distribution. The [migration audit](migration-audit.md) is the consolidated record of the former v2 implementation and its comparison with Rust. Historical per-release reports remain in Git history.

Credentials and environment files, `.apex` state, `.codex` configuration, installed dependencies, compiled output, caches, temporary files and runtime locks remain outside version control. `docs/reports/` is a local output directory for test and measurement artifacts; its only tracked file is the ignore policy that keeps the directory available in fresh checkouts.

The [benchmark publication](../benchmark/README.md) is explicitly included with its frozen source archive, public input datasets, completed experiment results and provenance. Its manifest records the exact published bytes. Local environments, executables, temporary runs and intermediate work remain excluded, as do the separate experiment plan and root benchmark/report helper scripts.

The publication scanner accepts the declared `benchmark/algorithm-9bc9dbf.zip` only when both manifest hashes match. It inspects bounded archive contents for unsafe paths, symlinks, local artifacts, credentials and workstation paths without extracting or executing them. Other ZIP files still require review. Run the benchmark's independent publication validator for the full dataset, result and schedule audit described in its guide.

## Release checks

1. Inspect the intended release contents, including any generated archive, for customer data, credentials and local artifacts. Run `python scripts/check_publication.py` against the distribution directory; its file/content checks do not prove that all data is synthetic.
2. Distribute the complete [APEX Source Available License 1.1](../LICENSE), preserve required third-party notices and verify authority over the actual release contents. The [legal document guide](legal/README.md) identifies the operative license and summaries; commercial and contributor agreements are handled separately.
3. Build the Rust release and run the required Rust, MCP/HTTP and viewer checks for that release. Keep architecture proposals distinct from implemented capabilities and measured results distinct from design goals.
4. Audit history, earlier releases and external archives separately when required. Deleting a file from the current branch does not remove it from Git history or revoke an exposed credential.
