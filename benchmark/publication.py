"""Inventory and independently validate the cleaned, current-result publication tree."""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import zipfile
import zipimport
from pathlib import Path

ROOT = Path(__file__).resolve().parent
RUN = ROOT / "results/scaling-20260924-current-parallel"
SOURCE_ARCHIVE = ROOT / "algorithm-9bc9dbf.zip"
OMITTED_BINARY = "snapshot/benchmark/target/release/apex-benchmark.exe"
SELF_OUTPUTS = {"publication_manifest.json", "publication_validation.json"}


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def safe_path(root, relative):
    path = (root / relative.replace("\\", "/")).resolve()
    assert path.is_relative_to(root.resolve()), f"Path escapes publication tree: {relative}"
    return path


def archive_assets():
    with zipfile.ZipFile(SOURCE_ARCHIVE) as bundle:
        return json.loads(bundle.read("reproduction-assets/manifest.json"))


def check_sources():
    """Verify archived source bytes against the original experiment manifest."""
    manifest = read(RUN / "manifest.json")
    assets = archive_assets()
    members = {"reproduction-assets/manifest.json"}
    count = 0
    with zipfile.ZipFile(SOURCE_ARCHIVE) as bundle:
        for relative, expected in manifest["snapshot_hashes"].items():
            relative = relative.replace("\\", "/")
            if relative == OMITTED_BINARY:
                continue
            if relative.startswith("snapshot/"):
                members.add(relative)
                payload = bundle.read(relative)
            else:
                payload = safe_path(RUN, relative).read_bytes()
            assert hashlib.sha256(payload).hexdigest() == expected, relative
            count += 1
        for relative, entry in assets["files"].items():
            name = "reproduction-assets/" + relative
            members.add(name)
            assert hashlib.sha256(bundle.read(name)).hexdigest() == entry["sha256"], name
        assert len(bundle.namelist()) == len(members) and set(bundle.namelist()) == members, "Unexpected archive members"
        for name in members:
            safe_path(ROOT, name)
    return count


def extract_snapshot(output):
    """Restore verified build sources only into a new external directory."""
    output = output.resolve()
    assert not output.is_relative_to(ROOT), "Extract sources outside the publication directory"
    assert not output.exists(), "Refusing to overwrite an existing snapshot"
    check_sources()
    with zipfile.ZipFile(SOURCE_ARCHIVE) as bundle:
        files = {name.removeprefix("snapshot/"): bundle.read(name)
                 for name in bundle.namelist() if name.startswith("snapshot/")}
        for relative in archive_assets()["files"]:
            assert relative not in files, "Supplemental asset would replace frozen source"
            files[relative] = bundle.read("reproduction-assets/" + relative)
    # Validate all paths before writing any file.
    targets = [(safe_path(output, name), payload) for name, payload in files.items()]
    for target, payload in targets:
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(payload)


def check_data():
    """Verify the accessible dataset against the immutable experiment inputs."""
    for public, frozen in [("scaling-instances.json", "instances.json"),
                           ("scaling-selection.json", "selection.json")]:
        assert digest(ROOT / "data" / public) == digest(RUN / frozen), public
    expected = {}
    for row in read(RUN / "selection.json"):
        source = row["source"]
        relative = source["file"].replace("\\", "/")
        assert relative.startswith("data/raw/")
        assert relative not in expected or expected[relative] == source["sha256"]
        expected[relative] = source["sha256"]
    actual = {p.relative_to(ROOT).as_posix() for p in (ROOT / "data/raw").rglob("*") if p.is_file()}
    assert actual == set(expected), "Original dataset file coverage mismatch"
    for relative, sha256 in expected.items():
        assert digest(safe_path(ROOT, relative)) == sha256, relative
    return dict(selected_instances=len(read(RUN / "instances.json")),
                candidate_instances=len(read(RUN / "selection.json")), raw_source_files=len(expected))


