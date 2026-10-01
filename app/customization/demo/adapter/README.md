# Factory adapter boundary

The future adapter consumes a configured MES API and an actual Excel working
file. It must create a reproducible, validated APEX input snapshot with source
IDs, revisions and explicit assumptions. It must not import Python modules or
read seed internals from the parent development repository. No live factory
adapter or source-system writeback is implemented in this structural package.

The repository demo deployment mounts a `sources.json` here with `mes_url` and
`planning_workbook`, plus the actual working file at `/sources/demo`. These are
adapter inputs reserved by that deployment, not settings consumed by the generic
middleware. No automatic extraction or import occurs until this adapter exists.
