"""Declared workplan revisions and immutable production instruction snapshots."""
import json


def rows(db, entity):
    return [json.loads(r[0]) for r in db.execute("SELECT data FROM records WHERE entity=? ORDER BY id", (entity,))]


def get(db, entity, ident):
    row = db.execute("SELECT data FROM records WHERE entity=? AND id=?", (entity, ident)).fetchone()
    if row is None:
        raise ValueError(f"Unknown {entity} record: {ident}")
    return json.loads(row[0])


def update(db, entity, record, *, bump=True):
    clean = {k: v for k, v in record.items() if not k.startswith("_")}
    db.execute("UPDATE records SET data=?,version=version+? WHERE entity=? AND id=?",
               (json.dumps(clean), int(bump), entity, record["id"]))


def steps_for(db, routing_id):
    return sorted((s for s in rows(db, "routing_steps") if s["routing_id"] == routing_id and s["active"]),
                  key=lambda s: s["sequence"])


def options_for(db, step, item):
    return [dict(resource_id=m["resource_id"], setup_minutes=m["setup_minutes"],
                 minutes_per_unit=m["minutes_per_unit"], attendance=step["attendance"])
            for m in rows(db, "routing_modes") if m["step_id"] == step["id"] and m["active"]
            and m["size"] == item["size"] and m["material"] == item["material"]]


def requirements_for(db, step, item, quantity):
    return [dict(material_id=m["material_id"], quantity_per_unit=m["quantity_per_unit"],
                 quantity=round(quantity * m["quantity_per_unit"], 6))
            for m in rows(db, "routing_materials") if m["step_id"] == step["id"] and m["active"]
            and m["body_material"] in ("Any", item["material"])]


def validate_route(db, routing_id, item=None):
    steps = steps_for(db, routing_id)
    if not 1 <= len(steps) <= 30:
        raise ValueError("A workplan needs between 1 and 30 active steps")
    if len({s["sequence"] for s in steps}) != len(steps):
        raise ValueError("Active step numbers must be unique within a workplan")
    for step in steps:
        modes = [m for m in rows(db, "routing_modes") if m["step_id"] == step["id"] and m["active"]]
        if not modes:
            raise ValueError(f"{step['name']} needs at least one active machine alternative")
        keys = [(m["resource_id"], m["size"], m["material"]) for m in modes]
        if len(set(keys)) != len(keys):
            raise ValueError(f"{step['name']} has duplicate machine alternatives")
        for mode in modes:
            if get(db, "machines", mode["resource_id"])["group"] != step["group"]:
                raise ValueError("A machine alternative must belong to the step's resource group")
        for material in ("Aluminium", "Stainless steel"):
            needs = [r["material_id"] for r in rows(db, "routing_materials")
                     if r["step_id"] == step["id"] and r["active"] and r["body_material"] in ("Any", material)]
            if len(needs) != len(set(needs)):
                raise ValueError("A component must occur only once per step and body material")
        if item is not None and not options_for(db, step, item):
            raise ValueError(f"{step['name']} has no machine alternative for {item['id']}")
    if item is None:
        # A released revision must have at least one usable end-to-end product configuration.
        configurations = [{(m["size"], m["material"]) for m in rows(db, "routing_modes")
                           if m["step_id"] == s["id"] and m["active"]} for s in steps]
        if not set.intersection(*configurations):
            raise ValueError("No body size/material combination can pass all workplan steps")
    return steps


def check_link(db, item, routing_id):
    route = get(db, "routings", routing_id)
    if route["status"] != "Released" or route["family"] != item["variant"]:
        raise ValueError("Choose a released workplan for the article's product family")
    validate_route(db, routing_id, item)


