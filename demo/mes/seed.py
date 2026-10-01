"""Deterministic, deliberately fictional business records and planner baseline."""
from datetime import datetime, timedelta
import json
from pathlib import Path
import random

from .catalog import CATALOG, ROUTES, SKILLS, STEPS
from .availability import normalize_machine

DEMO = Path(__file__).resolve().parents[1]


def stamp(value):
    return value.isoformat(timespec="minutes")


def expand_order(order, item, *, lot_size, release, first_lot):
    lots, operations = [], []
    remaining = order["quantity"]
    while remaining:
        qty = min(lot_size, remaining)
        lot_id = f"L{first_lot + len(lots):05d}"
        lots.append(dict(id=lot_id, order_id=order["id"], item_id=item["id"], quantity=qty,
                         release=release, status="Released", location="Raw material store", note=""))
        factor = {"Compact": .8, "Standard": 1, "Large": 1.35}[item["size"]]
        for step_no, step in enumerate(ROUTES[item["routing_id"]], 1):
            name, group, skill, per_unit, setup = STEPS[step]
            multiplier = factor * (1.25 if item["material"] == "Stainless steel" and step == "machine" else 1)
            operations.append(dict(id=f"{lot_id}-{step_no * 10:03d}", lot_id=lot_id,
                sequence=step_no * 10, name=name, group=group, skill=skill,
                setup_minutes=setup, run_minutes=round(qty * per_unit * multiplier),
                status="Waiting", completed_quantity=0, resource_id=""))
        remaining -= qty
    return lots, operations


def create_records(config):
    rng = random.Random(config["seed"])
    rows = {name: [] for name in CATALOG}
    rows["shifts"] = [
        dict(id="EARLY", name="Early shift", start="06:00", end="14:00", break_start="10:00", break_end="10:30"),
        dict(id="LATE", name="Late shift", start="14:00", end="22:00", break_start="18:00", break_end="18:30"),
        dict(id="TWO", name="Machine two-shift window", start="06:00", end="22:00", break_start="22:00", break_end="22:00"),
    ]
    for key, name in [("RT-D", "Distributor"), ("RT-R", "Regulator"), ("RT-S", "Sensor")]:
        rows["routings"].append(dict(id=key, name=name, steps=" / ".join(STEPS[s][0] for s in ROUTES[key]), revision="A"))
    for variant, route in [("Distributor", "RT-D"), ("Regulator", "RT-R"), ("Sensor", "RT-S")]:
        for size in ["Compact", "Standard", "Large"]:
            for metal, suffix in [("Aluminium", "AL"), ("Stainless steel", "SS")]:
                rows["items"].append(dict(id=f"{route[-1]}-{size[0]}-{suffix}",
                    name=f"{size} {variant.lower()} / {suffix}", variant=variant, size=size,
                    material=metal, lot_size=20, routing_id=route))
    groups = [("CNC", 6), ("Deburr", 2), ("Wash", 2), ("Assembly", 8),
              ("Electronics", 2), ("Calibration", 1), ("Test", 3)]
    for group, count in groups:
        prefix = {"CNC": "CNC", "Deburr": "DB", "Wash": "WS", "Assembly": "AS",
                  "Electronics": "EL", "Calibration": "CAL", "Test": "QA"}[group]
        for n in range(1, count + 1):
            capability = "All variants"
            if group == "CNC":
                capability = "Small / medium aluminium" if n <= 2 else "Universal" if n <= 4 else "Large / stainless"
            if group == "Test":
                capability = "Distributor only" if n == 1 else "Universal function test"
            rows["machines"].append(dict(id=f"{prefix}-{n:02d}", name=f"{group} {n:02d}",
                group=group, capability=capability, calendar="TWO", permanently_unavailable=False, note=""))
    first_names = ["Alex", "Blair", "Casey", "Drew", "Ellis", "Finley", "Harper", "Jamie", "Jules", "Kit", "Morgan", "Noel"]
    qualifications = []
    for i in range(24):
        team = "Machining" if i % 12 < 3 else "Assembly" if i % 12 < 9 else "Quality"
        employee = dict(id=f"P{i+1:02d}", name=f"{first_names[i % 12]} {['Aster', 'Vale'][i // 12]}",
                        team=team, shift_id="EARLY" if i < 12 else "LATE", attendance="Present", note="")
        rows["personnel"].append(employee)
        skills = ["Mechanical"]
        if team == "Machining":
            skills += ["Setup"]
        if i % 12 in [4, 5, 7, 8]:
            skills += ["Precision"]
        if i % 12 in [6, 7, 8]:
            skills += ["Electrical"]
        if i % 12 in [8, 9]:
            skills += ["Calibration"]
        if team == "Quality":
            skills += ["Testing"]
        qualifications.append(dict(person_id=employee["id"], name=employee["name"],
                                   **{key: key in skills for key in SKILLS}))
    for ident, name, stock, threshold in [
        ("BODY-AL", "Aluminium body blanks", 8500, 1500), ("BODY-SS", "Stainless body blanks", 7500, 1200),
        ("SEAL-KIT", "Seal and closure kit", 8000, 2000), ("VALVE-KIT", "Control valve kit", 6000, 1200),
        ("SENSOR-KIT", "Sensor and cable kit", 600, 800),
    ]:
        rows["materials"].append(dict(id=ident, name=name, unit="pcs", stock=stock,
                                     location="Stores A" if ident.startswith("BODY") else "Stores B", reorder_point=threshold))
    rows["receipts"] = [
        dict(id="IN-1001", material_id="SENSOR-KIT", quantity=4000, available_at="2026-10-07T10:00", status="Confirmed", note="Supplier-confirmed Wednesday delivery"),
        dict(id="IN-1002", material_id="SEAL-KIT", quantity=8000, available_at="2026-10-06T08:00", status="Confirmed", note="Weekly replenishment"),
        dict(id="IN-1003", material_id="VALVE-KIT", quantity=6000, available_at="2026-10-08T07:00", status="Confirmed", note="Reserved inbound shipment"),
    ]
    rows["downtime"] = [
        dict(id="DT-001", resource_id="CNC-03", start="2026-10-07T06:00", end="2026-10-07T10:00", reason="Spindle inspection"),
        dict(id="DT-002", resource_id="QA-03", start="2026-10-08T14:00", end="2026-10-08T17:00", reason="Reference instrument calibration"),
        dict(id="DT-003", resource_id="WS-02", start="2026-10-12T06:00", end="2026-10-12T09:00", reason="Bath service"),
    ]
    origin = datetime.fromisoformat(config["plan_start"])
    days = [origin + timedelta(days=i) for i in range(12) if (origin + timedelta(days=i)).weekday() < 5]
    for i in range(config["orders"]):
        item = rows["items"][(i * 7) % len(rows["items"])]
        qty = rng.choice([5, 10, 15, 20, 25])
        due = days[2 + i % 8].replace(hour=16)
        order = dict(id=f"SO-{26001+i}", customer=f"OEM {1+i % 12:02d}", item_id=item["id"],
            quantity=qty * config["lots_per_order"], due=stamp(due),
            priority="Urgent" if i % 23 == 0 else "High" if i % 7 == 0 else "Normal",
            status="Released", note="Ship all lots together" if i % 3 == 0 else "")
        rows["orders"].append(order)
        lots, operations = expand_order(order, item, lot_size=qty, release=config["plan_start"], first_lot=1+len(rows["lots"]))
        rows["lots"].extend(lots)
        rows["operations"].extend(operations)
    return rows, qualifications