def record():
    """Bind the final files; preserve original source manifests verbatim."""
    path = ROOT / "publication_manifest.json"
    assert not path.exists(), "A published inventory must not be silently replaced"
    data = check_data()
    check_sources()
    original = read(RUN / "manifest.json")
    binary_hash = next(value for key, value in original["snapshot_hashes"].items() if key.replace("\\", "/") == OMITTED_BINARY)
    hashes = {}
    forbidden = {".venv", "target", "__pycache__", ".pytest_cache", "node_modules", "work", "workbook_build"}
    for file in sorted(ROOT.rglob("*")):
        assert not file.is_symlink(), f"Unexpected symlink: {file}"
        if file.is_file():
            relative = file.relative_to(ROOT)
            assert not forbidden.intersection(relative.parts), file
            assert file.suffix.lower() not in {".exe", ".dll", ".pdb", ".pyc", ".tmp", ".log"}, file
            if relative.as_posix() not in SELF_OUTPUTS:
                hashes[relative.as_posix()] = digest(file)
    previous = "provenance/publication-v2-manifest.json"
    manifest = dict(publication_id="apex-current-benchmark-20260924-v3", current_run=RUN.name, file_hashes=hashes,
                    previous_inventory=dict(path=previous, sha256=digest(ROOT / previous)),
                    public_dataset=data,
                    omitted_frozen_files={OMITTED_BINARY: dict(sha256=binary_hash, reason="Host-specific executable retained in the local archive; rebuild from the frozen source with reproduce.py.")},
                    original_manifest_unchanged=True, raw_results_unchanged=True,
                    source_archive=dict(path=SOURCE_ARCHIVE.name, sha256=digest(SOURCE_ARCHIVE)),
                    supplemental_build_assets=archive_assets(),
                    archive_scope="Historical runs, caches, temporary requests, local environments, logs and workbook render intermediates are outside the repository. See the local cleanup record for relocation details.")
    path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(dict(recorded_files=len(hashes), original_binary_archived=True)))


def validate():
    publication = read(ROOT / "publication_manifest.json")
    for relative, expected in publication["file_hashes"].items():
        file = safe_path(ROOT, relative)
        assert file.is_file() and digest(file) == expected, f"Publication hash mismatch: {relative}"
    data = check_data()
    manifest = read(RUN / "manifest.json")
    omitted = publication["omitted_frozen_files"]
    assert set(omitted) == {OMITTED_BINARY}
    assert next(value for key, value in manifest["snapshot_hashes"].items()
                if key.replace("\\", "/") == OMITTED_BINARY) == omitted[OMITTED_BINARY]["sha256"]
    check_sources()
    for relative, expected in read(RUN / "figures-v1/raw_source_hashes.json").items():
        assert digest(safe_path(RUN, relative)) == expected
    # Load the verified audit from the ZIP without extracting legacy source.
    loader = zipimport.zipimporter(str(SOURCE_ARCHIVE) + "/snapshot/benchmark")
    namespace = {"__name__": "apex_frozen_scheduling_audit"}
    exec(loader.get_code("scheduling"), namespace)
    audit, hypervolume = namespace["audit"], namespace["hypervolume"]
    instances = {r["id"]: r for r in read(RUN / "instances.json")}
    expected = {(name, method, seed) for name in instances for method in manifest["methods"] for seed in manifest["random_seeds"]}
    seen, audited, valid, timeouts = set(), 0, 0, 0
    for file in sorted((RUN / "runs").glob("*.json")):
        row = read(file)
        key = row["instance"], row["method"], row["seed"]
        assert key in expected and key not in seen
        seen.add(key)
        assert row["status"] in {"completed", "timeout", "error"}
        assert bool(row.get("valid")) == (row["status"] == "completed")
        timeouts += row["status"] == "timeout"
        if not row.get("valid"):
            continue
        valid += 1
        instance = instances[row["instance"]]
        assert 0 < row["evaluations"] <= manifest["evaluation_budget"]
        assert row["evaluations"] == row["observed_attempts"] == len(row["trace"])
        points = []
        for entry in row["archive"]:
            audit(instance, entry["schedule"], entry["objectives"])
            points.append(entry["objectives"])
            audited += 1
        scale = [instance["serial_upper"], instance["serial_upper"] * len(instance["jobs"])]
        assert math.isclose(hypervolume(points, scale), row["hypervolume"], abs_tol=1e-12)
        assert min(p[0] for p in points) == row["best_ms"]
        assert min(p[1] for p in points) == row["best_ft"]
    assert seen == expected
    result = dict(passed=True, publication_files=len(publication["file_hashes"]), planned_runs=len(expected), recorded_runs=len(seen),
                  valid_runs=valid, timeouts=timeouts, audited_schedules=audited, public_dataset=data,
                  archived_binary_required=False, solver_executions=0)
    (ROOT / "publication_validation.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["record", "validate"])
    args = parser.parse_args()
    record() if args.action == "record" else validate()
