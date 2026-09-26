# Agent development and release workflow

`main` is the default branch and contains released application states plus contributor maintenance. `dev` integrates reviewed features. Feature work starts on `codex/<topic>` from current `origin/dev`; release preparation snapshots `dev` onto `codex/release-<version>`. A security hotfix starts from `main` on `codex/hotfix-<topic>`. Both long-lived branches require PRs, resolved review conversations and `Required checks` on an up-to-date branch. No independent approval is required by this repository's maintainer process; user authorization in the task can drive the complete normal merge flow.

## Branch lifecycle

Only `main` and `dev` are permanent. A merge request includes cleanup of the task's successfully merged temporary branches; a merged PR does not by itself mean its local branch was deleted.

| Branch | Integration | Cleanup |
| --- | --- | --- |
| Feature or dependency branch | PR to `dev` | GitHub deletes the remote head after merge; remove the merged local branch when the task finishes. |
| Release or hotfix branch | PR to `main`, then publication and `main` to `dev` synchronization | GitHub deletes the remote temporary head after merge; remove its local branch after publication and synchronization succeed. |
| `main`, `dev` | Normal merge PRs between the long-lived branches | Retain both; deletion protection also prevents automatic PR-head deletion. |

Enable GitHub's [automatic head-branch deletion](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-the-automatic-deletion-of-branches) only with deletion protection active for both permanent branches. `configure_github.py` establishes that protection before enabling cleanup. Verify the merged temporary remote head is gone; if it remains, delete only the reviewed branch after checking that its tip has not advanced beyond the merged work. Never delete a head that was reused for new commits or another open PR.

For local cleanup, fetch and prune remote refs, verify the exact task branch is fully contained in its integration target, and use `git branch -d <branch>`. If it is checked out in this task's clean worktree, switch that worktree to a detached merged revision first. Retain its files unless directory removal was separately requested. Do not switch another active task's checkout or delete a branch merely because `git branch --merged` lists it: a newly created feature branch can have no unique commits yet. Preserve unfinished work and report any deferred cleanup. Historical cleanup may include explicitly identified completed checkouts, but must still verify ancestry and preserve local files.

After a release and synchronization, `main` and `dev` must have identical tracked content unless newer feature work has entered `dev`. Check `git diff --exit-code origin/main origin/dev` for equality and verify that the released `main` commit is an ancestor of `dev`. Normal synchronization creates an additional merge commit on `dev`, so different commit IDs or an ahead count alone do not indicate different product content. Do not force-push or reset either permanent branch to make their IDs equal. Preserve and report newer development changes instead of erasing them.

## Skills and user decisions

Canonical contributor instructions live under [dev/skills](../skills). Tiny generated entries under [.agents/skills](../../.agents/skills) provide discovery without maintaining two copies of each procedure. Run `python -B dev/scripts/sync_skills.py` after editing skill metadata; CI checks for drift. Product/customer skills remain in `app/skills` and are independent of these developer workflows.

| Request | Skill | Decision boundary |
| --- | --- | --- |
| Turn an idea into a feature | [apex-development](../skills/apex-development/SKILL.md) | Investigate and clarify, present a concrete plan, obtain implementation approval, then implement |
| Integrate completed work | [apex-merge-dev](../skills/apex-merge-dev/SKILL.md) | An explicit merge request authorizes PR creation, required corrections and merging to dev |
| Publish dev or a hotfix | [apex-release](../skills/apex-release/SKILL.md) | An explicit release request authorizes the release PR, normal merge, publication and synchronization |
| Reduce source/context complexity | [apex-refactor](../skills/apex-refactor/SKILL.md) | Audit and propose responsibility-based batches; implement the approved batch |
| Reconcile documentation | [apex-docs](../skills/apex-docs/SKILL.md) | Repair demonstrated drift; clarify contradictions about intended behavior |
| Clean legacy/generated artifacts | [apex-artifacts](../skills/apex-artifacts/SKILL.md) | Establish provenance and consumers; obtain a scoped removal decision when not already authorized |
| Explicitly maintain the planning board | [apex-board](../skills/apex-board/SKILL.md) | Invoke by name; inspect and propose, then apply the approved classification and changes |

The [project board](project-board.md) holds features, child tasks, bugs and draft notes. Its skill has implicit invocation disabled; routine development does not take work from it or maintain it automatically. Discovery synchronization also copies a skill's `agents/openai.yaml` so invocation policy remains effective.

Prior approval of a concrete plan remains valid. Skills must not ask for the same approval again. New ideas require clarification only where it affects scope, behavior, compatibility or meaningful risk. Generic GitHub approval clicks are not part of the normal process. Missing authentication or a real failed check is a blocker to explain, not a reason to bypass safeguards.

## A cheap preflight

