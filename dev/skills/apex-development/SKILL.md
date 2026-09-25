---
name: apex-development
description: Investigate APEX feature ideas, clarify requirements, obtain approval of a concrete plan, then implement on a feature branch. Use for repository development, not customer planning or customization workflows.
---

# Develop an APEX feature

Read root `AGENTS.md`, [repository layout](../../docs/repository-layout.md) and the relevant part of [application architecture](../../../app/docs/architecture/README.md). Work from the repository root unless a command says otherwise.

## Investigate and clarify

Run `python -B dev/scripts/repo_status.py` once at the start of development. Briefly surface other open work and a dirty checkout before editing. Treat local unmerged branches as candidates, not proven conflicts; inspect only relevant overlaps. Do not load every PR diff or repeat the inventory on each turn. An unavailable GitHub check is an explicit limitation, not evidence that no work exists.

Turn spoken notes or pasted ideas into an outcome, boundaries and observable acceptance criteria. Read the affected entry points and tests before proposing changes. Ask one to three consequential questions at a time; distinguish unresolved requirements from choices the agent can make. Continue independent investigation while answers are pending.

Check assumptions, contradictory requirements, dependencies, migration needs, failure cases, ownership/state boundaries and the cost of extra context. For scheduling changes trace typed input, readiness, evaluation, objective/search semantics and independent validation together. Read only relevant references from the architecture change map.

## Plan and implementation approval

Present a concrete, proportionate plan: intended behavior, affected modules, ordered changes, acceptance checks, compatibility/release impact, open decisions and material risks. Offer alternatives only where they change a meaningful tradeoff. For larger work use a short file under `dev/plans/`; otherwise an in-task plan is sufficient. Do not turn a plan into a second copy of architecture documentation.

For an exploratory idea, wait for approval of the concrete implementation plan before modifying product code. Say that this stage is required by this skill and link this file. A previous explicit approval of that plan, including an instruction to implement it, already satisfies the gate; do not ask again. Follow explicit user instructions that change the planning process. Ask again only when a consequential change exceeds the approved scope.

## Implement the approved plan

Fetch `origin`, then create `codex/<topic>` from `origin/dev` in an isolated worktree when another task owns the checkout or it has unrelated changes. Continue the current task's existing feature branch rather than creating duplicates. If `dev` is absent, report the bootstrap requirement in [developer workflow](../../docs/developer-workflow.md); do not silently use a different integration branch. Never switch another task's checkout, reset its work or carry unrelated changes into a PR.

Keep edits within the approved scope. Keep customer workflows in `app/skills`, contributor workflows in `dev/skills` and generated discovery entries in `.agents/skills`. Split modules only for an understood responsibility boundary; use `apex-refactor` for a broader audit.

Run the checks required by `AGENTS.md`, including a fresh release build before integration checks and the standalone check when distribution boundaries change. Update affected contracts and evidence with the implementation.

## Finish the authorized workflow

Carry the user's existing authorization through to completion without asking for the same permission again:

- For implementation only, summarize the changes and checks and retain the unmerged feature branch. Development approval alone does not authorize merging or publishing.
- When integration into `dev` is requested, continue with [apex-merge-dev](../apex-merge-dev/SKILL.md) through verified merging and temporary-branch cleanup. Do not stop at a local commit or an open PR.
- When promotion to `main` is requested, finish feature integration first, then use [apex-release](../apex-release/SKILL.md) for the selected release scope, publication verification, synchronization back to `dev` and release-branch cleanup. A request only to prepare a release retains that stopping point.

Treat [branch lifecycle](../../docs/developer-workflow.md#branch-lifecycle) as the completion contract: verify the task's merged temporary branches are gone remotely and locally, preserve `main`, `dev`, active work and worktree files, and report any deferred cleanup with its reason. After release synchronization, verify ancestry and equal tracked content, retaining and explaining any newer development changes. A merged PR alone is not completion.

Report the actual checks, integration/publication state, branch cleanup and remaining limitations that apply to the authorized scope. Start later work on a fresh branch from current `origin/dev`; do not reuse a completed branch.
