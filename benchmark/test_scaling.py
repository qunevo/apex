import copy
import json
from collections import Counter

import pytest

from run import METHODS, save
from scaling import execute, run_one
from scaling_data import JSP_SIZES, PFSP_SIZES, LEGACY_IDS, select_instances, taillard_jsp
from scaling_report import summarize, validate
from test_benchmark import synthetic


def candidate_frame():
    rows = []
    for name in LEGACY_IDS[:5]:
        rows.append(dict(id=name, kind="JSP", family="OR-Library legacy", jobs=[[]] * 6, machines=6))
    for index in range(1, 16):
        rows.append(dict(id=f"mk{index:02}", kind="FJSP", family="Brandimarte", jobs=[[]] * 10, machines=6))
    for kind, sizes in [("JSP", JSP_SIZES), ("PFSP", PFSP_SIZES)]:
        for block, (n, m) in enumerate(sizes):
            for index in range(1, 11):
                prefix = "tai_jsp" if kind == "JSP" else "ta"
                rows.append(dict(id=f"{prefix}{block * 10 + index:03}", kind=kind, family="Taillard", jobs=[[]] * n, machines=m))
    for prefix, m in [("sm", 20), ("med", 40), ("lar", 60)]:
        for index, n in enumerate([10, 20, 50, 100], 1):
            for repetition in range(1, 6):
                rows.append(dict(id=f"{prefix}{index:02}_{repetition}", kind="FJSP", family="Behnke-Geiger", jobs=[[]] * n, machines=m))
    return rows


def test_selection_is_outcome_independent_and_preserves_every_size():
    candidates = candidate_frame()
    selected = select_instances(candidates)
    identifiers = {row["id"] for row in selected}
    assert len(identifiers) == 88
    assert set(LEGACY_IDS) <= identifiers
    blocks = Counter((row["kind"], len(row["jobs"]), row["machines"]) for row in selected if row["family"] in {"Taillard", "Behnke-Geiger"})
    assert len(blocks) == 32 and min(blocks.values()) == 2
    changed = copy.deepcopy(candidates[::-1])
    for row in changed:
        row.update(best_known=0, prior_result=999, processing_time=12345)
    assert {row["id"] for row in select_instances(changed)} == identifiers
    with pytest.raises(AssertionError):
        select_instances(candidates[:-1])


def test_taillard_jsp_mapping_and_global_offset():
    block = "Nb of jobs, Nb of Machines, Time seed, Machine seed, Upper bound, Lower bound\n2 2 123 456 50 40\nTimes\n3 5\n7 11\nMachines\n2 1\n1 2\n"
    rows = taillard_jsp(block * 10, 70)
    assert rows[0]["id"] == "tai_jsp071" and rows[-1]["id"] == "tai_jsp080"
    assert rows[0]["jobs"] == [[[[1, 3]], [[0, 5]]], [[[0, 7]], [[1, 11]]]]
    assert rows[0]["source_time_seed"] == 123
    with pytest.raises(AssertionError):
        taillard_jsp(block.replace("2 1", "2 2") * 10, 0)


def test_fresh_process_matrix_and_corruption_detection(tmp_path):
    instance = synthetic()
    instance.update(cohort="synthetic-test", size_block="2x2", operations=4)
    save(tmp_path / "instances.json", [instance])
    save(tmp_path / "protocol.json", dict(seeds=[19], run_order_seed=3, evaluation_budget=100,
                                         process_timeout_seconds=30, concurrent_runs=3))
    save(tmp_path / "manifest.json", dict(run_id="synthetic-test", specification_id="test",
                                         snapshot_hashes={}, planned_runs=len(METHODS),
                                         effective_sample=[{k: v for k, v in instance.items() if k != "jobs"} | {"job_count": 2}]))
    execute(tmp_path)
    assert json.loads((tmp_path / "status.json").read_text())["state"] == "completed"
    report = summarize(tmp_path)
    assert report["paired_complete_instances"] == ["synthetic"]
    assert report["statuses"] == {"completed": len(METHODS)}
    path = next((tmp_path / "runs").glob("*.json"))
    row = json.loads(path.read_text())
    row["archive"][0]["schedule"][0][4] += 1
    save(path, row)
    assert not validate(tmp_path)["passed"]
    path.unlink()
    assert len(validate(tmp_path)["missing"]) == 1
    assert summarize(tmp_path)["paired_complete_instances"] == []


def test_timeout_is_a_recorded_outcome(tmp_path):
    instance = synthetic()
    instance.update(cohort="synthetic-test", size_block="2x2", operations=4)
    save(tmp_path / "instances.json", [instance])
    (tmp_path / "work").mkdir()
    (tmp_path / "runs").mkdir()
    row = run_one(tmp_path, instance, "T", 19, dict(evaluation_budget=100, process_timeout_seconds=0.001))
    assert row["status"] == "timeout" and not row["valid"]
    assert "hypervolume" not in row
    assert len(list((tmp_path / "runs").glob("*.json"))) == 1
