from export_views import NAMES, aggregate, class_balanced, instance_view


def row(name, method, seed, value, valid=True, kind="JSP"):
    return dict(instance=name, method=method, seed=seed, valid=valid,
                status="completed" if valid else "timeout", problem_class=kind,
                family="synthetic", cohort="test", jobs=2, machines=2, operations=4,
                operation_band="1-150", mean_flexibility=1,
                hv_deficit_pct=value if valid else None, makespan_gap_pct=value if valid else None,
                flowtime_gap_pct=value if valid else None, seconds=value if valid else None,
                best_makespan=value if valid else None, best_job_flowtime=value if valid else None,
                hypervolume=value if valid else None, evaluations=100 if valid else None)


def test_names_and_compact_handoffs():
    assert [NAMES[key] for key in ["F", "T", "B", "G", "T-B-G"]] == ["XG", "XH", "XT", "XE", "XHTE"]
    assert len(NAMES) == len(set(NAMES.values())) == 12


def test_seed_median_and_paired_exclusion_preserve_timeout_coverage():
    rows = [row("a", method, seed, value) for method in ["XH", "XE"] for seed, value in [(1, 1), (2, 2), (3, 90)]]
    rows += [row("b", method, seed, 4, not (method == "XE" and seed == 3)) for method in ["XH", "XE"] for seed in [1, 2, 3]]
    per_instance = instance_view(rows, [1, 2, 3], ["XH", "XE"])
    totals = aggregate(per_instance, [])
    assert all(r["paired_instances"] == 1 and r["mean_seconds"] == 2 for r in totals)
    xe = next(r for r in totals if r["method"] == "XE")
    assert xe["timeouts"] == 1 and xe["valid_runs"] == 5 and xe["planned_runs"] == 6
    assert next(r for r in per_instance if r["instance"] == "b" and r["method"] == "XH")["paired_complete"] is False


def test_empty_paired_group_does_not_invent_zero_quality():
    rows = [row("a", "XH", 1, 0, False), row("a", "XE", 1, 2)]
    totals = aggregate(instance_view(rows, [1], ["XH", "XE"]), [])
    assert all(r["mean_hv_deficit_pct"] is None for r in totals)


def test_class_balancing_does_not_weight_larger_collections_more():
    rows = []
    for method in NAMES.values():
        for kind, count, value in [("JSP", 1, 3), ("FJSP", 5, 6), ("PFSP", 2, 9)]:
            rows += [row(f"{kind}{i}", method, 1, value, kind=kind) for i in range(count)]
    per_instance = instance_view(rows, [1], list(NAMES.values()))
    balanced = class_balanced(aggregate(per_instance, ["problem_class"]))
    assert all(r["mean_hv_deficit_pct"] == 6 for r in balanced)
    assert aggregate(per_instance, [])[0]["mean_hv_deficit_pct"] == 51 / 8
