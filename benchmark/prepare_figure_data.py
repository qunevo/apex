"""Prepare descriptive paper-figure data from saved runs; never invoke a solver."""
from __future__ import annotations

import argparse
import csv
import hashlib
import itertools
import json
import math
import statistics
from collections import defaultdict
from pathlib import Path

from export_views import NAMES, METRICS, aggregate, load, save, sha

EVALUATIONS = [1, 10, 25, 50, 100, 250, 500, 750, 1000]
SECONDS = [0.001, 0.003, 0.01, 0.03, 0.1, 0.3, 1, 3, 10, 30, 60, 120, 240]


def quantile(values, fraction):
    """Linear sample quantile (Hyndman-Fan type 7), including n=1."""
    ordered = sorted(values)
    if not ordered:
        return None
    location = (len(ordered) - 1) * fraction
    lower, upper = math.floor(location), math.ceil(location)
    return ordered[lower] + (ordered[upper] - ordered[lower]) * (location - lower)


def box_stats(values):
    """Tukey 1.5-IQR whiskers plus sample variance; missing is not zero."""
    values = [v for v in values if v is not None]
    keys = ["mean", "sample_variance", "sample_sd", "min", "q1", "median", "q3", "max", "iqr", "lower_fence", "upper_fence", "whisker_low", "whisker_high"]
    if not values:
        return dict(n=0, **dict.fromkeys(keys), outlier_count=0, nonzero_range=False)
    q1, med, q3 = [quantile(values, p) for p in [0.25, 0.5, 0.75]]
    iqr = q3 - q1
    low, high = q1 - 1.5 * iqr, q3 + 1.5 * iqr
    inside = [v for v in values if low <= v <= high]
    return dict(n=len(values), mean=statistics.mean(values), sample_variance=statistics.variance(values) if len(values) > 1 else None,
                sample_sd=statistics.stdev(values) if len(values) > 1 else None,
                min=min(values), q1=q1, median=med, q3=q3, max=max(values), iqr=iqr,
                lower_fence=low, upper_fence=high, whisker_low=min(inside), whisker_high=max(inside),
                outlier_count=len(values) - len(inside), nonzero_range=max(values) != min(values))


