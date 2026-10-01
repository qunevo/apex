"""Production confirmations, component issues and quantity conservation."""
from datetime import datetime, timedelta
import re

from .workplans import get, rows, update


def balances(db, at):
    result = {m["id"]: m["stock"] for m in rows(db, "materials")}
    for receipt in rows(db, "receipts"):
        if receipt["status"] == "Received" and receipt["available_at"] <= at:
            result[receipt["material_id"]] += receipt["quantity"]
    for issue in rows(db, "material_issues"):
        if issue["at"] <= at:
            result[issue["material_id"]] -= issue["quantity"]
    return {key: round(value, 6) for key, value in result.items()}


def check_stock(db):
    stock = {m["id"]: m["stock"] for m in rows(db, "materials")}
    events = [(r["available_at"], 0, r["material_id"], r["quantity"])
              for r in rows(db, "receipts") if r["status"] == "Received"]
    events += [(r["at"], 1, r["material_id"], -r["quantity"]) for r in rows(db, "material_issues")]
    for _, _, material, quantity in sorted(events):
        stock[material] += quantity
        if stock[material] < -1e-6:
            raise ValueError("This change would make posted material stock negative")


def timestamp(value, label):
    if not isinstance(value, str) or not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}", value):
        raise ValueError(f"{label} must use local date and time")
    return datetime.fromisoformat(value)


def overlap(a, b, c, d):
    return a < d and c < b


def in_shift(db, calendar_id, start, end):
    shift = get(db, "shifts", calendar_id)
    if start.date() != end.date() or start.weekday() >= 5:
        return False
    day = start.strftime("%Y-%m-%dT")
    begin, finish, pause, resume = [datetime.fromisoformat(day + shift[k]) for k in ("start", "end", "break_start", "break_end")]
    return (begin <= start < finish and start <= end <= finish and not overlap(start, end, pause, resume)
            and not (start == end and pause <= start < resume))


