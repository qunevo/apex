# Demo model boundary

The fictional factory makes valve products to order. Its source environment
contains machining alternatives, washing, assembly and testing, with shared
equipment, personnel qualifications, dated availability and booked execution.

The MES owns released work instructions and execution facts. The actual Excel
working file owns proposed dispatch decisions, fixed decisions and skills.
Proposed assignments must remain distinct from commitments. Preserve stable
order, lot and operation IDs, the factory clock, units and source revisions.

The factory-specific APEX mapping is not implemented yet. Selecting this
package does not turn the technical `demo.create` fixture into the MES factory
or import MES data. Do not infer processing times or impose undocumented rules.
Prefer existing core types; new semantics require independent validation tests.
