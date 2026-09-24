"""Prepare a fresh frozen-source reproduction; execution always needs --execute."""
from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

from publication import ROOT, RUN, archive_assets, check_data, check_sources, digest, extract_snapshot, read


def prepare(output):
    check_data()
    check_sources()
    output = output.resolve()
    assert not output.exists(), "Refusing to overwrite an existing directory"
    assert not output.is_relative_to(ROOT), "Place reproductions outside the publication directory"
    cargo = shutil.which("cargo")
    if not cargo:
        default = Path.home() / ".cargo/bin" / ("cargo.exe" if os.name == "nt" else "cargo")
        cargo = str(default) if default.is_file() else None
    assert cargo, "Rust/cargo is required; see README.md"
    output.mkdir(parents=True)
    snapshot = output / "snapshot"
    extract_snapshot(snapshot)
    environment = os.environ.copy()
    environment["CARGO_TARGET_DIR"] = str(snapshot / "benchmark/target")
    subprocess.run([cargo, "build", "--release", "--locked", "--manifest-path", str(snapshot / "benchmark/Cargo.toml")], env=environment, check=True)
    binary = snapshot / "benchmark/target/release/apex-benchmark.exe"
    if os.name != "nt":
        shutil.copy2(binary.with_suffix(""), binary)
    for name in ["instances.json", "selection.json", "protocol.json", "paper-specification.json"]:
        shutil.copy2(RUN / name, output / name)
    manifest = read(RUN / "manifest.json")
    manifest.update(run_id=output.name, created_at=datetime.now(timezone.utc).isoformat(), parent_run_ids=[RUN.name],
                    parent_role="Fresh reproduction from frozen sources and inputs; no results copied", binary_sha256=digest(binary),
                    source_environment=manifest["environment"], environment=dict(python=sys.version, platform=platform.platform(),
                    rustc=subprocess.check_output([str(Path(cargo).with_name("rustc.exe" if os.name == "nt" else "rustc")), "--version"], text=True).strip()),
                    reproduction_assets=archive_assets())
    files = [p for p in snapshot.rglob("*") if p.is_file() and "target" not in p.relative_to(snapshot).parts]
    files += [binary] + [output / n for n in ["instances.json", "selection.json", "protocol.json", "paper-specification.json"]]
    manifest["snapshot_hashes"] = {p.relative_to(output).as_posix(): digest(p) for p in files}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (output / "status.json").write_text(json.dumps(dict(state="prepared_not_started", completed=0, planned=manifest["planned_runs"])) + "\n")
    print(json.dumps(dict(prepared=str(output), benchmark_started=False, instruction="Use --execute explicitly to start experiments.")))


def execute(output):
    output = output.resolve()
    assert not output.is_relative_to(ROOT), "Do not run solvers in the publication directory"
    assert read(output / "status.json")["state"] in {"prepared_not_started", "failed", "running"}, "Not a fresh or resumable reproduction"
    script = output / "snapshot/benchmark/scaling.py"
    assert script.is_file()
    subprocess.run([sys.executable, str(script), "--execute", str(output)], check=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--check", action="store_true")
    group.add_argument("--prepare", type=Path)
    group.add_argument("--execute", type=Path)
    args = parser.parse_args()
    if args.check:
        check_data()
        print(json.dumps(dict(verified_frozen_files=check_sources(), solver_executions=0)))
    elif args.prepare:
        prepare(args.prepare)
    else:
        execute(args.execute)