def eligible(machine, item, operation):
    if machine["group"] != operation["group"]:
        return False
    if operation["group"] == "CNC":
        n = int(machine["id"][-2:])
        if n <= 2:
            return item["size"] != "Large" and item["material"] == "Aluminium"
        return True
    return not (machine["id"] == "QA-01" and item["variant"] != "Distributor")


def make_baseline(config, records, qualifications):
    """Reconstruct a conventional earliest-slot planner baseline, never an APEX result.

    Fixed lot sizes, due-date order, fixed setup allowance and greedy assignment.
    CNC workers attend setup only; other operations need continuous attendance.
    No sequence optimization or claim of a complete industrial model is made.
    """
    origin = datetime.fromisoformat(config["plan_start"])
    as_of = datetime.fromisoformat(config["as_of"])
    machines = records["machines"]
    people = records["personnel"]
    skills = {p["person_id"]: p for p in qualifications}
    occupied = {p["id"]: [] for p in machines + people}
    for down in records["downtime"]:
        if not down.get("cancelled", False):
            occupied[down["resource_id"]].append((datetime.fromisoformat(down["start"]), datetime.fromisoformat(down["end"])))
    items = {a["id"]: a for a in records["items"]}
    orders = {a["id"]: a for a in records["orders"]}
    ops = {}
    for op in records["operations"]:
        ops.setdefault(op["lot_id"], []).append(op)
    supply = {m["id"]: [[origin, m["stock"]]] for m in records["materials"]}
    for rec in records["receipts"]:
        supply[rec["material_id"]].append([datetime.fromisoformat(rec["available_at"]), rec["quantity"]])

    def allocate(material, quantity):
        when = origin
        for bucket in supply[material]:
            take = min(quantity, bucket[1])
            if take:
                when = max(when, bucket[0])
                quantity -= take
                bucket[1] -= take
        if quantity:
            raise ValueError(f"Seed material shortage: {material}")
        return when

    def slot(earliest, machine_id, person, minutes, attendance):
        duration, labor = timedelta(minutes=minutes), timedelta(minutes=attendance)
        for day in range(max(0, (earliest.date() - origin.date()).days), 50):
            date = origin.replace(hour=0, minute=0) + timedelta(days=day)
            if date.weekday() >= 5:
                continue
            hours = [(6, 10), (10.5, 14)] if person["shift_id"] == "EARLY" else [(14, 18), (18.5, 22)]
            for begin, end in hours:
                candidate = max(earliest, date + timedelta(hours=begin))
                latest = min(date + timedelta(hours=end) - labor, date + timedelta(hours=22) - duration)
                while candidate <= latest:
                    finish = candidate + duration
                    collisions = [b for a, b in occupied[machine_id] if a < finish and b > candidate]
                    collisions += [b for a, b in occupied[person["id"]] if a < candidate + labor and b > candidate]
                    if not collisions:
                        return candidate, finish
                    candidate = max(collisions)
        raise ValueError("Seed planner exceeded its bounded horizon")

    plan = []
    sequence = {}
    priority = {"Urgent": 0, "High": 1, "Normal": 2}
    lots = sorted(records["lots"], key=lambda x: (orders[x["order_id"]]["due"], priority[orders[x["order_id"]]["priority"]], x["id"]))
    for lot in lots:
        item, order = items[lot["item_id"]], orders[lot["order_id"]]
        material_ready = {"CNC": allocate("BODY-AL" if item["material"] == "Aluminium" else "BODY-SS", lot["quantity"]),
                          "Assembly": allocate("SEAL-KIT", lot["quantity"])}
        if item["variant"] != "Distributor":
            material_ready["Assembly"] = max(material_ready["Assembly"], allocate("VALVE-KIT", lot["quantity"]))
        if item["variant"] == "Sensor":
            material_ready["Electronics"] = allocate("SENSOR-KIT", lot["quantity"])
        ready = datetime.fromisoformat(lot["release"])
        for op in ops[lot["id"]]:
            minutes = op["setup_minutes"] + op["run_minutes"]
            labor = op["setup_minutes"] if op["group"] == "CNC" else minutes
            ready = max(ready, material_ready.get(op["group"], origin))
            choices = []
            for machine in machines:
                if not normalize_machine(machine)["permanently_unavailable"] and eligible(machine, item, op):
                    for person in people:
                        if skills[person["id"]][op["skill"]]:
                            start, end = slot(ready, machine["id"], person, minutes, labor)
                            scarce = sum(skills[person["id"]][key] for key in ["Setup", "Testing", "Calibration", "Electrical"] if key != op["skill"])
                            # A conventional planner protects scarce specialists when
                            # an ordinary operator is available within a few hours.
                            choices.append((end + timedelta(minutes=120 * scarce), end, start, machine["id"], person["id"]))
            _, end, start, machine_id, person_id = min(choices)
            occupied[machine_id].append((start, end))
            occupied[person_id].append((start, start + timedelta(minutes=labor)))
            sequence[machine_id] = sequence.get(machine_id, 0) + 1
            done = end <= as_of
            running = start <= as_of < end
            op["status"] = "Complete" if done else "Running" if running else "Waiting"
            op["completed_quantity"] = lot["quantity"] if done else int(lot["quantity"] * (as_of-start).total_seconds() / (minutes*60)) if running else 0
            op["resource_id"] = machine_id if done or running else ""
            plan.append(dict(operation_id=op["id"], lot_id=lot["id"], order_id=order["id"],
                item_id=item["id"], variant=item["variant"], quantity=lot["quantity"], operation=op["name"],
                machine_id=machine_id, person_id=person_id, sequence=sequence[machine_id], start=stamp(start), end=stamp(end),
                setup_minutes=op["setup_minutes"], run_minutes=op["run_minutes"], attendance_minutes=labor,
                due=order["due"], priority=order["priority"], fixed="Yes" if start < as_of + timedelta(hours=2) else "No",
                note="Sensor receipt required" if ready == material_ready.get("Electronics") and op["group"] == "Electronics" and ready > origin else ""))
            ready = end
    for machine_id in sequence:
        for n, entry in enumerate(sorted((p for p in plan if p["machine_id"] == machine_id), key=lambda p: p["start"]), 1):
            entry["sequence"] = n
    refresh_progress(records)
    return sorted(plan, key=lambda p: (p["start"], p["machine_id"]))


