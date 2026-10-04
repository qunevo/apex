# APEX development board

The private [APEX Development project](https://github.com/orgs/qunevo/projects/1) is linked to `qunevo/apex`. It is a shared planning overview, not an execution queue. Maintain it only by explicitly invoking [apex-board](../skills/apex-board/SKILL.md); routine development and release skills do not read or update it automatically.

## Structure

| Status | Meaning |
| --- | --- |
| Deferred | Ideas, notes or work deliberately postponed; the intake default |
| Open | Selected work that has not started |
| In Progress | Work actively underway |
| Done | Agreed outcome completed; code changes integrated into dev |

Use native issue types **Feature**, **Task** and **Bug**. Features describe outcomes. Keep feature intake flat; create child tasks only when the user explicitly requests a breakdown, using native sub-issue relationships. A bug can stand alone or belong to a feature. Keep nesting shallow. An issue is GitHub's general work item, not a category competing with features. Draft items can hold early ideas or notes without creating a repository issue.

The **Board** view shows all items in the four status columns. **Bugs** and **Features** are filtered views of the same items, not separate backlogs. Tasks remain visible on the main board and under their parent issue. Avoid extra priority, estimate, sprint or release fields until needed.

## Maintenance rules

- Approve concrete proposed creation, classification, relationships and changes as one batch before the skill writes them. Existing exact approval remains valid.
- Write both titles and descriptions in simple English. Use short titles and 2-4 concise bullet points, without nested lists or long implementation plans. Keep branch references, code paths and implementation links out of card descriptions.
- Reconciliation is explicitly requested. Check full completion against current dev; keep partial or uncertain work open. Mark completed work Done and close its issue while preserving history.
- Keep built-in project workflows disabled, including automatic intake, status changes, child-item additions, agent assignment and archiving. Do not add CI jobs or scheduled board maintenance.
- The private project does not make its linked repository issues private. `qunevo/apex` issues are public; propose draft-to-issue conversion explicitly and keep private notes in drafts.

Example invocation: `Use $apex-board to organize these ideas. Show the proposed features and statuses before applying anything.`

GitHub references: [sub-issues](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/adding-sub-issues) and [built-in project workflows](https://docs.github.com/en/issues/planning-and-tracking-with-projects/automating-your-project/using-the-built-in-automations).
