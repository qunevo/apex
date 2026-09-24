"""Freeze, execute and validate the expanded benchmark without changing the core."""
from __future__ import annotations

import os
for variable in ["OMP_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "NUMEXPR_NUM_THREADS"]:
    os.environ[variable] = "1"

import argparse
import hashlib
import json
import platform
import random
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from datetime import datetime, timezone
from pathlib import Path

from instances import ROOT
from run import METHODS, annotate, external, native, save
from scaling_data import SELECTION_SALT, prepare_expanded


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def timestamp() -> str:
    return datetime.now(timezone.utc).isoformat()


def snapshot_files() -> list[Path]:
    project = ROOT.parent
    paths = list((project / "src" / "rust").rglob("*.rs"))
    paths += list((project / "customizations").rglob("*.rs"))
    paths += list(ROOT.glob("*.py")) + list((ROOT / "native").glob("*.rs"))
    paths += [project / "Cargo.toml", project / "Cargo.lock", ROOT / "Cargo.toml",
              ROOT / "Cargo.lock", ROOT / "requirements-lock.txt", ROOT / "scaling-protocol.json",
              ROOT / "README.md"]
    return sorted(set(path for path in paths if path.is_file()))


def freeze(output: Path) -> None:
    """Create a new immutable input snapshot; never reuse an earlier run directory."""
    assert not output.exists(), f"Refusing to overwrite an experiment: {output}"
    protocol = json.loads((ROOT / "scaling-protocol.json").read_text())
    assert protocol["selection_salt"] == SELECTION_SALT
    assert protocol["methods"] == METHODS and protocol["workers"] == 1
    instances, selection = prepare_expanded()
    assert len(selection) == protocol["candidate_frame_size"]
    # Reconcile the legacy inputs by content, not identifiers alone.
    pilot_path = ROOT / "data" / "instances.json"
    if pilot_path.exists():
        pilot = {row["id"]: row for row in json.loads(pilot_path.read_text())}
        for row in instances:
            if row["cohort"] == "legacy":
                assert row["jobs"] == pilot[row["id"]]["jobs"]
                assert row["machines"] == pilot[row["id"]]["machines"]
    paths = snapshot_files()
    initial = {str(path.relative_to(ROOT.parent)): digest(path) for path in paths}
    subprocess.run([str(Path.home() / ".cargo/bin/cargo.exe"), "build", "--release", "--locked",
                    "--manifest-path", str(ROOT / "Cargo.toml")], check=True)
    assert initial == {str(path.relative_to(ROOT.parent)): digest(path) for path in paths}, "Source changed during build"
    output.mkdir(parents=True)
    snapshot = output / "snapshot"
    for path in paths:
        target = snapshot / path.relative_to(ROOT.parent)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
    binary = ROOT / "target" / "release" / "apex-benchmark.exe"
    target = snapshot / "benchmark" / "target" / "release" / binary.name
    target.parent.mkdir(parents=True)
    shutil.copy2(binary, target)
    for origin in {row["source"]["file"] for row in selection}:
        path = ROOT / origin
        target = snapshot / "benchmark" / origin
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)
    save(output / "instances.json", instances)
    save(output / "selection.json", selection)
    shutil.copy2(ROOT / "scaling-protocol.json", output / "protocol.json")
    paper_spec = ROOT.parent.parent / "apex_paper/experiments/specifications/SPEC-APEX-SCALE-001.json"
    if paper_spec.exists():
        shutil.copy2(paper_spec, output / "paper-specification.json")
    assert initial == {str(path.relative_to(ROOT.parent)): digest(path) for path in paths}, "Source changed while freezing"
    environment = dict(python=sys.version, executable=sys.executable, platform=platform.platform(),
                       processor=platform.processor(), logical_cpus=os.cpu_count(),
                       packages=subprocess.check_output([sys.executable, "-m", "pip", "freeze"], text=True).splitlines(),
                       rustc=subprocess.check_output([str(Path.home() / ".cargo/bin/rustc.exe"), "--version"], text=True).strip())
    if os.name == "nt":
        environment["hardware"] = json.loads(subprocess.check_output([
            "powershell", "-NoProfile", "-Command",
            "Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors | ConvertTo-Json"
        ], text=True))
    files = [p for p in snapshot.rglob("*") if p.is_file()]
    files += [output / name for name in ["instances.json", "selection.json", "protocol.json"]]
    if (output / "paper-specification.json").exists():
        files.append(output / "paper-specification.json")
    manifest = dict(
        run_id=output.name, created_at=timestamp(), specification_id=protocol["specification_id"],
        setting_id="fixed-1000-seeds-19-42-73", data_interface_id="APEX-PUBLIC-SCALE-001",
        code_fingerprint=hashlib.sha256(json.dumps(initial, sort_keys=True).encode()).hexdigest(),
        data_fingerprint=digest(output / "instances.json"), specification_fingerprint=digest(output / "protocol.json"),
        source_hashes=initial, binary_sha256=digest(binary), environment=environment,
        random_seeds=protocol["seeds"], methods=METHODS, evaluation_budget=protocol["evaluation_budget"],
        concurrent_runs=protocol["concurrent_runs"], workers_per_algorithm=protocol["workers"],
        effective_sample=[{k: v for k, v in row.items() if k != "jobs"} | {"job_count": len(row["jobs"])} for row in instances],
        planned_runs=len(instances) * len(METHODS) * len(protocol["seeds"]),
        parent_run_ids=["pilot", "xe-change/comparison"],
        parent_role="Historical development context only; no old measurements imported",
        snapshot_hashes={str(path.relative_to(output)): digest(path) for path in files},
        artifact_paths=["runs", "results.json", "report.md", "integrity-check.json", "selection.json", "status.json"],
        validator_paths=["snapshot/benchmark/scaling_report.py", "snapshot/benchmark/scheduling.py"],
    )
    save(output / "manifest.json", manifest)
    save(output / "status.json", dict(state="frozen", created_at=timestamp(), completed=0, planned=manifest["planned_runs"]))
    print(json.dumps(dict(state="frozen", output=str(output), instances=len(instances), runs=manifest["planned_runs"])), flush=True)