def guard_master_edit(db, entity, data, old=None):
    if entity in ("routing_steps", "routing_modes", "routing_materials"):
        route_id = data["routing_id"] if entity == "routing_steps" else get(db, "routing_steps", data["step_id"])["routing_id"]
        if get(db, "routings", route_id)["status"] != "Draft":
            raise ValueError("Released workplans are locked. Create a draft revision first.")
    if entity == "routings" and old and old["status"] != "Draft":
        raise ValueError("Released workplans are locked. Create a draft revision first.")
    if entity == "routing_steps" and not 1 <= data["sequence"] <= 999:
        raise ValueError("Step number must be between 1 and 999")
    if entity == "routing_modes":
        if data["minutes_per_unit"] <= 0:
            raise ValueError("Minutes per piece must be positive")
        if get(db, "machines", data["resource_id"])["group"] != get(db, "routing_steps", data["step_id"])["group"]:
            raise ValueError("A machine alternative must belong to the step's resource group")
    if entity == "routing_materials" and data["quantity_per_unit"] <= 0:
        raise ValueError("Component quantity per piece must be positive")
    if entity == "item_routings":
        check_link(db, get(db, "items", data["item_id"]), data["routing_id"])
        if any(r["id"] != data["id"] and r["item_id"] == data["item_id"] and r["routing_id"] == data["routing_id"]
               for r in rows(db, "item_routings")):
            raise ValueError("This article/workplan link already exists")
        if not data["active"] and get(db, "items", data["item_id"])["routing_id"] == data["routing_id"]:
            raise ValueError("Change the article's default workplan before disabling this link")
    if entity == "items":
        check_link(db, data, data["routing_id"])
        for link in rows(db, "item_routings"):
            if link["item_id"] == data["id"] and link["active"]:
                check_link(db, data, link["routing_id"])
        if old and not any(r["item_id"] == data["id"] and r["routing_id"] == data["routing_id"] and r["active"]
                           for r in rows(db, "item_routings")):
            raise ValueError("Approve this workplan in Article workplans before making it the default")


def release_order(db, order, item, lot_size, release, first_lot):
    routing_id = order.get("routing_id") or item["routing_id"]
    check_link(db, item, routing_id)
    if not any(r["item_id"] == item["id"] and r["routing_id"] == routing_id and r["active"] for r in rows(db, "item_routings")):
        raise ValueError("This workplan is not approved for the article")
    route, steps = get(db, "routings", routing_id), steps_for(db, routing_id)
    instructions = [(step, options_for(db, step, item), requirements_for(db, step, item, 1)) for step in steps]
    order["routing_id"] = routing_id
    lots, operations = [], []
    remaining = order["quantity"]
    while remaining:
        qty = min(lot_size, remaining)
        ident = f"L{first_lot + len(lots):05d}"
        lots.append(dict(id=ident, order_id=order["id"], item_id=item["id"], quantity=qty,
                         release=release, routing_id=routing_id, routing_revision=route["revision"],
                         status="Released", location="Raw material store", good_quantity=0, scrap_quantity=0, note=""))
        for step, options, needs in instructions:
            default = options[0]
            operations.append(dict(id=f"{ident}-{step['sequence']:03d}", lot_id=ident, sequence=step["sequence"],
                name=step["name"], group=step["group"], skill=step["skill"], instruction=step["instruction"], output=step["output"],
                setup_minutes=default["setup_minutes"], run_minutes=round(qty * default["minutes_per_unit"], 2),
                machine_options=options, material_requirements=[dict(m, quantity=round(m["quantity_per_unit"]*qty, 6)) for m in needs],
                status="Waiting", completed_quantity=0, scrap_quantity=0, resource_id="", person_id="",
                actual_start="", actual_end="", scrap_reason=""))
        remaining -= qty
    return lots, operations


def revise(store, db, source, payload):
    ident = payload.get("id", "")
    if not isinstance(ident, str) or len(ident) > 24:
        raise ValueError("Use a workplan ID of at most 24 characters")
    route = dict(id=ident, name=payload.get("name") or source["name"], family=source["family"],
                 revision=payload.get("revision", ""), status="Draft")
    store.validate(db, "routings", route)
    store.insert(db, "routings", route)
    for index, step in enumerate([s for s in rows(db, "routing_steps") if s["routing_id"] == source["id"]], 1):
        old_id = step["id"]
        step.update(id=f"{ident}-S{index*10}", routing_id=ident)
        store.insert(db, "routing_steps", step)
        for entity, prefix in [("routing_modes", "M"), ("routing_materials", "B")]:
            for i, child in enumerate([r for r in rows(db, entity) if r["step_id"] == old_id], 1):
                child.update(id=f"{step['id']}-{prefix}{i}", step_id=step["id"])
                store.insert(db, entity, child)
    return route
