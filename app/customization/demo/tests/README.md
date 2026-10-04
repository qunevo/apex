# Demo acceptance boundary

Package selection, input scoping, restart persistence and cross-package negative
cases are exercised by [server configuration tests](../../../tests/server_config.rs).
Adapter tests use synthetic source records, check stable IDs, quantities, running
remainders, alternative modes and commitments, and independently validate the
resulting scheduling input/output.

## Source adapter tests

`test_adapter.py` uses deliberately synthetic records and OOXML parser fixtures
from `adapter_fixtures.py`. It covers dynamic workbook growth, header movement,
invalid IDs/values, source revision retries, HTTP/CLI export, calendars, quantities,
material and execution mapping, fixed decisions and an optional real-engine
planning/validation/corruption check. Run instructions are in the
[adapter guide](../adapter/README.md#tests). These tests need no parent demo code.
