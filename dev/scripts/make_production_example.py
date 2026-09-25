"""Generate fictional production orders with alternative complete workplans."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "app"
base = json.loads((ROOT / "examples/shift-factory.json").read_text(encoding="utf-8"))
base.update(schema_version="apex.v3.2", id="synthetic-route-factory", tasks=[], dependencies=[],
            transitions=[], locks=[], rules=[], inventory={"RAW": 100}, receipts=[], objectives=[])


def mode(resource, fixed, per_unit=0, shared=()):
    return {"id": f"on-{resource}", "primary": resource, "phases": [{
        "id": "process", "work": fixed, "work_per_unit": per_unit,
        "interruption": "calendar_resumable", "rate_resource": resource,
        "requirements": [{"resource": resource, "amount": 1, "retain": True}]
        + [{"resource": other, "amount": 1, "retain": False} for other in shared],
    }]}


def conditional(name, resource, seconds, release=False):
    return {"id": name, "modes": [mode(resource, seconds)], "after": [], "releases_product": release}


workplans = []
for plan, cutter, rate in [("standard", "CUT0", 55), ("express", "CUT1", 35)]:
    cut = mode(cutter, 0, rate, ["TOOL"])
    cut["pre"] = [conditional("prepare-tool", "TOOL", 90 if plan == "standard" else 150),
                  conditional("check-program", "OPS", 60)]
    cut["post"] = [conditional("quality-check", "OPS", 45, True), conditional("clean", cutter, 60)]
    cut["cost"] = 20 if plan == "standard" else 80
    workplans.append({"id": plan, "item": "PRODUCT", "tasks": [
        {"id": "cut", "family": "product", "quantity": 1, "attributes": {"urgency": 2},
         "modes": [cut], "consume": {"RAW": 1}, "produce": {"$job:WIP": 1}},
        {"id": "assemble", "quantity": 1, "attributes": {"urgency": 2},
         "modes": [mode("ASSEMBLY", 30, 40, ["OPS"])], "consume": {"$job:WIP": 1}, "produce": {"PRODUCT": 1}},
    ], "dependencies": [{"before": "cut", "after": "assemble"}]})
demands = [{"id": f"ORDER-{i+1}", "item": "PRODUCT", "quantity": quantity,
            "max_lot": 8, "workplans": ["standard", "express"],
            "due": 1000 + i * 750, "priority": 3 - i}
           for i, quantity in enumerate([12, 8, 16])]
result = {"problem": base, "workplans": workplans, "demands": demands}
(ROOT / "examples/production-orders.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
