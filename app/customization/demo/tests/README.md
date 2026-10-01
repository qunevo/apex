# Demo acceptance boundary

Package selection, input scoping, restart persistence and cross-package negative
cases are exercised by [server configuration tests](../../../tests/server_config.rs).
Future adapter tests belong here and must use synthetic source records, check
stable IDs, quantities, running remainders, alternative modes and commitments,
and independently validate the resulting scheduling input/output.