def report(store, db, operation_id, payload):
    allowed = {"expected_version", "completed_quantity", "scrap_quantity", "resource_id", "person_id", "actual_start", "actual_end", "scrap_reason"}
    if set(payload) - allowed:
        raise ValueError("Unknown production confirmation field")
    op = get(db, "operations", operation_id)
    if op["status"] in ("Complete", "Skipped"):
        raise ValueError("This operation is closed; its confirmation history is immutable")
    lot = get(db, "lots", op["lot_id"])
    if lot.get("quality_status") == "Hold":
        raise ValueError("Release the lot's quality hold before booking production")
    operations = sorted((o for o in rows(db, "operations") if o["lot_id"] == lot["id"]), key=lambda o: o["sequence"])
    predecessors = [o for o in operations if o["sequence"] < op["sequence"]]
    if any(o["status"] not in ("Complete", "Skipped") for o in predecessors):
        raise ValueError("Complete the previous operation first")
    previous = predecessors[-1] if predecessors else None
    available = previous["completed_quantity"] if previous else lot["quantity"]
    good, scrap = payload.get("completed_quantity"), payload.get("scrap_quantity", 0)
    for label, value, old in [("Good", good, op["completed_quantity"]), ("Scrap", scrap, op.get("scrap_quantity", 0))]:
        if isinstance(value, bool) or not isinstance(value, int) or not old <= value <= available:
            raise ValueError(f"{label} quantity must be a whole number between the booked and available input quantities")
    if good + scrap > available:
        raise ValueError("Good plus scrap cannot exceed the usable input from the previous step")
    good_delta, scrap_delta = good - op["completed_quantity"], scrap - op.get("scrap_quantity", 0)
    reason = payload.get("scrap_reason", "")
    if not isinstance(reason, str) or len(reason) > 2000 or (scrap_delta and not reason.strip()):
        raise ValueError("Enter a scrap reason when rejecting pieces")
    complete = good + scrap == available
    at = store.meta(db, "factory")["as_of"]
    start = timestamp(payload.get("actual_start"), "Actual start")
    end_text = payload.get("actual_end", "")
    if complete != bool(end_text):
        raise ValueError("Enter an actual finish exactly when all input pieces are accounted for")
    end = timestamp(end_text or at, "Actual finish")
    if start < timestamp(lot["release"], "Lot release") or start > end or end > timestamp(at, "Snapshot") or ((complete or good_delta + scrap_delta) and start == end):
        raise ValueError("Execution must lie between lot release and the demo snapshot, with finish after start")
    if previous and previous.get("actual_end") and payload["actual_start"] < previous["actual_end"]:
        raise ValueError("Actual start cannot precede the previous operation's finish")
    observed = [c["recorded_at"] for c in rows(db, "confirmations")
                if c["operation_id"] == op["id"] and not c["actual_end"]]
    if observed and (end_text or at) < max(observed):
        raise ValueError("Finish cannot precede an earlier in-progress confirmation")
    machine_id, person_id = payload.get("resource_id"), payload.get("person_id")
    if op["actual_start"] and any(payload.get(key) != op[key] for key in ("actual_start", "resource_id", "person_id")):
        raise ValueError("Start, machine and operator are fixed after the first confirmation")
    mode = next((m for m in op["machine_options"] if m["resource_id"] == machine_id), None)
    if mode is None:
        raise ValueError("Select a machine from this operation's released alternatives")
    machine, person = get(db, "machines", machine_id), get(db, "personnel", person_id)
    if machine["permanently_unavailable"]:
        raise ValueError("This equipment is permanently unavailable")
    if not in_shift(db, machine["calendar"], start, end):
        raise ValueError("Execution must fit one equipment shift window")
    for period in rows(db, "downtime"):
        if period["resource_id"] == machine_id and not period.get("cancelled") and (
            overlap(start, end, timestamp(period["start"], "Period"), timestamp(period["end"], "Period"))
            or (start == end and period["start"] <= payload["actual_start"] < period["end"])):
            raise ValueError("The equipment is unavailable during execution")
    qualified = next((p for p in store.meta(db, "qualifications") if p["person_id"] == person_id), {})
    if person["attendance"] != "Present" or not qualified.get(op["skill"]):
        raise ValueError("Select a present operator with the required skill in the Excel snapshot")
    labor_end = min(end, start + timedelta(minutes=mode["setup_minutes"])) if mode["attendance"] == "Setup only" else end
    if not in_shift(db, person["shift_id"], start, labor_end):
        raise ValueError("Operator attendance must fit the assigned shift, excluding breaks")
    for absence in rows(db, "absences"):
        if absence["person_id"] == person_id and not absence.get("cancelled") and (
            overlap(start, labor_end, timestamp(absence["start"], "Absence"), timestamp(absence["end"], "Absence"))
            or (start == labor_end and absence["start"] <= payload["actual_start"] < absence["end"])):
            raise ValueError("The operator is absent during execution")
    for other in rows(db, "operations"):
        if other["id"] == op["id"] or not other.get("actual_start") or other["status"] == "Skipped":
            continue
        a, b = timestamp(other["actual_start"], "Start"), timestamp(other["actual_end"] or at, "Finish")
        if other["resource_id"] == machine_id and (overlap(start, end, a, b)
            or (start == end and a <= start and (start < b or (start == b and not other["actual_end"])))):
            raise ValueError(f"The equipment already has an execution booking: {other['id']}")
        other_mode = next((m for m in other["machine_options"] if m["resource_id"] == other["resource_id"]), None)
        if other_mode and other_mode["attendance"] == "Setup only":
            b = min(b, a + timedelta(minutes=other_mode["setup_minutes"]))
        if other.get("person_id") == person_id and (overlap(start, labor_end, a, b)
            or (start == labor_end and a <= start < b)):
            raise ValueError(f"The operator already has an attendance booking: {other['id']}")
    ident = f"CF-{store.meta(db, 'revision')+1:06d}"
    store.insert(db, "confirmations", dict(id=ident, operation_id=op["id"], good_delta=good_delta, scrap_delta=scrap_delta,
        resource_id=machine_id, person_id=person_id, actual_start=payload["actual_start"], actual_end=end_text, recorded_at=at, note=reason))
    for i, need in enumerate(op["material_requirements"], 1):
        qty = round((good_delta + scrap_delta) * need["quantity_per_unit"], 6)
        if qty:
            store.insert(db, "material_issues", dict(id=f"{ident}-{i}", operation_id=op["id"], confirmation_id=ident,
                         material_id=need["material_id"], quantity=qty, at=end_text or at))
    if any(qty < -1e-6 for qty in balances(db, at).values()):
        raise ValueError("Not enough received material for this confirmation; check Material stock and Inbound deliveries")
    check_stock(db)
    op.update(completed_quantity=good, scrap_quantity=scrap, resource_id=machine_id, person_id=person_id,
              actual_start=payload["actual_start"], actual_end=end_text, scrap_reason=reason,
              status="Complete" if complete else "Running")
    update(db, "operations", op)
    if complete and good == 0:
        for following in operations:
            if following["sequence"] > op["sequence"]:
                following.update(status="Skipped", actual_end=end_text)
                update(db, "operations", following)
    from .seed import refresh_progress
    records = {key: rows(db, key) for key in ("operations", "lots", "orders")}
    before = {key: {r["id"]: r.copy() for r in records[key]} for key in ("lots", "orders")}
    refresh_progress(records)
    for key in ("lots", "orders"):
        for row in records[key]:
            if row != before[key][row["id"]]:
                update(db, key, row)
    return {"saved": True, "confirmation_id": ident}
