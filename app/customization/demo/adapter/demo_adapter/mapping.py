"""Factory-specific translation using existing canonical APEX types only."""
from collections import defaultdict
import math

from .calendar import Clock
from .common import indexed, number, require

PRIORITIES = {"Normal": 1, "High": 2, "Urgent": 3}


def ref(records, entity, ident, location):
    require(ident in records[entity], "REFERENCE", location, f"Unknown {entity} ID: {ident}")
    return records[entity][ident]


def seconds(minutes, location):
    return math.ceil(number(minutes, location) * 60 - 1e-8)


def requirement(resource):
    return dict(resource=resource, amount=1, retain=True)


def mode_for(op, alternative, person, quantity, plan):
    machine = alternative["resource_id"]
    setup = number(alternative["setup_minutes"], op["id"])
    run = number(alternative["minutes_per_unit"], op["id"], positive=True) * quantity
    if plan and plan["machine_id"] == machine:
        if plan["setup_minutes"] is not None:
            setup = plan["setup_minutes"]
        if plan["run_minutes"] is not None:
            require(plan["quantity"] is not None, "QUANTITY", plan["_row"], "Run allowance needs its source quantity")
            run = plan["run_minutes"] * quantity / plan["quantity"]
    attendance = alternative["attendance"]
    require(attendance in ("Setup only", "Continuous"), "ATTENDANCE", op["id"], "Unknown released attendance rule")
    if plan and plan["machine_id"] == machine and plan["attendance_minutes"] is not None:
        expected = setup if attendance == "Setup only" else setup + run
        # Scale the run part after scrap, while preserving one setup per operation.
        supplied = plan["attendance_minutes"]
        if attendance == "Continuous" and plan["quantity"] and quantity != plan["quantity"]:
            supplied = setup + max(0, supplied - setup) * quantity / plan["quantity"]
        require(abs(supplied - expected) <= 1 / 60, "ATTENDANCE", plan["_row"],
                "Excel attendance disagrees with the released Setup only/Continuous rule")
    phases = []
    for ident, work, staffed in (("setup", seconds(setup, op["id"]), True),
                                 ("run", seconds(run, op["id"]), attendance == "Continuous")):
        if work:
            phases.append(dict(id=ident, work=work, requirements=[requirement(machine)]
                               + ([requirement(person)] if staffed else [])))
    require(phases, "PROCESSING", op["id"], "Operation must have positive work")
    return dict(id=f"{machine}/{person}", primary=machine, phases=phases, contiguous=True)


def running_execution(op, mode, clock):
    start = clock.seconds(op["actual_start"])
    require(0 <= start <= clock.as_of and not op.get("actual_end"), "EXECUTION", op["id"],
            "Running operation needs actual start at/before snapshot and no finish")
    cursor, segments, reservations, remaining = start, [], [], {}
    for phase in mode["phases"]:
        elapsed = min(phase["work"], max(0, clock.as_of - cursor))
        remaining[phase["id"]] = phase["work"] - elapsed
        if elapsed:
            segments.append(dict(phase=phase["id"], start=cursor, end=cursor + elapsed, work=elapsed))
            for req in phase["requirements"]:
                reservations.append(dict(resource=req["resource"], start=cursor, end=cursor + elapsed, amount=1))
            cursor += elapsed
    require(sum(remaining.values()) > 0 and cursor == clock.as_of, "RUNNING_ESTIMATE", op["id"],
            "Running work has exhausted its duration estimate; correct the Excel allowance before importing")
    return dict(mode=mode["id"], as_of=clock.as_of, remaining_work=remaining,
                actual=dict(id=op["id"] + ":actual", task=op["id"], role="actual", mode=mode["id"],
                            start=start, end=clock.as_of, segments=segments, reservations=reservations))


