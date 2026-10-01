# Demo language, filters and execution visibility

Implemented on `codex/demo-language-filters`, following the user's request to add
German/English UI templates, column filters and a direct view of current production.

## Scope and order

1. Add validated column filters before pagination and CSV export; preserve exact
   order/lot drill-down scopes and expose full-scope checkbox choices.
2. Project actual machine and current/next operation onto lots; link orders, lots,
   operations and production confirmations without changing execution records.
3. Add browser-persisted DE/EN selection and central UI/known synthetic-label
   templates, including the showcase and API validation messages.
4. Verify filters, export parity, source-value preservation, navigation and layouts.

## Compatibility and acceptance

- Existing data and Excel workbooks remain unchanged; no database reset or migration.
- API identifiers and enum values remain canonical English. CSV and Excel remain
  source artifacts. Translated controls submit original values.
- In-progress orders may have no running operation. Actual machines must come from
  execution bookings, never from planned Excel assignments.
- Combined columns use AND, selected values use OR; numeric/date bounds are inclusive.
- Local validation: 35 Python tests, locale parameter/error checks, JavaScript syntax,
  documentation/publication checks and browser verification of filters and drill-down.
- Six German presentation chapters retain visible controls at 1280 × 720.

No scheduler or application distribution boundary changes. Implementation only;
integration into `dev` is a separate request.