def refresh_progress(records):
    grouped = {}
    for op in records["operations"]:
        grouped.setdefault(op["lot_id"], []).append(op)
    for lot in records["lots"]:
        operations = sorted(grouped[lot["id"]], key=lambda o: o["sequence"])
        complete = all(o["status"] == "Complete" for o in operations)
        active = next((o for o in operations if o["status"] != "Complete"), None)
        started = any(o["status"] != "Waiting" for o in operations)
        lot["status"] = "Complete" if complete else "In progress" if started else "Released"
        lot["location"] = "Finished goods" if complete else active["group"] if started else "Raw material store"
    for order in records["orders"]:
        lots = [lot for lot in records["lots"] if lot["order_id"] == order["id"]]
        order["status"] = "Complete" if all(lot["status"] == "Complete" for lot in lots) else "In progress" if any(lot["status"] == "In progress" or lot["status"] == "Complete" for lot in lots) else "Released"


def generate():
    config = json.loads((DEMO / "data/factory.json").read_text(encoding="utf-8"))
    records, qualifications = create_records(config)
    plan = make_baseline(config, records, qualifications)
    return dict(factory=config, records=records, qualifications=qualifications, plan=plan)


if __name__ == "__main__":
    target = DEMO / ".local/seed.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    data = generate()
    target.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"path": str(target), "lots": len(data["records"]["lots"]), "operations": len(data["plan"])}))
