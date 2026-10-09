# Demo workplan navigation and sequence-dependent setup overrides

Status: workplan navigation and setup overrides remain a refined proposal;
the precedence decision below is still open. The user explicitly approved the
fixed demo time, including chat analyses, and that part is implemented locally.
Continue on `codex/demo-fixes` and preserve the existing Excel opener fix. The
user authorized committing and pushing the current work to that feature branch
on October 7, 2026.

## Intended behavior

- Navigate from an article to its assigned workplan revisions, their steps and
  the eligible machine alternatives. Keep the selected article and revision
  visible while moving between these views.
- Put article/workplan approval and the default workplan inside the article's
  workplan list. The existing assignment records remain authoritative; remove
  their separate, unhelpful top-level tab.
- Give workplan steps, machine alternatives and component requirements explicit
  navigation entries. Direct access and drill-down must select the same active
  tab and preserve their context on return.
- In Machine alternatives, select article, workplan revision and step. Show
  only matching size/material alternatives, including their default setup and
  unit times. Preserve draft/release rules and existing released lot snapshots.
  Shared workplans remain shared; navigation does not clone them per article.
- Own sequence-dependent setup overrides in the saved Excel workbook. Use a
  filterable list with one transition per row, rather than a square grid.

## Excel setup contract

The Setup transitions sheet contains:

| Machine | Previous article | Previous workplan revision | Previous step | Next article | Next workplan revision | Next step | Setup minutes | Note |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |

Step references identify reusable steps of an exact workplan revision, not
lot-specific operation IDs or ambiguous display names. The current lot's
released workplan and step determine the applicable identity.

A matching row replaces the full default setup time of the next operation's
selected machine alternative. A missing row or empty override uses that released
default. Zero is an explicit zero-duration override. Overrides are never added
to the same default setup a second time. Reject duplicate keys, unknown or
incompatible references, negative/non-finite times and incomplete rows.

Select the predecessor on the actual candidate machine sequence. Reordering or
changing machines can change the setup duration. Do not freeze transitions to
the original Excel proposal during import. The first operation needs a defined
starting context; use known execution history where available and the default
when there is no known predecessor. Never infer historical machine state from a
future proposed Excel assignment. Already running work must not be set up again.

Open decision: the current Dispatch plan has editable setup allowances per
lot-operation. The recommended contract replaces this competing input with a
calculated/displayed setup and its source. Alternatively, retain a separate,
explicit per-operation override with precedence over transition/default values.
Do not silently treat existing populated baseline cells as overrides that mask
every new transition rule.

Product washing/deburring remain ordinary workplan steps. This change adds no
quality rework loops or new machine-cleaning activities. It can express setup
differences between their article/step combinations on shared equipment.

## Fixed demo time

Keep the existing business snapshot at 2026-10-05 10:00 Europe/Berlin and the
October 5-16 target period. Label this clearly as the demo snapshot, make the
normal clock display read-only, and keep all planning defaults based on that
snapshot. Do not shift orders, receipts, downtime or Excel dates with host time.
Explicit business edits and progress bookings remain available. Real audit
timestamps do not advance the simulated business clock.

Reject explicit clock changes as well. Carry the frozen instant and timezone in
chat onboarding, the demo planning skill, imported facts and source notices.
Distinguish overdue-at-snapshot analysis from lateness of a validated future plan.

Validation: 55 MES tests and 22 adapter tests passed; the optional engine-backed
adapter test was skipped. The tests include an edit under a simulated 2035 wall
clock and reopening the stored snapshot. JavaScript syntax and locale parity
checks passed. An isolated Docker instance shows the fixed snapshot in the UI;
the user's saved MES, workbook and imported scenarios have not been replaced.

## Implementation order and affected areas

1. Update `demo/mes/web/{app,details}.js` and focused workplan navigation code,
   styles/locales as needed. Add server projections or scoped query support only
   where article/revision/step joins cannot be represented by existing filters.
   Test article-specific alternatives, active tabs, return links and release locks.
2. Add the Excel transition table, source reference columns and effective setup
   display through `demo/planning/build-workbook.mjs`. Rebuild and visually check
   the synthetic baseline workbook. Resolve the per-operation override decision
   before changing the Dispatch plan's input contract.
3. Update the standalone demo adapter's workbook parser, source identity mapping
   and scheduling conversion. Preserve released defaults and execution history.
   Reuse typed transition/activity semantics where they preserve the selected
   machine, staffing and fixed-work contracts. Prove those contracts on small
   fixtures before applying the full factory; investigate any representation gap
   before proposing additional engine semantics. Keep domain logic in the demo
   customization, without parent-directory dependencies or format versions.
4. Clarify the snapshot display and remove the normal advance-clock control.
   Update source ownership, showcase wording, workbook sheet counts and docs.
5. Verify synthetic scenarios and the composed demo without resetting the user's
   current MES/Excel state. Preserve existing saved edits and access tokens.

## Acceptance checks

- Article -> assigned workplan -> step -> machine alternatives remains usable
  in both languages; direct tab entry yields the same filtered alternatives.
- Different workplan steps for the same article remain distinguishable, including
  repeated visits to one machine group and alternate workplan revisions.
- A/B and B/A can differ; same-article changes, zero overrides, missing rows,
  default fallback and machine changes behave as declared.
- Excel's displayed setup agrees with its proposed sequence. APEX recomputes
  setup for its selected sequence and retains actual machine/person occupancy.
- Invalid matrix rows fail with source locations. Fixed/running work is preserved;
  independent validation rejects corrupted durations and resource reservations.
- Reloading/restarting never advances the business clock or shifts source dates.
- Run affected MES/adapter/browser tests, spreadsheet recalculation and visual
  checks. If Rust changes prove necessary, run the required Rust checks and a
  release build before integration verification. Run the standalone check if
  distribution boundaries change. Report measured full-factory behavior without
  claiming optimality or cross-platform verification that was not performed.

The current authorization includes a feature-branch commit and push. Merge,
release, live demo reset and replacement of the user's working workbook remain
outside its scope.
