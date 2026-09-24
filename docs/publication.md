# Publication preparation

The source tree is being prepared for publication. It is not a completed release.

The public Rust customization example is `customizations/dummy_customer`, generated from scratch. Examples and fixtures are synthetic. Deployment credentials must come from the environment. The legacy Python/Cython implementation, its integration and deployment files, and its execution harnesses have been [removed](reports/legacy-baseline.md).

## Initial source-only Git snapshot

The initial Git synchronization includes the Rust core and viewer, tests, synthetic examples and customization, generated JSON schemas, portable agent skills, build/setup scripts, documentation and licensing files. `Cargo.lock` and `package-lock.json` pin dependencies and belong to this source snapshot.

Local credentials and environment files, `.apex` state, `.codex` configuration, installed dependencies, compiled output, caches, temporary files and runtime locks are excluded. Historical Markdown reports remain as evidence; raw measurement JSON and screenshots remain local. Links from those reports to raw artifacts therefore require the local evidence files.

The benchmark directory, experiment plan and benchmark/report-generation scripts are deferred while that workstream is in progress. References to those files describe the complete workspace; they are not included in this initial source snapshot. `.gitignore` records these exclusions. Revisit the benchmark exclusions explicitly when that work is ready to publish.

## Remaining release gates

1. Remove residual compiled modules, bytecode, profiling data, learned models/vector stores and archives containing legacy data. Documentation cleanup does not establish that the entire directory is safe to publish. Inspect ignored files too. Run `python scripts/check_publication.py` to find mechanically detectable blockers.
2. Revoke or rotate any credential that previously appeared in a working copy or shared output. Removing a credential from source does not revoke it.
3. Audit Git history, releases, backups and external storage separately. This workspace contained no Git metadata, so history was not checked or rewritten.
4. Distribute the complete [APEX Source Available License 1.1](../LICENSE) with the release. It contains the public grant and its eligibility and pricing rules; the [commercial and contributor templates](legal/README.md) are separate preparation materials. Verify publishing authority and third-party redistribution terms, including benchmark archives, independently. Obtain any legal review appropriate to the distribution and commercial agreements; this publication checklist is not a legal opinion or a completed rights audit.
5. Build the Rust release, run its semantic and regression tests, and verify the applicable MCP/HTTP and viewer workflows before tagging a release. Architecture proposals and schema sketches must remain clearly labeled as drafts. Historical v2 comparison reports are retained evidence, not runnable release checks.

The publication check detects forbidden file classes, inline secrets, workstation paths and unexpected customization packages. It cannot prove that arbitrary prose or numerical data contains no real customer information. Review the release contents as well.

## Historical preparation checks

The original preparation pass checked Python source parsing, JSON parsing, notebook output removal, local Markdown links, synthetic request consistency and all five Mermaid diagrams. These are historical checks, not verification of the current release. Source-text scans found no remaining known customer names, deployment endpoints or inline credential matches. The draft v3 example was checked for references, resource units and quantities; it was not solved.

Before v2 was removed, the legacy synthetic request ran successfully under Python 3.11 using the existing local Cython binaries. Its former integration check verified four operations, expected durations, precedence, resource eligibility, shift bounds, non-overlap and absence of outbound callbacks. This historical result does not establish a reproducible fresh native build or general solver correctness.

The generic queue-weight hook was corrected to return the dictionary produced by the settings importer; it previously indexed that dictionary as if it were a list.
