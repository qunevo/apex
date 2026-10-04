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

## MCP App showcase

[showcase.mjs](showcase.mjs) constructs a small synthetic canonical factory and
source summary. `npm run test:insights` from the application directory imports
it through the release server, plans and validates a result, then exercises the
served dashboard in a browser sandbox. API tests cover revision/tenant isolation,
bounded output, evidence validation and package fallback. See the
[insights contract](../../../docs/insights.md#verification).
