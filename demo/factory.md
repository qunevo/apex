# Northstar Valve Works

## Production and products

Northstar is a fictional make-to-order manufacturer of valve and manifold assemblies for twelve equally fictional equipment makers, identified as OEM 01 through OEM 12. Nothing in this case is copied from a customer system.

The baseline contains 120 customer orders, each split into five production lots. The 600 lots contain 5, 10, 15, 20 or 25 pieces. Eighteen article variants combine distributor/regulator/sensor functionality, compact/standard/large body size and aluminium/stainless steel. Each order uses one article. A complete shipment requires all its lots.

The planning period is 05–16 October 2026. The shopfloor snapshot is Monday 05 October at 10:00, Europe/Berlin. Production starts at 06:00. The conventional baseline may complete some work after the target period; overdue operations are visible in Excel, not silently moved inside the horizon.

## Material flow

All lots visit CNC machining, deburring and washing. The cleaned body is transferred as a whole lot. A machining cell becomes available after its operation; cleaning uses separate equipment.

| Variant | Downstream workplan |
| --- | --- |
| Distributor | Mechanical assembly, final leak/function test |
| Regulator | Mechanical assembly, valve adjustment, final leak/function test |
| Sensor | Mechanical preassembly, sensor installation, calibration, final assembly, final leak/function test |

Sensor lots return to the shared assembly area after calibration. There is no reentrant machining, automatic batch formation, yield loss or quantity splitting during an operation in this seed. No quality step is optional.

## Equipment and people

Six CNC cells form compact, universal and heavy-duty pairs. CNC-01/02 accept small and medium aluminium bodies; the other four accept every body in this initial case. Two deburring benches and two washers serve all lots. Eight assembly benches, two electronics benches, one calibration bench and three test benches support the second stage. QA-01 handles distributors only; QA-02/03 handle all variants.

The equipment window is weekdays 06:00–22:00. Personnel work 06:00–14:00 or 14:00–22:00 with a half-hour break at 10:00 or 18:00. There are 24 named synthetic people: six in machining, twelve in assembly and six in quality. Qualifications overlap but each person remains one capacity-one resource. Excel owns the qualification matrix; MES owns attendance.

The baseline assumes continuous personnel attendance for every operation except CNC machining, where the assigned person attends the setup phase only. CNC may continue across that person's break. Operations reserve the machine throughout their duration. The baseline does not split operations across shifts, change people inside an operation or infer unattended night operation.

## Materials and maintenance

Each piece consumes one material-appropriate body blank and one seal kit. Regulator and sensor variants also require a valve kit; sensors require a sensor kit at installation. Opening stocks and confirmed receipts are provided in pieces. Supply is allocated conservatively in order-due-date order. It is not replenishment planning.

The initial sensor stock is deliberately limited. More sensor kits arrive Wednesday 07 October at 10:00. Seal kits arrive Tuesday; valve kits arrive Thursday. The seed includes a CNC-03 spindle inspection, a QA-03 reference instrument calibration and WS-02 bath service. The baseline respects these blocked intervals.

Equipment unavailability is normally a dated exception with a start, end and reason. Status at the factory snapshot is derived from these periods; future maintenance does not make a machine unavailable today. The end is exclusive. A separate permanent-unavailability flag covers equipment out of service indefinitely. Periods may be edited or cancelled, and multiple periods per machine are supported. Operating shifts remain a separate calendar constraint.

## The existing Excel plan

The workbook represents a planner's reproducible starting arrangement, not an actual human performance measurement or an APEX result. Its construction follows order due dates and priority, then picks compatible available equipment and personnel. It protects scarce qualifications when another person is available within a reasonable wait. Setup allowances are fixed per operation. Sequence-dependent setup matrices are part of the future APEX model, not enforced in this baseline.

The workbook contains:

- **Dispatch plan:** all 3,800 operations, resource sequences, machine/person assignments, start times, setup/run allowances, calculated finishes, due dates, calculated lateness and fixed decisions.
- **Skills:** the editable person-by-qualification matrix.
- **Shift roster:** MES attendance/calendar snapshot and calculated net daily hours.
- **MES orders:** the original delivery/priority source used by plan formulas.
- **Material:** usable opening stock and confirmed incoming deliveries.

Operations before the snapshot are marked complete; those crossing it are running. The next two hours of planned starts are fixed in Excel. The protected interval is a demo planning convention, not an APEX freeze command.

Editing a start or duration in Excel recalculates that row's finish and lateness. It does **not** repair other assignments, reapply calendar rules or validate all constraints. The initial generated baseline is checked independently for occupancy and precedence; later manual edits require review. There is no live Excel synchronization in this implementation.

## Boundaries for later APEX integration

The adapter will consume business records and the planner's decisions, preserve stable order/lot/operation IDs and translate units, calendars, eligibility and commitments into typed APEX inputs. It must distinguish proposed assignments from fixed decisions and preserve booked execution.

Before adding sequence-dependent setups, fixtures, finite buffers, maximum waiting times, alternative complete routes or automatic wash batches, their exact occupancy and release contracts need to be specified and tested. The current mock supplies a realistic source environment; it does not claim those solver semantics are already integrated.
