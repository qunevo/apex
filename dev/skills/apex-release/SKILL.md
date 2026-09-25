---
name: apex-release
description: Prepare and execute an explicitly requested APEX release from dev to main, or a security hotfix, including versioning, PR checks, publication verification and synchronization back to dev.
---

# Release APEX

Read [developer workflow](../../docs/developer-workflow.md) and [versioning](../../docs/versioning.md). A request to publish/release the selected `dev` state authorizes preparing the PR, normal merging, source tag/release/wiki publication and the synchronization PR back to `dev`. A request only to prepare a release stops with reviewable local changes or a draft PR as requested. Reuse existing authorization; GitHub UI approval is optional unless the user requests it.

Run the compact preflight, fetch both branches and inspect outstanding work. Select and record the exact `dev` SHA; do not accidentally add later feature merges to the approved release. Check whether an existing release PR already covers it. Create an isolated `codex/release-<version>` branch from that SHA. Incorporate current `main` when needed, preserving ancestry. A security hotfix instead starts from current `main` on `codex/hotfix-<topic>` and contains only the reviewed fix.

Inspect the diff from `main`. Separate product changes from contributor maintenance. Infer a proposed patch/minor/major increment from the actual public contract and explain it briefly. Ask a targeted question only if compatibility or intended release scope is genuinely unresolved; do not guess away a breaking change. A major release, or transition to 1.0, needs clear user intent about the stable contract. Do not rename schema identifiers to match the product version.

For a product release, write complete notes with user-visible changes, compatibility/migration instructions and actual validation, then run:

```bash
python -B dev/scripts/release.py inspect --base origin/main
python -B dev/scripts/release.py prepare --base origin/main --bump patch --notes PATH_TO_NOTES
```

Use the selected bump type. Preparation updates only the product version, its own lockfile entry and versioned release notes; it does not upgrade dependencies. Commit those reviewed changes and validate with `release.py check --base origin/main --target main`. Pure maintenance can reach `main` without a product bump or a new tag.

Create or reuse a PR to `main` using the same PR tooling and SHA safeguards as [merge-dev](../apex-merge-dev/SKILL.md). Check the complete release diff, all required checks and resolved conversations before merging. Attach the PR. If `main` changed meanwhile, reconcile and recompute the candidate version before running CI again. Never bypass branch rules or publish an untested replacement commit.

After the merge, wait for `main` CI and `Publish`. Verify the immutable tag points at the merged SHA, the release notes match, and the wiki reports that revision. Re-running publication must reuse the same version. Report delayed or failed publication explicitly; do not increment again to hide it. An older queued run cannot replace a newer wiki. Do not add executable/container/package publishing to the source release scope.

Open and merge a normal `main` to `dev` synchronization PR after its required checks pass. Include hotfixes and the version commit; retain newer development work. Attach this PR too. Report the product version, release and both PR links, publication state and remaining blockers. Delete only this task's merged temporary branch when appropriate; never delete `dev` or another task's checkout.
