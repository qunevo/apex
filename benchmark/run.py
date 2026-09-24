"""Run standard pymoo methods and isolated APEX configurations; save auditable JSON."""
from __future__ import annotations

import os
for name in ["OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "NUMEXPR_NUM_THREADS"]:
    os.environ[name] = "1"

import argparse
import hashlib
import json
import platform
import random
import statistics
import subprocess
import sys
import time
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

import numpy as np
import pymoo
from pymoo.algorithms.moo.nsga2 import NSGA2
from pymoo.algorithms.moo.spea2 import SPEA2
from pymoo.algorithms.moo.moead import MOEAD
from pymoo.algorithms.moo.sms import SMSEMOA
from pymoo.core.problem import Problem
from pymoo.optimize import minimize

from instances import ROOT, prepare
from scheduling import ShopSampling, ShopCrossover, ShopMutation, audit, decode, hypervolume

METHODS = ["F", "T", "B", "G", "T-B", "T-G", "B-G", "T-B-G", "NSGA-II", "SPEA2", "MOEA-D", "SMS-EMOA"]


def save(path, value):
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(value, indent=2, allow_nan=False), encoding="utf-8")
    tmp.replace(path)


class ShopProblem(Problem):
    def __init__(self, instance):
        self.instance = instance
        length = len(instance["jobs"]) if instance["kind"] == "PFSP" else instance["operations"]
        if instance["kind"] == "FJSP":
            length *= 2
        super().__init__(n_var=length, n_obj=2, vtype=int)
        self.scale = np.array([instance["serial_upper"], instance["serial_upper"]*len(instance["jobs"])])
        self.archive, self.trace = [], []
        self.started = time.perf_counter()

    def _evaluate(self, X, out, *args, **kwargs):
        fitness = []
        for genome in X:
            objectives, schedule = decode(self.instance, genome)
            audit(self.instance, schedule, objectives)
            point = np.array(objectives)
            added = not any(all(a <= b for a, b in zip(row["objectives"], objectives)) for row in self.archive)
            if added:
                self.archive = [row for row in self.archive if not all(a <= b for a, b in zip(objectives, row["objectives"]))]
                self.archive.append(dict(objectives=objectives, schedule=schedule))
            self.trace.append(dict(evaluation=len(self.trace)+1, seconds=time.perf_counter()-self.started,
                                   objectives=objectives, archive_changed=added))
            fitness.append(point/self.scale)
        out["F"] = np.array(fitness)


def external(instance, method, evaluations, seed):
    problem = ShopProblem(instance)
    kwargs = dict(sampling=ShopSampling(), crossover=ShopCrossover(), mutation=ShopMutation())
    if method == "NSGA-II":
        algorithm = NSGA2(**kwargs)
    elif method == "SPEA2":
        algorithm = SPEA2(**kwargs)
    elif method == "SMS-EMOA":
        algorithm = SMSEMOA(**kwargs)
    else:
        weights = np.linspace(0, 1, 100)
        algorithm = MOEAD(ref_dirs=np.column_stack([weights, 1-weights]), **kwargs)
    cpu = time.process_time()
    result = minimize(problem, algorithm, ("n_eval", evaluations), seed=seed, verbose=False)
    elapsed = time.perf_counter()-problem.started
    a = result.algorithm
    return dict(seconds=elapsed, cpu_seconds=time.process_time()-cpu, evaluations=int(a.evaluator.n_eval),
                observed_attempts=len(problem.trace), failed_evaluations=0,
                archive=problem.archive, trace=problem.trace, phases=[],
                settings=dict(library=f"pymoo {pymoo.__version__}", population_size=int(a.pop_size),
                    crossover_probability=0.9, mutation_probability=1.0,
                    mutation="One sequence swap; additionally one eligible machine change for FJSP",
                    crossover="LOX" if instance["kind"] == "PFSP" else "POX + uniform machine crossover where applicable",
                    n_neighbors=getattr(a, "n_neighbors", None),
                    decomposition=type(getattr(a, "decomposition", None)).__name__,
                    offspring_count=getattr(a, "n_offsprings", None),
                    duplicate_elimination=type(a.eliminate_duplicates).__name__))


def apex_problem(instance):
    horizon = instance["serial_upper"] + 1
    tasks, dependencies, jobs = [], [], []
    for j, job in enumerate(instance["jobs"]):
        jobs.append(dict(id=f"J{j}", item="benchmark", quantity=1))
        for k, alternatives in enumerate(job):
            modes = [dict(id=f"mode{a}", primary=f"M{machine}", phases=[dict(
                id="process", work=duration, interruption="non_interruptible",
                rate_resource=f"M{machine}", requirements=[dict(resource=f"M{machine}", amount=1, retain=True)])])
                for a, (machine, duration) in enumerate(alternatives)]
            tasks.append(dict(id=f"J{j}_O{k}", job=f"J{j}", modes=modes,
                              attributes=dict(operation=k), source=instance["source"]["url"]))
            if k:
                dependencies.append(dict(before=f"J{j}_O{k-1}", after=f"J{j}_O{k}"))
    return dict(schema_version="apex.v3.4", id=instance["id"], horizon=horizon,
        resources=[dict(id=f"M{k}", capacity=1, calendar=[dict(start=0, end=horizon)]) for k in range(instance["machines"])],
        tasks=tasks, jobs=jobs, dependencies=dependencies,
        objectives=[dict(metric="makespan", weight=0.5, priority=0, scale=instance["serial_upper"]),
                    dict(metric="job_flow_time", weight=0.5, priority=0, scale=instance["serial_upper"]*len(jobs))])