def build(snapshot, plans, skills, provenance, *, horizon_end=None):
    records = {entity: indexed(rows, entity) for entity, rows in snapshot["records"].items()}
    clock = Clock(snapshot["factory"], horizon_end)
    warnings = []
    for person in skills:
        ref(records, "personnel", person, skills[person]["_row"])
    for ident, plan in plans.items():
        op = ref(records, "operations", ident, plan["_row"])
        lot = ref(records, "lots", op["lot_id"], ident)
        require(plan["lot_id"] == lot["id"] and plan["order_id"] == lot["order_id"],
                "EXCEL_JOIN", plan["_row"], "Operation, lot and order do not match the MES")
        for key, entity in (("machine_id", "machines"), ("person_id", "personnel")):
            if plan[key]:
                ref(records, entity, plan[key], plan["_row"])
        if plan["start"] is not None:
            clock.date(plan["start"])

    resources = []
    for entity, key, target in (("downtime", "resource_id", "machines"), ("absences", "person_id", "personnel")):
        for row in records[entity].values():
            ref(records, target, row[key], row["id"])
    for entity, shift_key, exception_entity, resource_key in (
            ("machines", "calendar", "downtime", "resource_id"),
            ("personnel", "shift_id", "absences", "person_id")):
        for ident, row in sorted(records[entity].items()):
            shift = ref(records, "shifts", row[shift_key], ident)
            exceptions = [r for r in records[exception_entity].values() if r[resource_key] == ident]
            unavailable = row.get("permanently_unavailable", False) or (entity == "personnel" and row["attendance"] != "Present")
            windows = [] if unavailable else clock.windows(shift, exceptions)
            resources.append(dict(id=ident, capacity=1, calendar=windows))
    indexed(resources, "resources")
    inventory = {ident: number(row["on_hand"], ident) for ident, row in sorted(records["materials"].items())}
    receipts = []
    for ident, row in sorted(records["receipts"].items()):
        ref(records, "materials", row["material_id"], ident)
        at = clock.seconds(row["available_at"])
        require(row["status"] in ("Received", "Confirmed", "Delayed"), "RECEIPT", ident, "Unknown delivery status")
        if row["status"] == "Received" and at <= clock.as_of:
            continue  # Included in MES on_hand already.
        if at <= clock.as_of:
            warnings.append(dict(code="OVERDUE_RECEIPT", id=ident,
                                 message="Unreceived overdue supply excluded until its arrival is updated"))
            continue
        receipts.append(dict(item=row["material_id"], at=at, amount=number(row["quantity"], ident, positive=True)))

    grouped = defaultdict(list)
    for op in records["operations"].values():
        ref(records, "lots", op["lot_id"], op["id"])
        grouped[op["lot_id"]].append(op)
    tasks, dependencies, locks, jobs, orders = [], [], [], [], []
    excluded, running_reservations, proposals = [], defaultdict(float), []
    fixed_orders = defaultdict(list)
    for lot_id, lot in sorted(records["lots"].items()):
        order = ref(records, "orders", lot["order_id"], lot_id)
        require(order["priority"] in PRIORITIES, "PRIORITY", order["id"], "Unknown priority")
        priority = PRIORITIES[order["priority"]]
        due = clock.seconds(order["due"])
        available = number(lot["quantity"], lot_id, positive=True, integer=True)
        operations = sorted(grouped[lot_id], key=lambda op: op["sequence"])
        require(operations and len({o["sequence"] for o in operations}) == len(operations),
                "OPERATIONS", lot_id, "Expected released operations with unique step numbers")
        previous, has_task = None, False
        for op in operations:
            ident = op["id"]
            require(op["status"] in ("Waiting", "Running", "Complete", "Skipped"), "STATUS", ident, "Unknown operation status")
            good = number(op["completed_quantity"], ident, integer=True)
            scrap = number(op.get("scrap_quantity", 0), ident, integer=True)
            require(good + scrap <= available, "QUANTITY", ident, "Booked output exceeds predecessor supply")
            if op["status"] in ("Complete", "Skipped"):
                require(previous is None, "PRECEDENCE", ident, "Closed operation follows unfinished work")
                if op["status"] == "Complete":
                    require(good + scrap == available and op.get("actual_end")
                            and clock.seconds(op["actual_end"]) <= clock.as_of,
                            "EXECUTION", ident, "Completed operation needs accounted quantities and actual finish")
                else:
                    require(available == 0, "QUANTITY", ident, "Skipped operation still has usable input")
                available = good
                excluded.append(dict(id=ident, status=op["status"], completed_quantity=good,
                                     scrap_quantity=scrap, actual_start=op.get("actual_start"), actual_end=op.get("actual_end")))
                continue
            require(available > 0 and good + scrap < available, "QUANTITY", ident, "Unfinished work must have remaining input")
            require(lot.get("quality_status", "Released") == "Released", "QUALITY_HOLD", lot_id,
                    "Release the quality hold or supply an explicit release decision before planning this lot")
            plan = plans.get(ident)
            is_running = op["status"] == "Running"
            require(not is_running or previous is None, "PRECEDENCE", ident, "Running operation has an unfinished predecessor")
            if not is_running:
                require(not good and not scrap and not op.get("actual_start"), "EXECUTION", ident,
                        "Waiting work cannot contain execution bookings")
            candidates = []
            alternatives = indexed(op["machine_options"], ident, "resource_id")
            if plan and plan["machine_id"]:
                require(plan["machine_id"] in alternatives, "MACHINE", plan["_row"], "Proposed machine is not a released alternative")
            if is_running:
                ref(records, "machines", op["resource_id"], ident)
                ref(records, "personnel", op["person_id"], ident)
                require(not plan or not plan["machine_id"] or plan["machine_id"] == op["resource_id"],
                        "EXECUTION_PROPOSAL", ident, "A running operation cannot move to the proposed Excel machine")
            for machine, alternative in sorted(alternatives.items()):
                equipment = ref(records, "machines", machine, ident)
                if is_running and machine != op["resource_id"]:
                    continue
                if equipment["permanently_unavailable"]:
                    continue
                for person, worker in sorted(records["personnel"].items()):
                    if is_running and person != op["person_id"]:
                        continue
                    if worker["attendance"] != "Present" or not skills.get(person, {}).get(op["skill"], False):
                        continue
                    candidates.append(mode_for(op, alternative, person, available, plan))
            require(candidates, "NO_ELIGIBLE_MODE", ident, "No available qualified machine/operator combination")
            source = f"MES@{snapshot['revision']}/operations/{ident}; Excel:{provenance['workbook_sha256']}"
            task = dict(id=ident, release=max(clock.as_of, clock.seconds(lot["release"])), due=due,
                        priority=priority, family=lot["item_id"], modes=candidates, quantity=available - scrap,
                        job=lot_id, stage=op["group"], source=source, consume={})
            for need in op["material_requirements"]:
                material = need["material_id"]
                ref(records, "materials", material, ident)
                amount = number(need["quantity_per_unit"], ident, positive=True)
                if is_running:
                    running_reservations[material] += amount * (available - good - scrap)
                else:
                    task["consume"][material] = task["consume"].get(material, 0) + amount * available
            if is_running:
                require(len(candidates) == 1, "EXECUTION", ident, "Actual machine and operator must resolve uniquely")
                task["execution"] = running_execution(op, candidates[0], clock)
                task["release"] = clock.seconds(op["actual_start"])
                if plan and plan["fixed"] and (plan["machine_id"] != op["resource_id"] or plan["person_id"] != op["person_id"]
                                               or clock.seconds(plan["start"]) != task["release"]):
                    raise_fixed(plan, "Excel fixation conflicts with booked MES execution")
            elif plan and plan["fixed"]:
                mode = f"{plan['machine_id']}/{plan['person_id']}"
                require(any(m["id"] == mode for m in candidates), "FIXED_MODE", plan["_row"], "Fixed assignment is not qualified/available")
                start = clock.seconds(plan["start"])
                require(task["release"] <= start < clock.horizon, "FIXED_START", plan["_row"], "Fixed start is outside the remaining planning period")
                locks.extend([dict(kind="mode", task=ident, mode=mode), dict(kind="start", task=ident, at=start)])
                if plan["sequence"] is not None:
                    fixed_orders[plan["machine_id"]].append((plan["sequence"], start, ident))
            if plan:
                proposals.append({key: value.isoformat() if hasattr(value, "isoformat") else value for key, value in plan.items()})
            if previous:
                dependencies.append(dict(before=previous, after=ident))
            tasks.append(task)
            previous, has_task = ident, True
            available -= scrap  # No forecast scrap beyond amounts already booked.
        if has_task:
            jobs.append(dict(id=lot_id, item=lot["item_id"], quantity=available, due=due, priority=priority))
    for machine, entries in sorted(fixed_orders.items()):
        entries.sort()
        require(len({entry[0] for entry in entries}) == len(entries) and [e[1] for e in entries] == sorted(e[1] for e in entries),
                "FIXED_ORDER", machine, "Fixed sequence contains duplicates or contradicts fixed starts")
        if len(entries) > 1:
            locks.append(dict(kind="order", resource=machine, tasks=[e[2] for e in entries]))
    for material, amount in running_reservations.items():
        inventory[material] = round(inventory[material] - amount, 6)
        require(inventory[material] >= 0, "RUNNING_MATERIAL", material,
                "Received stock cannot cover the unissued balance of running work")
    for ident, order in sorted(records["orders"].items()):
        members = [job["id"] for job in jobs if records["lots"][job["id"]]["order_id"] == ident]
        if members:
            orders.append(dict(id=ident, jobs=members, due=clock.seconds(order["due"]), priority=PRIORITIES[order["priority"]]))
    missing = sorted(set(records["personnel"]) - set(skills))
    if missing:
        warnings.append(dict(code="MISSING_QUALIFICATIONS", ids=missing, message="People without Excel qualifications have no eligible modes"))
    outside = [plan["operation_id"] for plan in proposals
               if plan["start"] is not None and clock.seconds(plan["start"]) >= clock.horizon]
    if outside:
        warnings.append(dict(code="PROPOSAL_HORIZON", ids=outside,
                             message=f"{len(outside)} Excel proposed starts fall at/after the planning horizon. Review the horizon explicitly before planning."))
    time_basis = dict(mode="fixed", as_of=clock.date(snapshot["factory"]["as_of"]).isoformat(),
                      timezone=snapshot["factory"]["timezone"], plan_start=clock.epoch.isoformat(),
                      source_horizon_end=clock.date(snapshot["factory"]["horizon_end"]).isoformat(),
                      horizon_end=clock.end.isoformat())
    assumptions = [
        f"Frozen demo snapshot: {time_basis['as_of']} ({time_basis['timezone']}). Today/now and overdue-at-snapshot queries use this instant, never the host date, chat date or import time. Do not shift source dates.",
        "Overdue at snapshot compares unfinished work's due date with the frozen snapshot. Predicted plan lateness compares validated completion with due date; the snapshot is not a predicted completion.",
        "Only unfinished released MES operations are scheduled; completed/skipped history remains in the import report.",
        "MES facts override workbook copies of orders, calendars and material; Excel proposals are not hard commitments unless Fixed=Yes.",
        "Excel setup/run allowances apply to the proposed machine; run allowances scale to current input quantity. Other machines use released MES rates.",
        "Running work is uninterrupted since actual_start with setup followed by run; remaining duration is the allowance minus observed elapsed time.",
        "Unissued materials for running operations are reserved from current stock before planning; previously posted issues are not consumed twice.",
        "No additional scrap is forecast. Completed order/lot metrics are outside this remaining-work snapshot.",
        "Priorities map Normal=1, High=2, Urgent=3; dates are soft due dates. No additional factory scheduling policies are activated.",
    ]
    problem = dict(id="demo-" + provenance["mes_sha256"][:12] + "-" + provenance["workbook_sha256"][:12],
                   epoch=clock.epoch.isoformat(), horizon=clock.horizon, resources=resources, tasks=tasks,
                   dependencies=dependencies, locks=locks, inventory=inventory, receipts=receipts,
                   jobs=jobs, orders=orders, assumptions=assumptions)
    report = dict(provenance=provenance, factory_snapshot=snapshot["factory"]["as_of"], time_basis=time_basis, assumptions=assumptions,
                  counts=dict(mes_operations=len(records["operations"]), planned_operations=len(tasks),
                              closed_operations=len(excluded), excel_operations=len(plans), resources=len(resources)),
                  operations_without_excel=sorted(t["id"] for t in tasks if t["id"] not in plans),
                  closed_operations=excluded, running_material_reserved=dict(running_reservations),
                  proposals=proposals, warnings=warnings)
    return problem, report


def raise_fixed(plan, message):
    require(False, "FIXED_EXECUTION", plan["_row"], message)
