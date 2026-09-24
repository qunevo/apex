"""Acquire public benchmark instances with source hashes; never synthesize times."""
from __future__ import annotations

import hashlib
import json
import re
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent
DATA = ROOT / "data"


def fetch(url: str, name: str) -> tuple[str, dict]:
    path = DATA / "raw" / name
    path.parent.mkdir(parents=True, exist_ok=True)
    if not path.exists():
        with urllib.request.urlopen(url, timeout=60) as response:
            payload = response.read()
        path.write_bytes(payload)
    payload = path.read_bytes()
    return payload.decode("utf-8-sig"), {
        "url": url, "file": str(path.relative_to(ROOT)),
        "sha256": hashlib.sha256(payload).hexdigest(),
    }


def jsp(text: str, name: str) -> dict:
    lines = text.splitlines()
    idx = next(i for i, line in enumerate(lines) if line.strip() == f"instance {name}") + 1
    while not re.fullmatch(r"\s*\d+\s+\d+\s*", lines[idx]):
        idx += 1
    n, m = map(int, lines[idx].split())
    jobs = []
    for line in lines[idx + 1:idx + 1 + n]:
        row = list(map(int, line.split()))
        assert len(row) == 2*m
        jobs.append([[[row[k], row[k+1]]] for k in range(0, len(row), 2)])
    return dict(id=name, kind="JSP", machines=m, jobs=jobs)


def fjsp(text: str, name: str) -> dict:
    lines = [line for line in text.splitlines() if line.strip()]
    n, m = map(int, lines[0].split()[:2])
    assert len(lines) == n+1
    jobs = []
    for line in lines[1:]:
        row = list(map(int, line.split()))
        count, pos, job = row[0], 1, []
        for _ in range(count):
            alternatives = row[pos]
            pos += 1
            job.append([[row[pos+2*k], row[pos+2*k+1]] for k in range(alternatives)])
            pos += 2*alternatives
        assert pos == len(row)
        jobs.append(job)
    # SchedulingLab's version explicitly uses zero-based machine identifiers.
    return dict(id=name, kind="FJSP", machines=m, jobs=jobs)


def pfsp(text: str, count: int = 5) -> list[dict]:
    lines = text.splitlines()
    rows = []
    for i, line in enumerate(lines):
        if "number of jobs" not in line.lower():
            continue
        header = list(map(int, lines[i+1].split()))
        n, m, seed, upper, lower = header
        assert "processing times" in lines[i+2].lower()
        matrix = [list(map(int, lines[i+3+k].split())) for k in range(m)]
        assert all(len(row) == n for row in matrix)
        rows.append(dict(
            id=f"ta{len(rows)+1:03}", kind="PFSP", machines=m,
            jobs=[[[[k, matrix[k][j]]] for k in range(m)] for j in range(n)],
            source_seed=seed, source_makespan_upper=upper, source_makespan_lower=lower,
            bound_status="Historical source values, not asserted current optima",
        ))
    assert len(rows) >= count
    return rows[:count]


def validate_instance(instance: dict) -> None:
    assert instance["jobs"] and instance["machines"] > 0
    for job in instance["jobs"]:
        assert job
        for alternatives in job:
            assert alternatives and len({a[0] for a in alternatives}) == len(alternatives)
            assert all(0 <= machine < instance["machines"] and duration > 0
                       for machine, duration in alternatives)
    if instance["kind"] == "PFSP":
        assert all(len(job) == instance["machines"] for job in instance["jobs"])
        assert all(op[0][0] == k and len(op) == 1
                   for job in instance["jobs"] for k, op in enumerate(job))


def prepare() -> list[dict]:
    rows = []
    raw, origin = fetch("https://people.brunel.ac.uk/~mastjjb/jeb/orlib/files/jobshop1.txt", "jobshop1.txt")
    for name in ["ft06", "la01", "la06", "la16", "abz5"]:
        item = jsp(raw, name)
        item["source"] = origin
        rows.append(item)
    for i in range(1, 6):
        name = f"mk{i:02}"
        raw, origin = fetch(f"https://raw.githubusercontent.com/SchedulingLab/fjsp-instances/main/brandimarte/{name}.txt", f"{name}.txt")
        item = fjsp(raw, name)
        item["source"] = origin
        rows.append(item)
    raw, origin = fetch("https://mistic.iict-heig-vd.ch/taillard/problemes.dir/ordonnancement.dir/flowshop.dir/tai20_5.txt", "tai20_5.txt")
    for item in pfsp(raw):
        item["source"] = origin
        rows.append(item)
    for item in rows:
        validate_instance(item)
        item["operations"] = sum(map(len, item["jobs"]))
        item["serial_upper"] = sum(max(p for _, p in op) for job in item["jobs"] for op in job)
        item["mean_flexibility"] = sum(len(op) for job in item["jobs"] for op in job)/item["operations"]
        item["assumptions"] = ["All jobs released at zero", "No due dates", "Nonpreemptive processing", "Unit-capacity machines"]
    DATA.mkdir(exist_ok=True)
    (DATA / "instances.json").write_text(json.dumps(rows, indent=2), encoding="utf-8")
    return rows


if __name__ == "__main__":
    for value in prepare():
        print(value["kind"], value["id"], len(value["jobs"]), value["machines"], value["operations"])