def native(instance, method, evaluations, seed):
    binary = ROOT / "target" / "release" / "apex-benchmark.exe"
    request = dict(problem=apex_problem(instance), kind=instance["kind"], method=method,
                   evaluations=evaluations, seed=seed)
    start = time.perf_counter()
    process = subprocess.run([str(binary)], input=json.dumps(request), capture_output=True, text=True, timeout=240)
    wall = time.perf_counter()-start
    if process.returncode:
        raise RuntimeError(process.stdout + process.stderr)
    result = json.loads(process.stdout)
    result["cold_process_seconds"] = wall
    result["cpu_seconds"] = None
    result["cpu_seconds_note"] = "Not measured for native process; wall time is measured inside the process"
    return result


def annotate(instance, result, budget):
    scale = [instance["serial_upper"], instance["serial_upper"]*len(instance["jobs"])]
    start = time.perf_counter()
    for entry in result["archive"]:
        audit(instance, entry["schedule"], entry["objectives"])
    result["audit_seconds"] = time.perf_counter()-start
    points = [row["objectives"] for row in result["archive"]]
    assert points, "No valid results"
    assert result["evaluations"] <= budget, "Evaluation cap exceeded"
    result["valid"] = True
    result["hypervolume"] = hypervolume(points, scale)
    result["best_ms"] = min(row[0] for row in points)
    result["best_ft"] = min(row[1] for row in points)
    result["archive_size"] = len(points)
    result["preference_results"] = []
    for weight in np.linspace(0, 1, 11):
        best = min(points, key=lambda row: weight*row[0]/scale[0] + (1-weight)*row[1]/scale[1])
        result["preference_results"].append(dict(ms_weight=round(float(weight), 1), objectives=best,
            value=weight*best[0]/scale[0]+(1-weight)*best[1]/scale[1]))
    result["milliseconds_per_evaluation"] = result["seconds"]*1000/result["evaluations"]
    result["first_valid_seconds"] = next(row["seconds"] for row in result["trace"] if "objectives" in row)


def source_manifest():
    root = ROOT.parent
    paths = list((root / "src" / "rust").glob("*.rs"))
    paths += list((root / "customizations" / "dummy_customer").glob("*.rs"))
    paths += list(ROOT.glob("*.py")) + list((ROOT / "native").glob("*.rs"))
    paths += [ROOT / "Cargo.lock", ROOT / "requirements-lock.txt"]
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths if p.exists()}


