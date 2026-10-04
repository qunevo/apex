# MCP App insights and customization views

`views.get` opens a read-only dashboard before or after planning in an MCP Apps
host. Other hosts receive structured/text data. `results.get` remains the Gantt
view. Use `{ "scenario_id": "<UUID>" }`, optionally adding `revision`, `result_id`
and `view_id`. The default revision is the result's revision when supplied,
otherwise the scenario's current revision. A result from another scenario or
revision is rejected; its content hash must match.

Responses identify the scenario, revision, hash, package, selected view and
optional result/validation. Refresh and view switching retain that snapshot.
They do not poll, import, plan, approve or publish. Call the tool again without
`revision` to inspect the latest input. `POST /v1/views` takes the same arguments
with viewer authorization. The tool advertises `ui://apex/insights.html` through
`_meta.ui.resourceUri`. The UI uses only the host bridge, without credentials,
direct network requests or remote chart dependencies.

## Presentation contract

[views.rs](../middleware/control/src/views.rs) defines `Provider`, `Registry`,
`Context`, `Dashboard`, `Panel` and `Visual`. Providers receive borrowed input and
an optional result after tenant-scoped authorization. Dashboards contain metric
cards and `bars`, `donut`, `table` or `custom` panels, grouped into `input`,
`orders`, `resources` or `result` sections. Shared widgets provide filtering, category selection,
exact-value tables, themes and responsive layouts.

Limits: 12 metrics, 12 panels, 24 points/rows per panel, 8 table columns, finite
numbers (or `null` for an unknown metric), nonnegative chart values, bounded text, unique panel IDs and 64 KiB per
serialized document. Custom panel data is limited to 8 KiB. `grouped` retains the
largest 23 categories and combines the rest into a labeled group without losing
totals. Providers must explain units and omissions. Visual summaries are not new
scheduling objectives or independently validated engine KPIs.

## Standard planning overview

`overview` is always the default, independently of customization. For APEX it provides:

- Order, operation and resource counts; on-time delivery, late deliveries and plan span.
  If orders are absent, delivery statistics use lots (jobs), then operations.
- Upcoming commitments before planning, or up to 12 critical records from a saved
  validated result. Missing completion comes first, then lateness, remaining
  buffer and priority. A buffer of at most one hour is a presentation threshold,
  not a scheduling constraint. Dates use UTC when an input epoch is available.
- Delivery status distribution, retaining undated and unscheduled records.
  Completion comes from saved order/job completion maps or operation `ready`,
  including required post-processing; it is never inferred from a Gantt envelope.
- The highest 24 resource utilization ratios from saved `utilization_7d` metrics
  (main work and recorded actual work, first seven days capped by the planning
  horizon; excludes conditional pre/post-processing). Ratios are never summed into
  an Other group or a share of total. Before planning, show calendar capacity hours.
- Operation counts by stage and selected engine metrics with explicit units.

Unknown outcomes render as a dash, not zero. No due dates means no on-time
percentage. Invalid results withhold all outcome KPIs and predicted completions.
Input counts can include alternative-route candidates. The dashboard does not
compute OEE, scrap, actual production progress or financial KPIs without evidence.

Open schedule reads `results.get` through the host bridge and verifies the result,
scenario, revision and content hash. It displays main operations on their primary
resources, with resource/text filters and up to 200 matching operations at a time.
The saved view contains all operations; filtering changes the displayed subset.

Other engines retain a generic saved-metric and primary-resource-span fallback.
Those spans include pauses and exclude secondary resources and pre/post work;
they are not capacity utilization. Shared branding in `ui/mcp-app/brand/` embeds
the original APEX wordmark and by Qunevo byline with no website/runtime dependency.

## Register a customization view

1. Implement `Provider` in `customization/<id>/ui/`. Declare a unique view ID,
   title, exact package identity/version and optional engine ID. Return bounded
   declarative panels from `build`.
2. Link the module and register it with `Registry::register` at startup through
   `Control::with_views`. Duplicate IDs fail. Views match their declared engine
   and the revision's pinned package version. `overview` remains the default;
   `available_views` lists matching extensions and the standard overview.
   `package.json` stays an identity manifest, without code discovery.
3. Prefer shared visual types. For a special component use `Visual::Custom` with
   a package-prefixed ID, such as `demo.import-flow`. Register a function through
   `window.apexViews.register(id, render)` in a package script; explicitly embed
   it and its styles in [ui.rs](../middleware/api/src/ui.rs). Renderers receive a
   container, panel and safe DOM/number/table helpers. Unknown renderers show
   bounded text; a failed renderer does not hide other panels. Tool data is never
   evaluated as code or HTML.
4. Rebuild and test the served resource in an MCP sandbox. Package code is
   trusted, statically bundled application code. There is no runtime code loader.

The [demo](../customization/demo/ui/README.md) exercises both hooks. The scheduling
core does not depend on presentation modules. Historical packages without a
matching custom provider retain the generic overview.

## Adapter evidence

Optional `content.source_summary` is separate from `facts` and planning intent.
It is persisted and hashed with the revision, but not passed to the engine:

- `sources`: up to 8 `{label, sha256, revision?}` fingerprints.
- `counts`: up to 32 named nonnegative integer counts.
- `notices`: up to 20 `{code, message, entity?}` diagnostic samples.
- `notice_count`: total notices, including omitted samples.

This is **adapter-reported evidence**, not verified source truth or schedule
validation. Full reports and sources stay local to the adapter. Missing evidence
means unknown coverage; old evidence is not inherited automatically. Supply fresh
evidence with imports and omit it when editing facts it no longer describes.
Include fingerprints, not private URLs or credentials.

## Verification

After a fresh release build, run `npm run test:insights` and `npm run test:mcp-app`
from the application directory. Set `APEX_CONTROL_BINARY` for a nondefault build.
The insights test uses the official MCP SDK against a temporary release server,
imports a synthetic factory and independently validates its plan. It exercises
the served HTML/documents in an opaque browser sandbox: charts, filtering, view
selection, schedule navigation, identity, errors, escaping, renderer fallback,
late refresh, themes, layouts from 320 to 1160 px and teardown. API tests verify
post-processing completion, missing due dates, invalid outcomes and absent completion.

Screenshots and an interactive synthetic preview appear under
`.apex/reports/planning-overview-*` as local artifacts. The preview has no live source
connection. Individual chat hosts still require deployment verification.
