# Factory adapter boundary

The future adapter consumes a configured MES API and an actual Excel working
file. It must create a reproducible, validated APEX input snapshot with source
IDs, revisions and explicit assumptions. It must not import Python modules or
read seed internals from the parent development repository. No live factory
adapter or source-system writeback is implemented in this structural package.
