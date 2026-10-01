"""Add synthetic master data and preserve existing released business records."""
from copy import deepcopy

from .catalog import ROUTES, STEPS
from .workplans import get, options_for, requirements_for, rows, update


def install(store, db, seed, *, existing):
    if store.meta(db, "mes_schema") == 2:
        return
    # Legacy releases have no machine/BOM snapshot. Recover original seeded article
    # attributes where possible; user-created legacy lots use the current article.
    source_items = {r["id"]: r for r in seed["records"]["items"]}
    source_lots = {r["id"] for r in seed["records"]["lots"]}
    machines = rows(db, "machines")
    from .seed import eligible
    for route in rows(db, "routings"):
        route.update(family={"RT-D": "Distributor", "RT-R": "Regulator", "RT-S": "Sensor"}[route["id"]], status="Released")
        route.pop("steps", None)
        update(db, "routings", route, bump=existing)
    for base, family in [("RT-D", "Distributor"), ("RT-R", "Regulator"), ("RT-S", "Sensor")]:
        for split in (False, True):
            route_id = base + ("-SPLIT" if split else "")
            if split:
                store.insert(db, "routings", dict(id=route_id, name=f"{family} / separate roughing and drilling",
                             family=family, revision="A", status="Released"))
            keys = (["rough", "drill"] + ROUTES[base][1:]) if split else ROUTES[base]
            assembled = False
            for n, key in enumerate(keys, 1):
                name, group, skill, rate, setup = STEPS.get(key, {
                    "rough": ("Rough machining", "CNC", "Setup", 1.35, 14),
                    "drill": ("Finish drilling & threads", "CNC", "Setup", .85, 9),
                }.get(key, ()))
                ident = f"{route_id}-S{n*10}"
                output = "Machined body" if group == "CNC" else "Clean body" if group == "Wash" else "Tested finished valve" if group == "Test" else name + " complete"
                step = dict(id=ident, routing_id=route_id, sequence=n*10, name=name, group=group, skill=skill,
                            attendance="Setup only" if group == "CNC" else "Continuous", active=True,
                            instruction=f"{name}. Verify article and lot identity before processing. Record any rejected pieces.", output=output)
                store.insert(db, "routing_steps", step)
                counter = 0
                for size, factor in [("Compact", .8), ("Standard", 1), ("Large", 1.35)]:
                    for material in ("Aluminium", "Stainless steel"):
                        item = dict(size=size, material=material, variant=family)
                        for machine in machines:
                            if not eligible(machine, item, step):
                                continue
                            if key == "rough" and machine["id"] not in ("CNC-03", "CNC-05"):
                                continue
                            if key == "drill" and machine["id"] not in ("CNC-04", "CNC-06"):
                                continue
                            counter += 1
                            speed = .85 if split and machine["id"] in ("CNC-05", "CNC-06") else 1
                            store.insert(db, "routing_modes", dict(id=f"{ident}-M{counter}", step_id=ident,
                                resource_id=machine["id"], size=size, material=material, setup_minutes=setup,
                                minutes_per_unit=round(rate*factor*(1.25 if material == "Stainless steel" and group == "CNC" else 1)*speed, 4), active=True))
                needs = []
                if n == 1:
                    needs = [("BODY-AL", "Aluminium"), ("BODY-SS", "Stainless steel")]
                if group == "Assembly" and not assembled:
                    needs = [("SEAL-KIT", "Any")] + ([("VALVE-KIT", "Any")] if family != "Distributor" else [])
                    assembled = True
                if group == "Electronics":
                    needs = [("SENSOR-KIT", "Any")]
                for i, (material, applies) in enumerate(needs, 1):
                    store.insert(db, "routing_materials", dict(id=f"{ident}-B{i}", step_id=ident,
                                 material_id=material, body_material=applies, quantity_per_unit=1, active=True))
    for item in rows(db, "items"):
        for route_id in (item["routing_id"], item["routing_id"] + "-SPLIT"):
            store.insert(db, "item_routings", dict(id=f"{item['id']}-{route_id}", item_id=item["id"], routing_id=route_id, active=True))
    plan = {p["operation_id"]: p for p in seed.get("plan", [])}
    items = {i["id"]: i for i in rows(db, "items")}
    lots = {l["id"]: l for l in rows(db, "lots")}
    at = store.meta(db, "factory")["as_of"]
    for lot in lots.values():
        item = source_items.get(lot["item_id"], items[lot["item_id"]]) if lot["id"] in source_lots else items[lot["item_id"]]
        lot.update(routing_id=item["routing_id"], routing_revision=get(db, "routings", item["routing_id"])["revision"], good_quantity=0, scrap_quantity=0)
        update(db, "lots", lot, bump=existing)
    for order in rows(db, "orders"):
        order["routing_id"] = next(l["routing_id"] for l in lots.values() if l["order_id"] == order["id"])
        update(db, "orders", order, bump=existing)
    source_ops = {o["id"]: o for o in seed["records"]["operations"]}
    option_cache, need_cache = {}, {}
    for op in rows(db, "operations"):
        lot = lots[op["lot_id"]]
        item = source_items.get(lot["item_id"], items[lot["item_id"]]) if lot["id"] in source_lots else items[lot["item_id"]]
        step = get(db, "routing_steps", f"{lot['routing_id']}-S{op['sequence']}")
        key = (step["id"], item["size"], item["material"])
        if key not in option_cache:
            option_cache[key] = options_for(db, step, item)
        need_key = (*key, lot["quantity"])
        if need_key not in need_cache:
            need_cache[need_key] = requirements_for(db, step, item, lot["quantity"])
        op.update(machine_options=option_cache[key], material_requirements=need_cache[need_key],
                  instruction=step["instruction"], output=step["output"], scrap_quantity=0, scrap_reason="",
                  actual_start="", actual_end="", person_id="")
        baseline = plan.get(op["id"])
        original = source_ops.get(op["id"], {})
        known_baseline = baseline and baseline["start"] <= at and all(op[k] == original.get(k) for k in ("status", "completed_quantity", "resource_id"))
        if op["status"] != "Waiting":
            if known_baseline:
                op.update(actual_start=baseline["start"], actual_end=baseline["end"] if op["status"] == "Complete" else "", person_id=baseline["person_id"])
            confirmation = dict(id=f"SEED-{op['id']}", operation_id=op["id"], good_delta=op["completed_quantity"], scrap_delta=0,
                resource_id=op["resource_id"], person_id=op["person_id"], actual_start=op["actual_start"], actual_end=op["actual_end"],
                recorded_at=at, note="Synthetic baseline" if known_baseline else "Legacy booking; execution times were not recorded")
            store.insert(db, "confirmations", confirmation)
            for i, requirement in enumerate(op["material_requirements"], 1):
                quantity = op["completed_quantity"] * requirement["quantity_per_unit"]
                if quantity:
                    store.insert(db, "material_issues", dict(id=f"SEED-{op['id']}-{i}", operation_id=op["id"],
                        confirmation_id=confirmation["id"], material_id=requirement["material_id"], quantity=quantity, at=op["actual_start"] or at))
        update(db, "operations", op, bump=existing)
    from .seed import refresh_progress
    records = {key: deepcopy(rows(db, key)) for key in ("orders", "lots", "operations")}
    refresh_progress(records)
    for entity in ("lots", "orders"):
        for row in records[entity]:
            update(db, entity, row, bump=False)
    store.put_meta(db, "mes_schema", 2)
