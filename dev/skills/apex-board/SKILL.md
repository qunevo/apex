---
name: apex-board
description: Maintain the APEX GitHub Project only when explicitly invoked; propose concise features, tasks, bugs and notes, obtain approval of their classification, and reconcile approved items with completed work.
---

# Maintain the APEX board

Use only when the user explicitly invokes this skill by name. Read [the board contract](../../docs/project-board.md) for the project, statuses and views. This skill maintains planning records; it does not select or implement development work. Do not invoke it from another skill, normal coding, CI, a schedule or a background agent.

## Inspect the requested scope

Establish whether the user wants to capture ideas, organize items or reconcile progress. Read the project's current fields and the relevant items, including their parents and children. Search narrowly for duplicates before creating anything. Do not inventory the entire repository or every PR for routine intake. Reuse existing items and preserve user-written notes.

Propose native issue types: Feature for an outcome, Task for a concrete step, Bug for a defect. Keep feature intake flat by default; do not create subtasks unless the user explicitly requests a breakdown. When requested, use native sub-issues for parent/child relationships and keep nesting shallow. Bugs can stand alone or belong to a relevant feature. Never create a fake parent called "Issues" to categorize bugs. Use draft items for loose ideas and notes; converting a draft into a repository issue is a separate proposed action.

Flag relevant duplicates, incompatible outcomes, dependencies and overlapping active work. Distinguish evidence from suspicion. Ask focused questions only when the answer changes classification, scope or acceptance. Keep unresolved ideas Deferred rather than inventing commitments, owners, deadlines or priorities.

## Propose and obtain approval

Show one compact batch: action, title, type, parent, status and proposed description. Include material conflicts and uncertainties alongside it. Ask the user to approve or amend the concrete classification and changes before creating, importing, converting, reparenting, editing, moving or closing items. Explain that this gate implements the user's requested board workflow and link this skill. Previously approved exact changes remain authorized; do not ask per item or request the same approval twice. Invocation alone authorizes inspection and proposals, not an unspecified write batch.

Write all card titles and descriptions in simple English, even when the conversation is in another language. Keep titles short, normally 3-8 words. Use 2-4 short bullet points for the outcome and observable completion criteria; a simple note can use one. Avoid long prose, nested lists and implementation plans in cards. Do not add branch names, commit hashes, code paths or implementation links to card titles or descriptions. Native parent/child relationships supply navigation. Put evidence needed for approval in the conversation, not in the card body.

## Reconcile only on request

When asked to check progress, inspect only the relevant implementation and checks. For code work, verify the complete acceptance criteria against current remote dev, not just a local branch, an unmerged PR, an issue title or the existence of a file. Report unavailable remote evidence as a limitation. For non-code work, use the agreed deliverable or the user's confirmation. Partial or uncertain completion stays open with a concise explanation.

Propose Done and closing as completed only for fully satisfied work. A feature requires its own outcome and required children to be complete; closing a child does not prove the parent is done. Cancellation or duplication is not completion: propose Deferred or a clearly explained closure as not planned. Preserve history; never delete items or silently remove unfinished acceptance criteria to make them appear complete.

## Apply and verify

Use available authenticated GitHub tools; use the browser when project fields or sub-issues are unsupported. Read the current state again before the approved batch, checking for concurrent edits. Pause only affected operations when changed facts invalidate approval. Apply just the approved items, fields and relationships. Confirm repository-issue visibility before converting private draft notes: the project is private, but qunevo/apex issues are public.

Verify the resulting items and relationships. Report applied changes, skipped operations and failures briefly; do not claim that a partial batch succeeded. Keep all project workflows off and leave unrequested items untouched. Never assign work to an agent, open development branches, start implementation or arrange recurring maintenance as a side effect.
