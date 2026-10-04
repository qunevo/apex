# Source details

`views.get` with `view_id: "demo.sources"` selects this read-only extension for
APEX scenarios pinned to `demo@1`. The standard Planning overview is the default.
Source details works after importing the adapter's `scenario.json`; a result is optional.

- [provider.rs](provider.rs) implements `demo.sources`: remaining operations,
  Excel coverage and import notices.
- [renderers.js](renderers.js) and [style.css](style.css) supply `demo.import-flow`
  with MES/Excel counts and source fingerprints.
- General delivery KPIs, critical orders, stage charts, resource utilization and
  schedule navigation belong to the main app. This extension only adds source evidence.

Missing source evidence means unknown coverage. Counts inconsistent with the
current task count are not shown as coverage. Refresh does not reread sources.
Providers, scripts and styles require explicit build/server registration; see
the [extension contract](../../../docs/insights.md). The scheduling core is unchanged.