def worker(request_path: Path, result_path: Path) -> None:
    request = json.loads(request_path.read_text())
    instances = json.loads(Path(request["instances_path"]).read_text())
    instance = next(row for row in instances if row["id"] == request["instance"])
    result = {key: request[key] for key in ["instance", "method", "seed", "nominal_evaluations"]}
    result.update(kind=instance["kind"], size_block=instance["size_block"], operations=instance["operations"],
                  cohort=instance["cohort"], started_at=timestamp())
    started = time.perf_counter()
    try:
        method, budget, seed = request["method"], request["nominal_evaluations"], request["seed"]
        result.update(external(instance, method, budget, seed) if method in METHODS[8:] else native(instance, method, budget, seed))
        annotate(instance, result, budget)
        result["status"] = "completed"
    except subprocess.TimeoutExpired:
        result.update(valid=False, status="timeout", error="Native process exceeded the watchdog")
    except Exception as error:
        result.update(valid=False, status="error", error=f"{type(error).__name__}: {error}")
    result.update(worker_seconds=time.perf_counter() - started, finished_at=timestamp())
    save(result_path, result)


def run_one(output: Path, instance: dict, method: str, seed: int, protocol: dict) -> dict:
    """Supervise one isolated process with unique files and a process-tree deadline."""
    identity = f"{instance['kind']}-{instance['id']}-{method}-{seed}"
    result_path = output / "runs" / f"{identity}.json"
    if result_path.exists():
        return json.loads(result_path.read_text())
    request = dict(instance=instance["id"], method=method, seed=seed,
                   nominal_evaluations=protocol["evaluation_budget"], instances_path=str(output / "instances.json"))
    request_path = output / "work" / f"{identity}-request.json"
    temporary_result = output / "work" / f"{identity}-result.json"
    log_path = output / "work" / f"{identity}.log"
    save(request_path, request)
    job_start = time.perf_counter()
    try:
        with log_path.open("w", encoding="utf-8") as log:
            process = subprocess.Popen([sys.executable, str(Path(__file__).resolve()), "--worker", str(request_path), str(temporary_result)],
                                       stdout=log, stderr=subprocess.STDOUT,
                                       creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
            try:
                process.wait(timeout=protocol["process_timeout_seconds"])
                if temporary_result.exists():
                    result = json.loads(temporary_result.read_text())
                    temporary_result.unlink()
                else:
                    result = dict(valid=False, status="error", error=log_path.read_text()[-6000:])
            except subprocess.TimeoutExpired:
                if os.name == "nt":
                    subprocess.run(["taskkill", "/PID", str(process.pid), "/T", "/F"], capture_output=True, check=False)
                else:
                    process.kill()
                process.wait()
                result = dict(valid=False, status="timeout", error="Common process watchdog exceeded; no final audited archive available")
    except Exception as error:
        result = dict(valid=False, status="error", error=f"Supervisor: {type(error).__name__}: {error}")
    result.update(instance=instance["id"], kind=instance["kind"], method=method, seed=seed,
                  nominal_evaluations=protocol["evaluation_budget"], operations=instance["operations"],
                  size_block=instance["size_block"], cohort=instance["cohort"],
                  process_seconds=time.perf_counter() - job_start, recorded_at=timestamp())
    save(result_path, result)
    return result


def execute(output: Path) -> None:
    from scaling_report import summarize, validate
    protocol = json.loads((output / "protocol.json").read_text())
    manifest = json.loads((output / "manifest.json").read_text())
    for relative, expected in manifest["snapshot_hashes"].items():
        assert digest(output / relative) == expected, f"Changed frozen input: {relative}"
    instances = json.loads((output / "instances.json").read_text())
    jobs = [(row, method, seed) for row in instances for method in METHODS for seed in protocol["seeds"]]
    random.Random(protocol["run_order_seed"]).shuffle(jobs)
    (output / "runs").mkdir(exist_ok=True)
    (output / "work").mkdir(exist_ok=True)
    started = time.perf_counter()
    lock = output / "execution.lock"
    with lock.open("x") as handle:
        handle.write(str(os.getpid()))
    try:
        concurrency = protocol["concurrent_runs"]
        save(output / "status.json", dict(state="running", updated_at=timestamp(), completed=0,
                                           planned=len(jobs), concurrent_runs=concurrency, pid=os.getpid()))
        summarize(output)
        with ThreadPoolExecutor(max_workers=concurrency) as pool:
            futures = [pool.submit(run_one, output, instance, method, seed, protocol) for instance, method, seed in jobs]
            for index, future in enumerate(as_completed(futures), 1):
                result = future.result()
                identity = f"{result['kind']}-{result['instance']}-{result['method']}-{result['seed']}"
                save(output / "status.json", dict(state="running", updated_at=timestamp(), completed=index,
                                                   planned=len(jobs), last_completed=identity,
                                                   concurrent_runs=concurrency, pid=os.getpid()))
                print(json.dumps(dict(done=index, total=len(jobs), run=identity, status=result["status"],
                                      seconds=round(result["process_seconds"], 2), elapsed=round(time.perf_counter() - started))), flush=True)
                if index % 96 == 0:
                    summarize(output)
        summarize(output)
        validation = validate(output)
        save(output / "status.json", dict(state="completed" if validation["passed"] else "validation_failed",
                                           updated_at=timestamp(), completed=len(jobs), planned=len(jobs),
                                           integrity_check="integrity-check.json"))
        assert validation["passed"], validation
    except Exception as error:
        save(output / "status.json", dict(state="failed", updated_at=timestamp(),
                                           error=f"{type(error).__name__}: {error}"))
        raise
    finally:
        lock.unlink(missing_ok=True)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--out", type=Path)
    parser.add_argument("--execute", type=Path)
    parser.add_argument("--worker", nargs=2, type=Path)
    args = parser.parse_args()
    if args.worker:
        worker(*args.worker)
    elif args.execute:
        execute(args.execute.resolve())
    else:
        assert args.out is not None, "Specify a new --out directory"
        freeze(args.out.resolve())


if __name__ == "__main__":
    main()
