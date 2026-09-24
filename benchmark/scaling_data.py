"""Outcome-independent size coverage for the expanded public benchmark."""
from __future__ import annotations

import hashlib
import json
from collections import defaultdict

from instances import DATA, fetch, fjsp, jsp, pfsp, validate_instance

JSP_SIZES = [(15, 15), (20, 15), (20, 20), (30, 15), (30, 20),
             (50, 15), (50, 20), (100, 20)]
PFSP_SIZES = [(20, 5), (20, 10), (20, 20), (50, 5), (50, 10), (50, 20),
              (100, 5), (100, 10), (100, 20), (200, 10), (200, 20), (500, 20)]
LEGACY_IDS = ["ft06", "la01", "la06", "la16", "abz5"] + [f"mk{i:02}" for i in range(1, 6)] + [f"ta{i:03}" for i in range(1, 6)]
SELECTION_SALT = "APEX-SCALE-20260924-v1"
TAILLARD_URL = "https://mistic.iict-heig-vd.ch/taillard/problemes.dir/ordonnancement.dir"


def taillard_jsp(text: str, offset: int) -> list[dict]:
    """Read job-major times and one-based machine routes from the author files."""
    lines = text.splitlines()
    instances = []
    for i, line in enumerate(lines):
        if "nb of jobs" not in line.lower():
            continue
        n, m, time_seed, machine_seed, upper, lower = map(int, lines[i + 1].split())
        assert lines[i + 2].strip().lower() == "times"
        times = [list(map(int, lines[i + 3 + j].split())) for j in range(n)]
        assert lines[i + 3 + n].strip().lower() == "machines"
        machines = [list(map(int, lines[i + 4 + n + j].split())) for j in range(n)]
        assert all(len(row) == m for row in times)
        assert all(sorted(row) == list(range(1, m + 1)) for row in machines)
        instances.append(dict(
            id=f"tai_jsp{offset + len(instances) + 1:03}", kind="JSP", machines=m,
            jobs=[[[[machines[j][k] - 1, times[j][k]]] for k in range(m)] for j in range(n)],
            source_time_seed=time_seed, source_machine_seed=machine_seed,
            source_makespan_upper=upper, source_makespan_lower=lower,
            bound_status="Historical source values, not asserted current optima",
        ))
    assert len(instances) == 10
    return instances


def selection_rank(instance: dict) -> str:
    """Rank identifiers only; neither processing times nor results affect selection."""
    return hashlib.sha256(f"{SELECTION_SALT}|{instance['kind']}|{instance['id']}".encode()).hexdigest()


def select_instances(candidates: list[dict]) -> list[dict]:
    selected = set(LEGACY_IDS)
    selected.update(row["id"] for row in candidates if row["family"] == "Brandimarte")
    blocks = defaultdict(list)
    for row in candidates:
        if row["family"] in {"Taillard", "Behnke-Geiger"}:
            blocks[row["kind"], len(row["jobs"]), row["machines"]].append(row)
    assert len(blocks) == len(JSP_SIZES) + len(PFSP_SIZES) + 12
    for block in blocks.values():
        assert len(block) == (5 if block[0]["family"] == "Behnke-Geiger" else 10)
        selected.update(row["id"] for row in sorted(block, key=selection_rank)[:2])
    assert set(LEGACY_IDS) <= {row["id"] for row in candidates}
    return [row for row in candidates if row["id"] in selected]


def prepare_expanded() -> tuple[list[dict], list[dict]]:
    """Acquire the declared candidate frame and retain every inclusion decision."""
    candidates = []
    raw, source = fetch("https://people.brunel.ac.uk/~mastjjb/jeb/orlib/files/jobshop1.txt", "jobshop1.txt")
    for name in LEGACY_IDS[:5]:
        candidates.append(jsp(raw, name) | dict(source=source, family="OR-Library legacy"))
    for index in range(1, 16):
        name = f"mk{index:02}"
        raw, source = fetch(f"https://raw.githubusercontent.com/SchedulingLab/fjsp-instances/main/brandimarte/{name}.txt", f"{name}.txt")
        candidates.append(fjsp(raw, name) | dict(source=source, family="Brandimarte"))
    for prefix in ["sm", "med", "lar"]:
        for size_index in range(1, 5):
            for repetition in range(1, 6):
                name = f"{prefix}{size_index:02}_{repetition}"
                raw, source = fetch(f"https://raw.githubusercontent.com/SchedulingLab/fjsp-instances/main/behnke/{name}.txt", f"behnke_{name}.txt")
                candidates.append(fjsp(raw, name) | dict(source=source, family="Behnke-Geiger"))
    for kind, sizes, subdir in [("JSP", JSP_SIZES, "jobshop"), ("PFSP", PFSP_SIZES, "flowshop")]:
        for block, (n, m) in enumerate(sizes):
            raw, source = fetch(f"{TAILLARD_URL}/{subdir}.dir/tai{n}_{m}.txt", f"{subdir}_tai{n}_{m}.txt")
            rows = taillard_jsp(raw, block * 10) if kind == "JSP" else pfsp(raw, count=10)
            for index, row in enumerate(rows):
                if kind == "PFSP":
                    row["id"] = f"ta{block * 10 + index + 1:03}"
                assert len(row["jobs"]) == n and row["machines"] == m
                candidates.append(row | dict(source=source, family="Taillard"))
    for row in candidates:
        validate_instance(row)
        row["operations"] = sum(map(len, row["jobs"]))
        row["serial_upper"] = sum(max(p for _, p in op) for job in row["jobs"] for op in job)
        row["mean_flexibility"] = sum(len(op) for job in row["jobs"] for op in job) / row["operations"]
        row["size_block"] = f"{len(row['jobs'])}x{row['machines']}"
        row["cohort"] = "legacy" if row["id"] in LEGACY_IDS else "extension"
        row["assumptions"] = ["Zero job releases", "Nonpreemptive processing", "Unit-capacity machines", "No due dates"]
    assert len(candidates) == 280
    selected = select_instances(candidates)
    identifiers = {row["id"] for row in selected}
    assert len(identifiers) == len(selected)
    ledger = []
    for row in candidates:
        item = {k: v for k, v in row.items() if k != "jobs"}
        item.update(job_count=len(row["jobs"]), selected=row["id"] in identifiers,
                    selection_rank=selection_rank(row))
        ledger.append(item)
    DATA.mkdir(exist_ok=True)
    # Never overwrite the pilot's input interface.
    (DATA / "scaling-instances.json").write_text(json.dumps(selected, indent=2), encoding="utf-8")
    (DATA / "scaling-selection.json").write_text(json.dumps(ledger, indent=2), encoding="utf-8")
    return selected, ledger
