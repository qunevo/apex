---
name: apex-merge-dev
description: Finish an APEX feature branch by creating or updating its pull request and merging verified work into dev when the user requests integration.
---

# Integrate a feature into dev

The user's instruction to merge this work into `dev` authorizes the PR, push, required fixes and normal merge for this task. Do not ask for the same authorization again or require manual GitHub clicks. An explicit request for `main` is a release request, not permission to silently change the destination; route it to `apex-release`.

Read [developer workflow](../../docs/developer-workflow.md). Confirm the task's branch, reviewed diff and destination. Preserve unrelated dirty changes. Commit only the task's intended files. Fetch the current `dev`; reconcile it without rewriting shared history, rerun affected checks and review the final diff.

Use available GitHub tools or CLI. The standard-library fallback is:

```bash
git push -u origin HEAD
python -B dev/scripts/pull_request.py open --head codex/TOPIC --base dev --title "Concrete resulting behavior" --body-file PATH_TO_BODY
python -B dev/scripts/pull_request.py status NUMBER
python -B dev/scripts/pull_request.py merge NUMBER --expected-head FULL_SHA --expected-base dev
```

Replace illustrative arguments with verified values. Write the PR body to a file with actual newlines. State the problem, resulting behavior and measured validation. Reuse a matching open PR. Attach every created or continued PR to the task using the app's attachment tool when available.

Wait for `Required checks` on the current revision and inspect unresolved review conversations. Address relevant feedback; do not dismiss substantive comments as mere obstacles. Make authorized corrections and wait for their new checks. Stop only for missing access, a conflict requiring a user decision, an out-of-scope fix, or a repeatedly failing external service. Report the concrete blocker; never bypass protections or manufacture a second reviewer.

Immediately before merging, verify the expected head SHA and target. Use the normal merge endpoint, preserving merge commits so ancestry between long-lived branches remains clear. Verify the merged SHA and resulting `dev` CI. Finish the task's remote and local temporary-branch cleanup using [branch lifecycle](../../docs/developer-workflow.md#branch-lifecycle); merge authorization includes this cleanup. Never delete `main` or `dev`. Preserve active checkouts and worktree files, and report any deferred cleanup. Do not merge other open branches as incidental cleanup. Report the PR link, integration result and branch cleanup.