`python -B dev/scripts/repo_status.py` reports the current branch, dirty state, at most five other local unmerged branches and at most five open PRs. Local refs can be stale and squash merges can leave misleading ancestry; the report is a prompt to inspect relevant work, not a conflict verdict. Warn briefly at the start of development and investigate file overlaps only when relevant. Do not read all open PR diffs or repeat the inventory throughout a turn. Use worktrees to avoid changing another task's checkout.

The GitHub helpers use `GH_TOKEN`/`GITHUB_TOKEN`, existing GitHub CLI authentication or a noninteractive Git credential helper. They never store tokens. Anonymous public reads are possible; writes require authentication. CLI/MCP tools can be used instead, preserving the same expected-head and destination checks.

## CI and publication

Every PR to `dev` or `main`, and every push to those branches, creates CI and a stable `Required checks` gate. A narrowly defined prose/skill-only change skips the Rust matrix and detached build. Unknown paths, workflow changes, tooling and application code run the full checks. Publication tests, source/link/skill checks and release-boundary checks always run. The final gate accepts a skipped runtime job only when the scope job explicitly determined it was unnecessary; failed/cancelled scope or validation never passes.

The Rust matrix retains Linux, Windows and macOS tests, Clippy and release builds. Formatting runs once on Linux. Linux runs MCP/HTTP/viewer integration against its same freshly built executable. Cargo caches are keyed by OS, architecture, toolchain, lockfile and revision; only trusted branch pushes save Cargo caches. npm caches package downloads. The standalone build remains deliberately separate to verify `app/` without contributor files.

The `Publish` workflow in `.github/workflows/wiki.yml` runs only after successful `main` push CI, or an explicit retry on `main` that independently verifies successful CI. It checks out that exact revision. A version change produces a source tag and release; a maintenance-only merge does not. Main CI uses a separate concurrency group per revision, and publication queues pending runs instead of replacing them. The wiki uses the same tested source. A delayed older release can receive its immutable tag but cannot overwrite the wiki. GitHub selects the latest release using commit date and semantic version (`make_latest: legacy`), rather than the order of job completion; see the [release API](https://docs.github.com/en/rest/releases/releases#create-a-release). Main is checked again immediately before wiki publication. The release skill waits for publication before proceeding to another release.

Release preparation and CI use [versioning](versioning.md). After release/hotfix publication the agent opens a normal `main` to `dev` synchronization PR, waits for checks and merges it. Merge commits preserve the long-lived branches' ancestry. Finish with the content/ancestry verification and temporary-branch cleanup described in [branch lifecycle](#branch-lifecycle).

## Dependabot

Normal Cargo, npm test-tooling and GitHub Actions updates run monthly and target `dev`. Configuration is stored on default `main`. Security updates target default `main` independently of the normal update schedule. A security PR that changes the application still needs the version and notes required for a hotfix release; it must not bypass that boundary. The agent can prepare a reviewed hotfix branch from main incorporating that fix. Changes only to test tooling can be maintenance.

A dependency-graph refresh after a merge is not an instruction to search for all new package versions. No extra post-release Dependabot search is configured.

## Bootstrap and local-only preparation

The source files do not create GitHub branches or settings merely by existing locally. The initial repository restructuring must be integrated deliberately; never include another task's changes implicitly.

1. Review the complete local restructuring and contributor changes. Keep unrelated work out of the selected integration PR.
2. With maintainer authentication, run `python -B dev/scripts/configure_github.py` to inspect the live setup, then `--apply` only when activating this process is authorized. It keeps main as default, creates dev at main if missing, mirrors the main protections (including deletion protection) onto dev, enables automatic deletion of merged temporary branches, and enables dependency alerts and Dependabot security updates through the [repository API](https://docs.github.com/en/rest/repos/repos#enable-dependabot-security-updates).
3. Create the initial feature branch/PR to dev and wait for CI. To promote product changes to main, prepare the first versioned release with the new tools; the release checker understands the earlier root Cargo layout as its baseline.
4. Verify `Required checks` is required on both branches, security updates are enabled, and the SHA-pinned GitHub-owned actions used in CI are allowed. Do not relax existing merge gates to bootstrap a failed build. Setup uses separate API requests: a failure can leave completed steps in place, so inspect the reported failure and current state before retrying.
5. Confirm the main `Publish` run and synchronization PR. Subsequent tasks can use the skills without GitHub UI interaction.

## Design references

The skills use focused investigation, explicit plan approval and incremental verification, informed by [Superpowers planning](https://github.com/obra/superpowers/tree/main/skills) and [OpenAI guidance on concise skills](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra). These are design references, not installed dependencies or additional approval/delegation requirements. The APEX instructions are scoped to this repository, with one implementation approval and existing authorization preserved.
