"""Generate deliberately synthetic fixtures; no source-system or customer data."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "app"


def window(start, end, capacity=1, rate=1):
    return dict(start=start, end=end, capacity=capacity, rate=rate)


def requirement(resource, retain=False):
    return dict(resource=resource, amount=1, retain=retain)


def phase(name, work, resources, interruption="calendar_resumable"):
    return dict(id=name, work=work, interruption=interruption,
                rate_resource=resources[0]["resource"], requirements=resources)


def mode(name, primary, phases):
    return dict(id=name, primary=primary, phases=phases, contiguous=False, cost=0)


def conditional(name, machine, work, release=False):
    return dict(id=name, releases_product=release,
                modes=[mode(name, machine, [phase(name, work, [
                    requirement(machine, True), requirement("OPS")])])])


def generate():
    resources = []
    for name in ["CUT0", "CUT1", "ASSEMBLY", "OPS", "TOOL"]:
        capacity = 2 if name == "OPS" else 1
        resources.append(dict(id=name, capacity=capacity, initial_state="light",
                              calendar=[window(0, 1800, capacity),
                                        window(2100, 5400, capacity),
                                        window(5400, 14400, capacity, 0.8)],
                              retention_calendar=[]))
    tasks, dependencies = [], []
    for job in range(8):
        family = "dark" if job % 2 else "light"
        cutting_modes = []
        for index, machine in enumerate(["CUT0", "CUT1"]):
            cutting_modes.append(mode(machine, machine, [
                phase("load", 90, [requirement(machine, True), requirement("OPS")]),
                phase("cut", 300 + job * 30 + index * 45,
                      [requirement(machine, True), requirement("TOOL", True)])]))
        cut_id, assembly_id = f"J{job}-cut", f"J{job}-assemble"
        common = dict(release=0, due=1800 + job * 350, deadline=None,
                      priority=1 + job % 3, family=family,
                      attributes={"urgency": 1 + job % 5}, execution=None,
                      source=f"synthetic:job:{job}")
        tasks.append(dict(common, id=cut_id, modes=cutting_modes, pre=[], post=[],
                          consume={"RAW": 1}, produce={f"WIP-{job}": 1}))
        tasks.append(dict(common, id=assembly_id,
                          modes=[mode("assembly", "ASSEMBLY", [
                              phase("assemble", 240, [requirement("ASSEMBLY", True),
                                                     requirement("OPS")])])],
                          pre=[], post=[conditional("inspection", "ASSEMBLY", 60, True)],
                          consume={f"WIP-{job}": 1}, produce={"FINISHED": 1}))
        dependencies.append(dict(before=cut_id, after=assembly_id, min_lag=0))
    transitions = []
    for machine in ["CUT0", "CUT1"]:
        for previous in ["light", "dark"]:
            for following in ["light", "dark", "__end__"]:
                transitions.append(dict(
                    id=f"{machine}-{previous}-{following}", resource=machine,
                    **{"from": previous, "to": following},
                    previous_post=([conditional("clean", machine, 90)]
                                   if following == "__end__" else []),
                    next_pre=([conditional("setup", machine, 60)]
                              if following != "__end__" and previous != following else [])))
    problem = dict(schema_version="apex.v3.1", id="synthetic-shift-factory",
                   epoch="2026-09-24T06:00:00Z", horizon=14400, resources=resources,
                   tasks=tasks, dependencies=dependencies, locks=[],
                   transitions=transitions, inventory={"RAW": 8}, receipts=[],
                   objectives=[dict(metric="weighted_tardiness", weight=1, priority=0),
                               dict(metric="transition_work", weight=1, priority=1)],
                   rules=[], assumptions=[])
    target = ROOT / "examples" / "shift-factory.json"
    target.write_text(json.dumps(problem, indent=2) + "\n", encoding="utf-8")
    print(target)


if __name__ == "__main__":
    generate()
