"""Export reproducible views of a completed experiment without running any solver."""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import statistics
from collections import Counter, defaultdict
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent
NAMES = json.loads((ROOT / "method_names.json").read_text())
METRICS = ["hv_deficit_pct", "makespan_gap_pct", "flowtime_gap_pct", "seconds"]


def load(path):
    return json.loads(Path(path).read_text(encoding="utf-8"))


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2, allow_nan=False) + "\n", encoding="utf-8")


def operation_band(count):
    return "1-150" if count <= 150 else "151-500" if count <= 500 else "501+"


def median(values):
    values = [v for v in values if v is not None]
    return statistics.median(values) if values else None


def raw_view(report, manifest, source, output):
    instances = {row["id"]: row for row in manifest["effective_sample"]}
    records = []
    expected = {(name, method, seed) for name in instances for method in manifest["methods"] for seed in manifest["random_seeds"]}
    seen = set()
    for row in report["runs"]:
        key = row["instance"], row["method"], row["seed"]
        assert key in expected and key not in seen, f"Unexpected or duplicate run: {key}"
        seen.add(key)
        ins = instances[row["instance"]]
        raw_file = source / "runs" / f"{row['kind']}-{row['instance']}-{row['method']}-{row['seed']}.json"
        assert raw_file.is_file(), raw_file
        record = dict(instance=row["instance"], problem_class=row["kind"], method=NAMES[row["method"]],
                      seed=row["seed"], status=row["status"], valid=row["valid"],
                      best_makespan=row.get("best_ms"), best_job_flowtime=row.get("best_ft"),
                      hypervolume=row.get("hypervolume"), seconds=row.get("seconds"),
                      process_seconds=row.get("process_seconds"), evaluations=row.get("evaluations"),
                      nominal_evaluations=row["nominal_evaluations"],
                      jobs=ins["job_count"], machines=ins["machines"], operations=ins["operations"],
                      operation_band=operation_band(ins["operations"]), family=ins["family"],
                      cohort=ins["cohort"], mean_flexibility=ins["mean_flexibility"],
                      archive_size=row.get("archive_size"), first_valid_seconds=row.get("first_valid_seconds"),
                      failed_evaluations=row.get("failed_evaluations"), method_internal=row["method"],
                      source_json=os.path.relpath(raw_file, output).replace("\\", "/"))
        if row["valid"]:
            ref = report["instance_reference"][row["instance"]]
            record.update(hv_deficit_pct=100 * (1 - row["hypervolume"] / ref["hv"]),
                          makespan_gap_pct=100 * (row["best_ms"] / ref["ms"] - 1),
                          flowtime_gap_pct=100 * (row["best_ft"] / ref["ft"] - 1))
        else:
            record.update(hv_deficit_pct=None, makespan_gap_pct=None, flowtime_gap_pct=None)
        records.append(record)
    assert seen == expected, "Missing experiment rows"
    method_order = {name: index for index, name in enumerate(NAMES.values())}
    records.sort(key=lambda row: (row["problem_class"], row["instance"], method_order[row["method"]], row["seed"]))
    return records


def instance_view(raw, seeds, methods):
    groups = defaultdict(list)
    for row in raw:
        groups[row["instance"], row["method"]].append(row)
    valid_keys = {key for key, rows in groups.items() if {row["seed"] for row in rows if row["valid"]} == set(seeds)}
    complete = {name for name, _ in groups if all((name, method) in valid_keys for method in methods)}
    result = []
    for (name, method), rows in groups.items():
        head = rows[0]
        item = {key: head[key] for key in ["instance", "problem_class", "method", "family", "cohort", "jobs", "machines", "operations", "operation_band", "mean_flexibility"]}
        item.update(paired_complete=name in complete, planned_runs=len(seeds),
                    valid_runs=sum(row["valid"] for row in rows),
                    timeouts=sum(row["status"] == "timeout" for row in rows),
                    errors=sum(row["status"] == "error" for row in rows))
        for metric in METRICS + ["best_makespan", "best_job_flowtime", "hypervolume", "evaluations"]:
            values = [row[metric] for row in rows if row["valid"]]
            item[f"seed_median_{metric}"] = median(values)
        # Descriptive spread over observed seeds, not a confidence interval.
        valid = [row for row in rows if row["valid"]]
        for metric in ["hv_deficit_pct", "seconds"]:
            values = [row[metric] for row in valid]
            item[f"seed_min_{metric}"] = min(values) if values else None
            item[f"seed_max_{metric}"] = max(values) if values else None
        result.append(item)
    return result


