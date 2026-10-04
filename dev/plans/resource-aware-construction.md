# Resource-aware construction follow-up

Status: implemented, verified and integrated through PR #29.

## Observed problem

The demo adapter exports 3,705 unfinished operations from 3,800 MES operations
and 3,800 saved Excel rows. Input readiness succeeds. The original queue
constructor selects 402 hours for one person with 150 available hours in an
explicitly extended October 30 horizon. Excel-selected modes and chronological
order yield an independently valid complete schedule on the same extended input.
Neither this witness nor a failed greedy construction proves feasibility or
infeasibility within the original October 16 horizon.

Two general defects were corrected: secondary resource loads were absent from
construction estimates, and a global material consumption frontier prevented
valid earlier use of available stock. Estimates alone did not resolve the full
factory failure, motivating the approved shared placement state below.

## Proposed boundaries

1. Extract the existing main-placement state from `core/xg.rs`: resource booking,
   primary sequence tails, dated material reservations, predecessor completion
   and execution continuation. Keep the full decoder as the authoritative user
   of this state; avoid a second independently implemented placement algorithm.
2. Let construction probe and commit feasible task/mode placements against the
   current prefix for models whose earlier placements cannot change. Reuse the
   same calendars, material rules, phase semantics and hard commitments. Respect
   explicit mode choices, running work, fixed starts and forced prefixes.
3. Retain the current full-prefix route for transitions, conditional activities,
   native decorations or other rules that can change earlier work. Document the
   supported fast-path boundary explicitly. Do not silently omit such semantics.
4. Make mode selection use measured prefix availability, including secondary
   resources and shifts. Queue weights remain dispatch preferences; objective
   values still come only from complete, independently validated schedules.
5. Keep the input schema, customer adapters, public commands, persistence and
   independent validator contracts unchanged. Update architecture and search
   documentation to describe the implemented boundary.

This is a scheduling implementation refactor, not a demo-specific policy or an
Excel replay feature. It changes heuristic choices and needs explicit review of
the complete construction/decoding path. It does not promise optimality.

## Acceptance

- Preserve semantic, negative and corrupted-output coverage for calendars,
  capacity, phase-specific staffing, dated material, running work, fixed starts,
  sequence commitments, transitions and native dispatch policy replay.
- Demonstrate the adapter import and free construction on the complete captured
  factory input; validate independently and reconcile all operation IDs.
- Report the original and explicitly extended horizons separately. Never widen
  a source horizon or weaken a commitment automatically to get a result.
- Compare runtime and scheduling behavior on existing synthetic fixtures and
  the full factory. Report failed candidates and limits honestly.
- Run fmt, Clippy, workspace tests and a fresh release build, followed by MCP
  and browser checks, standalone distribution and repository publication checks.
- Merge/release only after the agreed acceptance checks pass. Dev integration
  passed all required checks; publication is recorded in the versioned release notes.

## Measured construction result

The shared state enables free `release`, `due`, `priority`, `shortest` and
`objective` construction of all 3,705 operations on the explicitly extended input.
Each completed in about 2.2–2.3 seconds in a local Windows release build and passed
independent validation. The `release` result preserves all operation IDs, six
running operations and 65 locks. It has 41 of 118 remaining orders on time and a
makespan of 1,959,012 seconds from the epoch. This is a feasible starting plan,
not a quality or optimality claim. Excel replay is unnecessary for this result.

The source October 16 horizon still fails free construction; the `queues` greedy
strategy also fails on the extended input. Neither observation proves
infeasibility. The source horizon, default strategy and commitments remain
unchanged. The optional adapter guide documents the explicit showcase choices.
Container acceptance covers the real public MES API, saved workbook, free
construction, independent validation and MCP overview using isolated state.
The final feature revision passed Linux, Windows and macOS checks, including the
desktop client, PostgreSQL, standalone export and complete container acceptance.
