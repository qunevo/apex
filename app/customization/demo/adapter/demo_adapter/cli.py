"""Explicit source-to-JSON export. Never mutate MES, Excel or control state."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from zipfile import BadZipFile

from .common import ImportFailure, require
from .mapping import build
from .sources import capture, digest
from .workbook import read_workbook


def write_json(path, value):
    # Canonical modes expand machine/person combinations; whitespace is costly at factory scale.
    path.write_text(json.dumps(value, ensure_ascii=False, allow_nan=False, separators=(",", ":")) + "\n", encoding="utf-8")


def check_engine(binary, problem):
    with tempfile.TemporaryDirectory(prefix="apex demo check ") as directory:
        root = Path(directory)
        write_json(root / "request.json", {"problem": problem})
        env = {k: v for k, v in os.environ.items() if k != "APEX_CONFIG"}
        result = subprocess.run([str(Path(binary).resolve()), "tool", "problem.import", "--args",
                                 str(root / "request.json"), "--workspace", str(root)],
                                env=env, capture_output=True, text=True, encoding="utf-8", timeout=120)
        require(result.returncode == 0, "ENGINE_INPUT", "problem", result.stderr.strip() or result.stdout.strip())


def main(argv=None):
    parser = argparse.ArgumentParser(description="Read demo MES and saved Excel into a canonical APEX problem")
    parser.add_argument("--config", type=Path, help="JSON with mes_url and planning_workbook; paths relative to this file")
    parser.add_argument("--mes-url", default=os.environ.get("DEMO_MES_URL"))
    parser.add_argument("--workbook", type=Path, default=os.environ.get("DEMO_PLANNING_WORKBOOK"))
    parser.add_argument("--horizon-end", help="Explicit plant-local/offset ISO datetime to extend the source horizon")
    parser.add_argument("--output", type=Path, required=True, help="New output directory (must not already exist)")
    parser.add_argument("--apex", help="Optional built apex executable for canonical input readiness checks")
    args = parser.parse_args(argv)
    try:
        config = json.loads(args.config.read_text(encoding="utf-8")) if args.config else {}
        mes_url = args.mes_url or config.get("mes_url")
        workbook = args.workbook
        if workbook is None and config.get("planning_workbook"):
            workbook = Path(config["planning_workbook"])
            if not workbook.is_absolute():
                workbook = args.config.resolve().parent / workbook
        require(mes_url and workbook, "CONFIG", "sources", "Provide MES URL and workbook via flags, environment or config")
        output = args.output.resolve()
        require(not output.exists(), "OUTPUT_EXISTS", str(output), "Choose a new output directory to preserve earlier snapshots")
        snapshot, content, provenance = capture(mes_url, workbook)
        plans, skills = read_workbook(content)
        problem, report = build(snapshot, plans, skills, provenance, horizon_end=args.horizon_end)
        if args.apex:
            check_engine(args.apex, problem)
        report["problem_sha256"] = digest(problem)
        report["validation"] = dict(mapping="passed", engine_input="passed" if args.apex else "not_run")
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix=".demo-import-", dir=output.parent) as directory:
            staged = Path(directory) / "snapshot"
            staged.mkdir()
            write_json(staged / "problem.json", problem)
            write_json(staged / "report.json", report)
            write_json(staged / "mes-snapshot.json", snapshot)
            (staged / "planning-source.xlsx").write_bytes(content)
            write_json(staged / "scenario.json", dict(name="Demo factory", engine="apex", customization="demo",
                       content={"facts": problem}, note=f"MES revision {snapshot['revision']}; workbook SHA256 {provenance['workbook_sha256']}"))
            os.rename(staged, output)
        print(json.dumps(dict(output=str(output), counts=report["counts"], warnings=len(report["warnings"]),
                              validation=report["validation"])))
        return 0
    except ImportFailure as error:
        print(json.dumps({"error": error.diagnostic}), file=sys.stderr)
    except (OSError, ValueError, KeyError, TypeError, BadZipFile, subprocess.SubprocessError) as error:
        print(json.dumps({"error": {"code": "SOURCE_INPUT", "message": str(error)}}), file=sys.stderr)
    return 1