def aggregate(per_instance, dimensions):
    groups = defaultdict(list)
    for row in per_instance:
        groups[tuple(row[key] for key in dimensions) + (row["method"],)].append(row)
    result = []
    order = {name: i for i, name in enumerate(NAMES.values())}
    for key, rows in sorted(groups.items(), key=lambda value: value[0][:-1] + (order[value[0][-1]],)):
        item = dict(zip(dimensions + ["method"], key))
        paired = [row for row in rows if row["paired_complete"]]
        item.update(planned_instances=len(rows), paired_instances=len(paired),
                    planned_runs=sum(row["planned_runs"] for row in rows),
                    valid_runs=sum(row["valid_runs"] for row in rows),
                    timeouts=sum(row["timeouts"] for row in rows), errors=sum(row["errors"] for row in rows))
        item["success_pct"] = 100 * item["valid_runs"] / item["planned_runs"]
        for metric in METRICS:
            values = [row[f"seed_median_{metric}"] for row in paired]
            item[f"mean_{metric}"] = statistics.mean(values) if values else None
        result.append(item)
    return result


def class_balanced(by_class):
    result = []
    for method in NAMES.values():
        rows = [row for row in by_class if row["method"] == method]
        assert {row["problem_class"] for row in rows} == {"JSP", "FJSP", "PFSP"}
        item = dict(method=method, weighting="Equal weight for each of JSP, FJSP and PFSP",
                    classes_with_paired_data=sum(row["paired_instances"] > 0 for row in rows))
        for metric in METRICS:
            values = [row[f"mean_{metric}"] for row in rows]
            item[f"mean_{metric}"] = statistics.mean(values) if all(v is not None for v in values) else None
        item["mean_class_success_pct"] = statistics.mean(row["success_pct"] for row in rows)
        result.append(item)
    return result


def coverage(raw):
    result = []
    for method in NAMES.values():
        item = dict(method=method)
        for kind in ["JSP", "FJSP", "PFSP", "all"]:
            rows = [row for row in raw if row["method"] == method and (kind == "all" or row["problem_class"] == kind)]
            item.update({f"{kind}_planned": len(rows), f"{kind}_valid": sum(row["valid"] for row in rows),
                         f"{kind}_timeouts": sum(row["status"] == "timeout" for row in rows),
                         f"{kind}_success_pct": 100 * sum(row["valid"] for row in rows) / len(rows)})
        result.append(item)
    return result