def write_rows(path, rows):
    assert rows, path
    with path.open("w", encoding="utf-8-sig", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def advance_front(front, point):
    if any(all(a <= b for a, b in zip(other, point)) for other in front):
        return front
    return [other for other in front if not all(a <= b for a, b in zip(point, other))] + [point]


def hv(front, scale):
    height, area = 1.1, 0.0
    for x, y in sorted((p[0] / scale[0], p[1] / scale[1]) for p in front):
        if x < 1.1 and y < height:
            area += (1.1 - x) * (height - y)
            height = y
    return area


def trace_states(trace, scale, reference):
    front, states = [], []
    for entry in trace:
        if "objectives" in entry:
            front = advance_front(front, entry["objectives"])
        state = dict(evaluations=entry["evaluation"], seconds=entry["seconds"],
                     hv_deficit_pct=None, makespan_gap_pct=None, flowtime_gap_pct=None)
        if front:
            state.update(hv_deficit_pct=100 * (1 - hv(front, scale) / reference["hv"]),
                         makespan_gap_pct=100 * (min(p[0] for p in front) / reference["ms"] - 1),
                         flowtime_gap_pct=100 * (min(p[1] for p in front) / reference["ft"] - 1))
        if states:
            assert state["evaluations"] > states[-1]["evaluations"]
            assert state["seconds"] >= states[-1]["seconds"]
        states.append(state)
    return states


def at_checkpoint(states, axis, checkpoint, final_seconds):
    observed = [s for s in states if s[axis] <= checkpoint]
    if not observed:
        return None, "no_observation_yet"
    last = observed[-1]
    if last["hv_deficit_pct"] is None:
        return last, "no_feasible_candidate_yet"
    finished = checkpoint >= (final_seconds if axis == "seconds" else states[-1]["evaluations"])
    return last, "final_value_carried" if finished else "observed"


def prepare(source, destination):
    assert not destination.exists(), "Use a new figure-data version"
    views_path = source / "views-v1/views.json"
    source_hashes = {name: sha(source / name) for name in ["manifest.json", "results.json", "instances.json", "views-v1/views.json"]}
    views = load(views_path)
    instances = {r["id"]: r for r in load(source / "instances.json")}
    report = load(source / "results.json")
    per_instance = views["per_instance"]
    paired = [r for r in per_instance if r["paired_complete"]]
    paired_ids = {r["instance"] for r in paired}
    methods = list(NAMES.values())
    destination.mkdir(parents=True)
    products = {}

    def emit(name, rows):
        write_rows(destination / f"{name}.csv", rows)
        products[name] = dict(rows=len(rows), sha256=sha(destination / f"{name}.csv"))

    # The sampling unit for cross-instance distributions is one seed median.
    points = []
    for row in paired:
        for metric in METRICS:
            points.append({k: row[k] for k in ["instance", "problem_class", "method", "family", "cohort", "operations", "operation_band"]} |
                          dict(metric=metric, value=row[f"seed_median_{metric}"], valid_seeds=row["valid_runs"]))
    emit("boxplot_points", points)
    stats, outliers = [], []
    for group_type, dimensions in [("overall", []), ("class", ["problem_class"]), ("class_size", ["problem_class", "operation_band"])]:
        groups = defaultdict(list)
        for row in per_instance:
            groups[tuple(row[d] for d in dimensions) + (row["method"],)].append(row)
        for key, group in groups.items():
            for metric in METRICS:
                good = [r for r in group if r["paired_complete"]]
                descriptor = dict(group_type=group_type, problem_class=group[0]["problem_class"] if dimensions else "ALL",
                                  operation_band=group[0]["operation_band"] if len(dimensions) == 2 else "ALL", method=key[-1], metric=metric,
                                  planned_instances=len(group))
                summary = box_stats([r[f"seed_median_{metric}"] for r in good])
                stats.append(descriptor | summary)
                for row in good:
                    value = row[f"seed_median_{metric}"]
                    if value < summary["lower_fence"] or value > summary["upper_fence"]:
                        outliers.append(descriptor | dict(instance=row["instance"], value=value))
    emit("boxplot_stats", stats)
    emit("boxplot_outliers", outliers)

    seed_groups = defaultdict(list)
    for row in views["raw_runs"]:
        seed_groups[row["instance"], row["method"]].append(row)
    seed_stats = []
    for row in per_instance:
        for metric in METRICS:
            summary = box_stats([r[metric] for r in seed_groups[row["instance"], row["method"]] if r["valid"]])
            seed_stats.append({k: row[k] for k in ["instance", "problem_class", "method", "paired_complete", "planned_runs", "timeouts"]} |
                              dict(metric=metric, deterministic_quality_reference=row["method"] == "XG" and metric != "seconds") | summary)
    emit("seed_statistics", seed_stats)
    spread_summary = []
    for kind in ["JSP", "FJSP", "PFSP"]:
        for method in methods:
            for metric in METRICS:
                rows = [r for r in seed_stats if r["problem_class"] == kind and r["method"] == method and r["metric"] == metric and r["n"] == 3]
                spread_summary.append(dict(problem_class=kind, method=method, metric=metric, instances_with_three_seeds=len(rows),
                                           instances_with_nonzero_range=sum(r["nonzero_range"] for r in rows),
                                           median_seed_range=statistics.median(r["max"] - r["min"] for r in rows) if rows else None))
    emit("seed_variation_summary", spread_summary)

    # ECDFs and all method-pair contrasts use the identical paired population.
    ecdf, differences, contrast_stats = [], [], []
    by_id = {(r["instance"], r["method"]): r for r in paired}
    for kind in ["ALL", "JSP", "FJSP", "PFSP"]:
        identifiers = sorted({r["instance"] for r in paired if kind == "ALL" or r["problem_class"] == kind})
        for metric in METRICS:
            for method in methods:
                values = sorted(by_id[name, method][f"seed_median_{metric}"] for name in identifiers)
                for value in sorted(set(values)):
                    ecdf.append(dict(problem_class=kind, method=method, metric=metric, threshold=value, count_leq=sum(v <= value for v in values), n=len(values), fraction_leq=sum(v <= value for v in values) / len(values)))
            for a, b in itertools.combinations(methods, 2):
                delta = []
                for name in identifiers:
                    av, bv = [by_id[name, method][f"seed_median_{metric}"] for method in [a, b]]
                    difference = av - bv
                    delta.append(difference)
                    if kind != "ALL":
                        differences.append(dict(problem_class=kind, instance=name, metric=metric, method_a=a, method_b=b,
                                                value_a=av, value_b=bv, difference_a_minus_b=difference))
                contrast_stats.append(dict(problem_class=kind, metric=metric, method_a=a, method_b=b, n=len(delta),
                                           a_better=sum(v < -1e-9 for v in delta), ties=sum(abs(v) <= 1e-9 for v in delta), b_better=sum(v > 1e-9 for v in delta),
                                           mean_difference=statistics.mean(delta), median_difference=statistics.median(delta), q1_difference=quantile(delta, .25), q3_difference=quantile(delta, .75)))
    emit("ecdf", ecdf)
    emit("paired_differences", differences)
    emit("paired_comparison_summary", contrast_stats)
    emit("scaling_points", per_instance)
    emit("coverage_heatmap", views["by_size"])
    emit("quality_time", [dict(problem_class="ALL", **r) for r in views["overall"]] + views["by_problem_class"])

    convergence, archives, raw_hashes = [], [], {}
    terminal_checks = 0
    for index, row in enumerate(views["raw_runs"]):
        path = source / row["source_json"].removeprefix("../")
        payload = path.read_bytes()
        raw_hashes[path.relative_to(source).as_posix()] = hashlib.sha256(payload).hexdigest()
        run = json.loads(payload)
        instance = instances[row["instance"]]
        reference = report["instance_reference"][row["instance"]]
        scale = [instance["serial_upper"], instance["serial_upper"] * len(instance["jobs"])]
        states = trace_states(run.get("trace", []), scale, reference)
        if row["valid"]:
            assert states and states[-1]["evaluations"] == row["evaluations"]
            for metric in METRICS[:3]:
                assert math.isclose(states[-1][metric], row[metric], rel_tol=1e-10, abs_tol=1e-10), (path, metric)
            terminal_checks += 1
            for i, entry in enumerate(run["archive"]):
                ms, ft = entry["objectives"]
                archives.append(dict(instance=row["instance"], problem_class=row["problem_class"], method=row["method"], seed=row["seed"], archive_index=i,
                                     makespan=ms, job_flowtime=ft, normalized_ms=ms / scale[0], normalized_ft=ft / scale[1], source_json=row["source_json"]))
        for axis, checkpoints in [("evaluations", EVALUATIONS), ("seconds", SECONDS)]:
            for checkpoint in checkpoints:
                state, status = at_checkpoint(states, axis, checkpoint, row["seconds"]) if states else (None, "no_saved_trace")
                convergence.append(dict(instance=row["instance"], problem_class=row["problem_class"], method=row["method"], seed=row["seed"],
                                        paired_complete=row["instance"] in paired_ids, run_status=row["status"], axis=axis, checkpoint=checkpoint,
                                        checkpoint_status=status, observed_evaluations=state["evaluations"] if state else 0,
                                        actual_total_evaluations=row["evaluations"], **{metric: state[metric] if state else None for metric in METRICS[:3]}))
        if (index + 1) % 600 == 0:
            print(f"Read {index + 1} saved runs; no solver execution", flush=True)
    emit("archive_points", archives)
    emit("convergence_runs", convergence)
    save(destination / "raw_source_hashes.json", raw_hashes)

    cells = defaultdict(list)
    for row in convergence:
        if row["paired_complete"]:
            cells[row["axis"], row["checkpoint"], row["instance"], row["method"]].append(row)
    curves = []
    for axis, checkpoints in [("evaluations", EVALUATIONS), ("seconds", SECONDS)]:
        for checkpoint in checkpoints:
            supported = {name for name in paired_ids if all(len(cells[axis, checkpoint, name, method]) == 3 and all(r["hv_deficit_pct"] is not None for r in cells[axis, checkpoint, name, method]) for method in methods)}
            for kind in ["ALL", "JSP", "FJSP", "PFSP"]:
                denominator = {name for name in paired_ids if kind == "ALL" or instances[name]["kind"] == kind}
                active = supported & denominator
                for method in methods:
                    values = {metric: [statistics.median(r[metric] for r in cells[axis, checkpoint, name, method]) for name in sorted(active)] for metric in METRICS[:3]}
                    curves.append(dict(problem_class=kind, method=method, axis=axis, checkpoint=checkpoint, terminal_paired_instances=len(denominator),
                                       common_available_instances=len(active), common_available_pct=100 * len(active) / len(denominator),
                                       **{f"mean_{metric}": statistics.mean(items) if items else None for metric, items in values.items()}))
    emit("convergence_summary", curves)
    assert source_hashes == {name: sha(source / name) for name in source_hashes}
    manifest = dict(export_id=destination.name, source_run=source.name, evidence_role="Post-result descriptive figure preparation",
                    source_hashes=source_hashes, code_sha256=sha(Path(__file__)), method_names=NAMES, artifacts=products,
                    quantiles="Linear interpolation, Hyndman-Fan type 7; whiskers end at observed values within 1.5 IQR; outliers retained.",
                    variance="Sample variance uses n-1; null for n<2. Cross-instance variation uses one seed median per paired instance. Seed variation is separate.",
                    inference="No significance tests, confidence intervals, KDE/violin density estimates, or new solver runs. Three seeds are descriptive repetitions.",
                    convergence="Observed nondominated prefix of saved traces. Missing early feasibility stays null. Finished methods carry final values forward, including XG after one evaluation. Timeouts have no saved trace and no imputed quality. Summaries use the common available instance set across all methods and seeds at each checkpoint; its size is explicit and may change.",
                    paired_differences="A minus B; negative favours A for all exported metrics. Numerical tie tolerance 1e-9. All unordered method pairs are retained.",
                    archive_scope="Plot objective-space fronts separately per instance; scales differ between instances. Post-hoc example selection must be disclosed.",
                    terminal_trace_checks=terminal_checks, archive_points=len(archives), raw_runs=len(raw_hashes), source_unchanged=True, experiments_rerun=0)
    save(destination / "manifest.json", manifest)
    print(json.dumps({"passed": True, "trace_endpoints_checked": terminal_checks, "archive_points": len(archives), "tables": len(products)}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    prepare(args.source.resolve(), args.destination.resolve())
