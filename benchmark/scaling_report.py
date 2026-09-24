"""Coverage-aware descriptive analysis and independent frozen-run validation."""
from __future__ import annotations

import argparse
import hashlib
import json
import statistics
from collections import Counter, defaultdict
from pathlib import Path

from run import METHODS, save
from scheduling import audit, hypervolume


def band(operations: int) -> str:
    return "small" if operations <= 150 else "medium" if operations <= 500 else "large"


def summarize(output: Path) -> dict:
    manifest = json.loads((output / "manifest.json").read_text())
    protocol = json.loads((output / "protocol.json").read_text())
    instances = {row["id"]: row for row in manifest["effective_sample"]}
    rows = [json.loads(path.read_text()) for path in sorted((output / "runs").glob("*.json"))]
    references = {}
    for name in instances:
        valid = [row for row in rows if row["instance"] == name and row.get("valid")]
        if valid:
            references[name] = dict(hv=max(row["hypervolume"] for row in valid),
                                    ms=min(row["best_ms"] for row in valid), ft=min(row["best_ft"] for row in valid))
    paired = defaultdict(set)
    for row in rows:
        if row.get("valid"):
            paired[row["instance"], row["seed"]].add(row["method"])
    complete_pairs = {key for key, methods in paired.items() if methods == set(METHODS)}
    # Primary summaries require every prescribed seed, preventing hidden unequal seed weights.
    complete_instances = {name for name in instances if all((name, seed) in complete_pairs for seed in protocol["seeds"])}
    metrics = []
    for row in rows:
        item = {k: v for k, v in row.items() if k not in ["archive", "trace", "phases", "options", "preference_results"]}
        item["operation_band"] = band(instances[row["instance"]]["operations"])
        item["paired_complete"] = row["instance"] in complete_instances
        if row.get("valid"):
            reference = references[row["instance"]]
            item.update(hv_deficit_percent=100 * (1 - row["hypervolume"] / reference["hv"]),
                        ms_gap_percent=100 * (row["best_ms"] / reference["ms"] - 1),
                        ft_gap_percent=100 * (row["best_ft"] / reference["ft"] - 1))
        metrics.append(item)
    groups = defaultdict(list)
    for item in instances.values():
        groups[item["kind"], band(item["operations"])].append(item["id"])
    aggregates = []
    for (kind, size), identifiers in sorted(groups.items()):
        for method in METHODS:
            observed = [row for row in metrics if row["instance"] in identifiers and row["method"] == method]
            by_instance = defaultdict(list)
            for row in observed:
                if row.get("valid") and row["paired_complete"]:
                    by_instance[row["instance"]].append(row)
            result = dict(kind=kind, operation_band=size, method=method,
                          planned=len(identifiers) * len(protocol["seeds"]), recorded=len(observed),
                          valid=sum(bool(row.get("valid")) for row in observed),
                          timeouts=sum(row["status"] == "timeout" for row in observed),
                          errors=sum(row["status"] == "error" for row in observed),
                          paired_instances=len(by_instance))
            for metric in ["hv_deficit_percent", "ms_gap_percent", "ft_gap_percent", "seconds"]:
                result[f"paired_mean_seed_median_{metric}"] = (statistics.mean(
                    statistics.median(row[metric] for row in values) for values in by_instance.values()
                ) if by_instance else None)
            result["conditional_median_seconds"] = statistics.median(row["seconds"] for row in observed if row.get("valid")) if any(row.get("valid") for row in observed) else None
            aggregates.append(result)
    per_instance = []
    for name, instance in instances.items():
        for method in METHODS:
            observed = [row for row in metrics if row["instance"] == name and row["method"] == method]
            valid = [row for row in observed if row.get("valid")]
            item = dict(instance=name, kind=instance["kind"], operation_band=band(instance["operations"]),
                        cohort=instance["cohort"], method=method, jobs=instance["job_count"],
                        machines=instance["machines"], operations=instance["operations"],
                        planned=len(protocol["seeds"]), statuses=dict(Counter(row["status"] for row in observed)),
                        paired_complete=name in complete_instances)
            for metric in ["hypervolume", "hv_deficit_percent", "ms_gap_percent", "ft_gap_percent", "seconds", "evaluations"]:
                item[f"conditional_median_{metric}"] = statistics.median(row[metric] for row in valid) if valid else None
            per_instance.append(item)
    report = dict(run_id=manifest["run_id"], specification_id=manifest["specification_id"],
                  planned_runs=manifest["planned_runs"], completed_runs=len(rows),
                  statuses=dict(Counter(row["status"] for row in rows)),
                  paired_complete_instances=sorted(complete_instances),
                  instance_reference=references, aggregates=aggregates, per_instance=per_instance, runs=metrics)
    save(output / "results.json", report)
    fmt = lambda value: "—" if value is None else f"{value:.3f}"
    lines = ["# Expanded APEX MS / job-flowtime benchmark", "",
             f"Run: `{manifest['run_id']}`. Recorded {len(rows)} of {manifest['planned_runs']} planned runs.", "",
             f"Statuses: {dict(Counter(row['status'] for row in rows))}. Fully paired instances: {len(complete_instances)} of {len(instances)}.", "",
             "This is a follow-up stress test informed by earlier development, not independent confirmation. Every original pilot case is retained and freshly evaluated. Selection was fixed before this run; see `protocol.json` and all 280 inclusion decisions in `selection.json`.", "",
             "Each search receives 1,000 evaluations with a common 240-second process watchdog. F uses one construction. Timeout rows have no inferred quality. Three seeds support descriptive comparisons only. Internal seconds exclude imports and output/audit; the watchdog includes them. This is not an equal-time comparison.", "",
             f"Execution uses {protocol.get('concurrent_runs', 1)} concurrent local runs, with one worker per algorithm. Resource contention and CPU heterogeneity affect runtime and timeout outcomes. These timings do not establish isolated performance or speedups. The run requires no LLM/API calls or agent monitoring.", "",
             "Primary quality summaries below use only instances completed validly by every method for every seed. First take the median across seeds within each instance, then average equally across instances. Missing coverage can make this subset unrepresentative; do not infer an overall winner. Small: <=150 operations; medium: 151–500; large: >500. Per-instance conditional summaries are exported in `results.json`.", "",
             "Gaps refer to best observed values across this run, not optima. MS and FT extrema may describe different schedules. Large-case results at this modest search budget do not establish convergence or industrial deployment performance.", "",
             "| Class | Operations | Method | Valid / planned | Timeouts | Errors | Paired instances | HV deficit % | MS gap % | FT gap % | Paired seconds |",
             "|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for row in aggregates:
        values = [fmt(row[f"paired_mean_seed_median_{metric}"]) for metric in ["hv_deficit_percent", "ms_gap_percent", "ft_gap_percent", "seconds"]]
        lines.append(f"| {row['kind']} | {row['operation_band']} | {row['method']} | {row['valid']}/{row['planned']} | {row['timeouts']} | {row['errors']} | {row['paired_instances']} | " + " | ".join(values) + " |")
    lines += ["", "## Dataset provenance", "",
              "JSP legacy: Fisher–Thompson, Lawrence and Adams–Balas–Zawack via [OR-Library](https://people.brunel.ac.uk/~mastjjb/jeb/orlib/jobshopinfo.html). JSP and PFSP size blocks: [Taillard (1993), author-hosted data](https://mistic.iict-heig-vd.ch/taillard/problemes.dir/ordonnancement.dir/ordonnancement.html). FJSP: Brandimarte (1993) and Behnke–Geiger (2012) via the [SchedulingLab distribution](https://github.com/SchedulingLab/fjsp-instances). Source bytes and SHA-256 hashes are retained; the mirror's bounds are not used as reference optima."]
    (output / "report.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return report


def validate(output: Path) -> dict:
    """Recompute archive feasibility/metrics and verify the entire prescribed matrix."""
    manifest = json.loads((output / "manifest.json").read_text())
    protocol = json.loads((output / "protocol.json").read_text())
    instances = {row["id"]: row for row in json.loads((output / "instances.json").read_text())}
    errors = []
    for relative, expected in manifest["snapshot_hashes"].items():
        path = output / relative
        if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            errors.append(f"Snapshot mismatch: {relative}")
    expected = {(name, method, seed) for name in instances for method in METHODS for seed in protocol["seeds"]}
    seen = set()
    audited = 0
    for path in sorted((output / "runs").glob("*.json")):
        try:
            row = json.loads(path.read_text())
            key = row["instance"], row["method"], row["seed"]
            assert key in expected and key not in seen, "Unexpected or duplicate run"
            seen.add(key)
            assert row["status"] in {"completed", "timeout", "error"}
            assert bool(row.get("valid")) == (row["status"] == "completed")
            if not row.get("valid"):
                continue
            instance = instances[row["instance"]]
            assert 0 < row["evaluations"] <= protocol["evaluation_budget"]
            assert row["evaluations"] == row["observed_attempts"] == len(row["trace"])
            assert row["failed_evaluations"] == sum("objectives" not in entry for entry in row["trace"])
            assert row["archive"]
            points = []
            for entry in row["archive"]:
                audit(instance, entry["schedule"], entry["objectives"])
                points.append(entry["objectives"])
                audited += 1
            scale = [instance["serial_upper"], instance["serial_upper"] * len(instance["jobs"])]
            assert abs(hypervolume(points, scale) - row["hypervolume"]) < 1e-12
            assert min(point[0] for point in points) == row["best_ms"]
            assert min(point[1] for point in points) == row["best_ft"]
            assert len(points) == row["archive_size"]
        except Exception as error:
            errors.append(f"{path.name}: {type(error).__name__}: {error}")
    missing = sorted(expected - seen)
    result = dict(passed=not errors and not missing, planned=len(expected), recorded=len(seen),
                  audited_archives=audited, missing=missing, errors=errors,
                  note="A complete valid experiment may contain correctly recorded solver timeouts/errors; these are outcomes, not missing rows.")
    save(output / "integrity-check.json", result)
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--validate", action="store_true")
    args = parser.parse_args()
    report = summarize(args.directory.resolve())
    print(json.dumps({key: report[key] for key in ["completed_runs", "planned_runs", "statuses"]}))
    if args.validate:
        result = validate(args.directory.resolve())
        print(json.dumps(result))
        raise SystemExit(0 if result["passed"] else 1)