def write_csv(path, rows):
    with path.open("w", encoding="utf-8-sig", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def markdown_table(rows, group=None):
    lines = ["| Method | Paired instances | HV deficit % | MS gap % | FT gap % | Mean seconds | Timeouts / planned |",
             "|---|---:|---:|---:|---:|---:|---:|"]
    for row in rows:
        values = [row[f"mean_{key}"] for key in METRICS]
        formatted = ["n.a." if value is None else f"{value:.3f}" for value in values]
        lines.append(f"| {row['method']} | {row.get('paired_instances', '—')} | " + " | ".join(formatted) +
                     f" | {row.get('timeouts', '—')} / {row.get('planned_runs', '—')} |")
    return "\n".join(lines)


def export(source, output):
    assert not output.exists(), f"Use a new view directory: {output}"
    report, manifest, integrity = [load(source / name) for name in ["results.json", "manifest.json", "integrity-check.json"]]
    assert integrity["passed"] and report["completed_runs"] == report["planned_runs"]
    assert list(NAMES) == manifest["methods"]
    input_paths = [source / name for name in ["results.json", "manifest.json", "integrity-check.json", "protocol.json"]]
    input_hashes = {path.name: sha(path) for path in input_paths}
    raw = raw_view(report, manifest, source, output)
    per_instance = instance_view(raw, manifest["random_seeds"], list(NAMES.values()))
    paired = {row["instance"] for row in per_instance if row["paired_complete"]}
    assert paired == set(report["paired_complete_instances"])
    by_class = aggregate(per_instance, ["problem_class"])
    views = dict(overall=aggregate(per_instance, []), by_problem_class=by_class,
                 by_size=aggregate(per_instance, ["problem_class", "operation_band"]),
                 by_family=aggregate(per_instance, ["problem_class", "family"]),
                 by_cohort=aggregate(per_instance, ["problem_class", "cohort"]),
                 overall_class_balanced=class_balanced(by_class), coverage=coverage(raw),
                 per_instance=per_instance, raw_runs=raw)
    # Check all inherited class/size aggregates against the frozen report independently.
    for frozen in report["aggregates"]:
        size = {"small": "1-150", "medium": "151-500", "large": "501+"}[frozen["operation_band"]]
        row = next(row for row in views["by_size"] if row["problem_class"] == frozen["kind"] and row["operation_band"] == size and row["method"] == NAMES[frozen["method"]])
        assert (row["paired_instances"], row["valid_runs"], row["timeouts"]) == (frozen["paired_instances"], frozen["valid"], frozen["timeouts"])
        for metric, old in zip(METRICS, ["hv_deficit_percent", "ms_gap_percent", "ft_gap_percent", "seconds"]):
            expected = frozen[f"paired_mean_seed_median_{old}"]
            assert row[f"mean_{metric}"] is None if expected is None else math.isclose(row[f"mean_{metric}"], expected, rel_tol=1e-12, abs_tol=1e-12)
    output.mkdir(parents=True)
    for name, rows in views.items():
        write_csv(output / f"{name}.csv", rows)
    save(output / "raw_runs.json", raw)
    save(output / "views.json", views)
    save(output / "method_names.json", NAMES)
    meta = dict(export_id=output.name, created_at=datetime.now(timezone.utc).isoformat(),
                source_run=manifest["run_id"], source_directory=os.path.relpath(source, output).replace("\\", "/"),
                source_hashes=input_hashes, method_mapping=NAMES,
                analysis_code_sha256=sha(Path(__file__)), naming_file_sha256=sha(ROOT / "method_names.json"),
                planned_instances=len(manifest["effective_sample"]), paired_instances=len(paired),
                paired_class_counts=dict(Counter(row["problem_class"] for row in per_instance if row["paired_complete"] and row["method"] == "XG")),
                row_counts={name: len(rows) for name, rows in views.items()},
                status_counts=dict(Counter(row["status"] for row in raw)),
                aggregation="Median of successful seed outcomes within each fully paired instance, then arithmetic mean with equal instance weights. Fully paired means every method and every seed succeeded. Class-balanced view averages the three class means equally.",
                missing_values="Null/blank means unavailable. Timeout quality is never imputed as zero. Per-instance medians use available seeds and disclose the valid count; aggregate quality uses fully paired instances only.",
                metric_units="Gap/deficit values are percentages: 3.5 means 3.5%, not 350%. Time is measured internal wall seconds. Processing-time MS/FT use the source instances' units, not asserted seconds.",
                raw_scope="Raw metrics contain one row per run, including failures. Full original JSON with traces, archives and phase evidence remains in source_json, unchanged. method_internal preserves original identifiers.",
                evidence_role="Post-result descriptive export; class balancing and family views are supplementary perspectives, not independent confirmation.",
                runtime_scope="Eight concurrent single-worker experiments; XG uses one construction, searches up to 1000 evaluations. Runtime and timeout outcomes include shared-host effects.",
                data_modified=False, experiments_rerun=0)
    save(output / "export_manifest.json", meta)
    lines = ["# APEX benchmark result views", "", f"Source run: `{manifest['run_id']}`.", "",
             "All original run files remain unchanged. XG = FastPlanner; XH = Trainer; XT = tree search; XE = direct genetic algorithm. Sequential combinations are XHT, XHE, XTE and XHTE.", "",
             "## Files", "",
             "[Excel workbook](outputs/scaling-20260924/benchmark_views.xlsx) contains the same tables as the CSV/JSON exports. It is a static research snapshot; regenerate it to refresh calculations.", "",
             "| View | Purpose | Rows |", "|---|---|---:|"]
    purposes = dict(overall="Equal-instance aggregate, matching the chat table", by_problem_class="JSP / FJSP / PFSP",
                    by_size="Problem class and fixed operation-count band", by_family="Dataset family within each problem class",
                    by_cohort="Legacy pilot cases versus newly added cases", overall_class_balanced="Equal weight for each problem class",
                    coverage="Successes and timeouts over all 88 instances", per_instance="Seed medians and min/max spread with sample counts",
                    raw_runs="All individual run metrics; source_json links to full raw evidence")
    for name, rows in views.items():
        lines.append(f"| [{name}.csv]({name}.csv) | {purposes[name]} | {len(rows)} |")
    lines += ["", "[All tables in JSON](views.json) · [Raw run metrics in JSON](raw_runs.json) · [Provenance and definitions](export_manifest.json)", "",
              "## Reading the tables", "", meta["aggregation"], "", meta["missing_values"], "", meta["metric_units"], "",
              "Gaps refer to the best observed value in this experiment, not a proven optimum. Minimum MS and minimum FT can belong to different schedules. Original failure outcomes remain in coverage columns even when excluded from paired quality summaries. No overall superiority claim follows from the successful subset alone.", "",
              meta["runtime_scope"], "", meta["evidence_role"], "", "## Overall, equal instance weights", "", markdown_table(views["overall"])]
    for kind in ["JSP", "FJSP", "PFSP"]:
        lines += ["", f"## {kind}", "", markdown_table([row for row in by_class if row["problem_class"] == kind])]
    lines += ["", "## Overall, equal problem-class weights", "", markdown_table(views["overall_class_balanced"])]
    (output / "README.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    assert input_hashes == {path.name: sha(path) for path in input_paths}, "Source changed while exporting"
    verification = dict(passed=True, matrix_complete=True, frozen_size_aggregates_reproduced=True,
                        source_hashes_unchanged=True, method_names_unique=len(set(NAMES.values())) == len(NAMES),
                        raw_rows=len(raw), paired_instances=len(paired), all_timeouts_retained=sum(row["status"] == "timeout" for row in raw))
    save(output / "export_validation.json", verification)
    print(json.dumps(verification))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    export(args.source.resolve(), args.output.resolve())
