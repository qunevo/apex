# APEX versioning

`app/Cargo.toml` is the maintained source of the product version. The product is the complete application distribution: scheduling components, CLI, MCP/HTTP interfaces, embedded viewer, supported formats, product skills and deployment helpers. It has one release stream. The application derives its version from `CARGO_PKG_VERSION`; API clients and generated reports should use that value. The package's own `Cargo.lock` entry is a generated mirror, not a second policy source.

## Compatibility determines the increment

Use [Semantic Versioning](https://semver.org/): compatible corrections are patch releases, compatible new functionality is minor, and incompatible public-contract changes are major. Public contracts include accepted input, output/API structure, documented scheduling semantics and deployment behavior. A large internal refactor does not imply a major release; a small incompatible field change can. Explicitly assess changes to planning constraints/objectives, not just JSON shape. A corrected heuristic can produce different valid plans without promising identical optimal results.

While APEX remains below 1.0, classify corrections as patch and feature or breaking development changes as minor; identify breaking changes and migration requirements in the notes. Transition to 1.0 is a deliberate decision about the supported public contract, not an incidental automated bump. Never infer production readiness or optimality from a version number.

Keep schema/protocol identifiers separate from product release numbers. A file's format identifies how to interpret it; its producer version identifies the implementation that created it. Multiple product releases may support the same format and older formats. Preserve existing `apex.v3.1` through `apex.v3.4` contracts until an explicit compatibility change is implemented and tested. Do not update format strings merely because Cargo's version changed. Data-format changes may affect only import/serialization or may require scheduling and validation changes, depending on semantics.

## Release and maintenance scope

Product source, runtime dependencies, schemas, product skills, deployment helpers and licenses are release-relevant. The checker conservatively recognizes these paths. Contributor-only `dev/`, `.github/` and `.agents/` changes do not by themselves require a product version. Neither do application documentation, tests or private npm test dependencies. A maintenance commit can reach main without a product release. The existing version tag continues to identify its immutable source snapshot; it is never moved to a later maintenance commit.

Path classification is a guard, not a semantic compatibility analysis. The agent must inspect actual changes and select the increment. Changes outside the usual boundaries that alter the shipped product still require an intentional release. Current guides avoid hardcoded "current APEX x.y.z" claims. Historical reports, release notes, dependency versions and schema/protocol identifiers retain their meaningful numbers.

## Prepare before merging

The release skill creates a clean `codex/release-*` or `codex/hotfix-*` candidate. `release.py prepare --base origin/main --bump patch|minor|major --notes FILE` increments the manifest and its lockfile mirror together and writes `dev/releases/<version>.md`. Notes describe changes, compatibility/migration needs and measured verification. Commit the prepared result before checking the PR. No dependency refresh or global text substitution is part of version preparation.

CI checks manifest/lockfile agreement on both branches. Main product changes require a one-step version increment and nonempty notes. Schema and negative compatibility tests remain required by the scheduling change map. The publication job creates `v<version>` only after successful main CI; retries verify and reuse that tag/release. Existing tags pointing elsewhere or conflicting notes fail rather than being overwritten. Development builds can additionally be identified by their Git SHA; a release number alone does not uniquely identify uncommitted or unreleased work.