def summary(output, instances, manifest):
    rows = [json.loads(p.read_text(encoding="utf-8")) for p in sorted((output / "runs").glob("*.json"))]
    valid = [row for row in rows if row.get("valid")]
    reference = {}
    for ins in instances:
        runs = [row for row in valid if row["instance"] == ins["id"]]
        if runs:
            reference[ins["id"]] = dict(hv=max(row["hypervolume"] for row in runs),
                ms=min(row["best_ms"] for row in runs), ft=min(row["best_ft"] for row in runs))
    groups = defaultdict(list)
    for row in valid:
        groups[row["kind"], row["method"]].append(row)
    aggregates = []
    for (kind, method), group in sorted(groups.items()):
        # Each selected instance has the same number of seeds in this pilot.
        aggregates.append(dict(kind=kind, method=method, runs=len(group),
            median_seconds=statistics.median(row["seconds"] for row in group),
            median_ms_per_evaluation=statistics.median(row["milliseconds_per_evaluation"] for row in group),
            mean_hv_deficit_percent=statistics.mean(100*(1-row["hypervolume"]/reference[row["instance"]]["hv"]) for row in group),
            mean_ms_gap_percent=statistics.mean(100*(row["best_ms"]/reference[row["instance"]]["ms"]-1) for row in group),
            mean_ft_gap_percent=statistics.mean(100*(row["best_ft"]/reference[row["instance"]]["ft"]-1) for row in group),
            mean_archive_size=statistics.mean(row["archive_size"] for row in group),
            failures=sum(row["failed_evaluations"] for row in group)))
    compact = [{k:v for k,v in row.items() if k not in ["trace", "archive", "phases", "options"]} for row in rows]
    report = dict(manifest=manifest, completed_runs=len(rows), valid_runs=len(valid),
                  instance_reference=reference, aggregates=aggregates, runs=compact)
    save(output / "results.json", report)
    lines = ["# Exploratory MS/FT benchmark", "", f"Completed: {len(rows)}; valid: {len(valid)}.", "",
        "Gaps refer to the best observed value within this pilot, not to proven optima.", "",
        "| Class | Method | Runs | HV deficit % | MS gap % | FT gap % | Median seconds |", "|---|---|---:|---:|---:|---:|---:|"]
    for r in aggregates:
        lines.append(f"| {r['kind']} | {r['method']} | {r['runs']} | {r['mean_hv_deficit_percent']:.3f} | {r['mean_ms_gap_percent']:.2f} | {r['mean_ft_gap_percent']:.2f} | {r['median_seconds']:.3f} |")
    lines += ["", "This is a small fixed-evaluation pilot. It does not establish statistical significance, optimality, equal-time superiority or performance of fully tuned methods.",
        "", "APEX uses unchanged native source modules with a benchmark-only PFSP policy. Python baselines use earliest-gap insertion for JSP/FJSP and the PFSP recurrence. Timings include each implementation's search and online correctness checks; these are implementation comparisons, including different decoder and validation costs.",
        "", "Internal populations use their default sizes (APEX 16, external MOEAs 100). An external observer records all nondominated evaluated points. Hybrid phases share one total allowance and pass one incumbent; they are not the production multi-policy Improve portfolio."]
    (output / "report.md").write_text("\n".join(lines)+"\n", encoding="utf-8")
    return report


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--evaluations", type=int, default=1000)
    parser.add_argument("--seeds", default="19,42,73")
    parser.add_argument("--instances", default="")
    parser.add_argument("--methods", default=",".join(METHODS))
    parser.add_argument("--out", type=Path, default=ROOT/"results"/"pilot")
    parser.add_argument("--resume", action="store_true")
    args = parser.parse_args()
    assert args.evaluations >= 100 and args.evaluations % 100 == 0
    instances = prepare()
    if args.instances:
        wanted = set(args.instances.split(","))
        instances = [ins for ins in instances if ins["id"] in wanted]
        assert {ins["id"] for ins in instances} == wanted
    seeds = list(map(int, args.seeds.split(",")))
    methods = args.methods.split(",")
    assert set(methods) <= set(METHODS)
    output = args.out.resolve()
    output.mkdir(parents=True, exist_ok=True)
    manifest = dict(created_at=datetime.now(timezone.utc).isoformat(), python=sys.version,
        platform=platform.platform(), processor=platform.processor(), logical_cpus=os.cpu_count(),
        numpy=np.__version__, pymoo=pymoo.__version__, methods=methods, seeds=seeds,
        evaluation_budget=args.evaluations, workers=1, objective_scale="MS/S, FT/(n*S)",
        hypervolume_reference=[1.1, 1.1], instances=[{k:v for k,v in i.items() if k != "jobs"} | {"job_count":len(i["jobs"])} for i in instances],
        source_hashes=source_manifest(), binary_sha256=hashlib.sha256((ROOT/"target/release/apex-benchmark.exe").read_bytes()).hexdigest(),
        scope="Exploratory fixed-evaluation pilot; post-hoc preferences; no tuning; no time-to-target claims",
        crossover_probability=0.9, mutation_probability=1.0,
        probability_provenance="Pinned pymoo generic Crossover and Mutation constructor defaults; equal across the four external methods; replaces continuous operator settings",
        runtime_boundary="Search setup, construction, search, observation and online validation; excludes package import, raw parsing, final JSON export and post-run audit",
        run_order_seed=20260924)
    old_manifest = output/"manifest.json"
    if args.resume and old_manifest.exists():
        old = json.loads(old_manifest.read_text())
        for key in ["methods", "seeds", "evaluation_budget", "source_hashes", "binary_sha256"]:
            assert old[key] == manifest[key], f"Cannot resume changed experiment: {key}"
        manifest = old
    save(old_manifest, manifest)
    jobs = [(ins, method, seed) for ins in instances for method in methods for seed in seeds]
    random.Random(20260924).shuffle(jobs)
    started = time.perf_counter()
    for number, (ins, method, seed) in enumerate(jobs, 1):
        file = output / "runs" / f"{ins['kind']}-{ins['id']}-{method}-{seed}.json"
        if args.resume and file.exists():
            continue
        result = dict(instance=ins["id"], kind=ins["kind"], method=method, seed=seed, nominal_evaluations=args.evaluations)
        try:
            value = external(ins, method, args.evaluations, seed) if method in METHODS[8:] else native(ins, method, args.evaluations, seed)
            result.update(value)
            annotate(ins, result, args.evaluations)
        except Exception as error:
            result.update(valid=False, error=f"{type(error).__name__}: {error}")
        save(file, result)
        print(json.dumps(dict(done=number, total=len(jobs), instance=ins["id"], method=method,
            valid=result["valid"], seconds=round(result.get("seconds", 0), 3),
            error=result.get("error"), elapsed=round(time.perf_counter()-started, 1))), flush=True)
    report = summary(output, instances, manifest)
    assert source_manifest() == manifest["source_hashes"], "Source changed during benchmark"
    print(json.dumps(dict(results=str(output/"results.json"), valid=report["valid_runs"], completed=report["completed_runs"])), flush=True)


if __name__ == "__main__":
    main()
