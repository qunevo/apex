"""Source, mapping and optional real-engine regression tests; no parent demo imports."""
from copy import deepcopy
from datetime import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import threading
import unittest
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "adapter"))
from demo_adapter.calendar import Clock
from demo_adapter.cli import check_engine
from demo_adapter.common import ImportFailure
from demo_adapter.mapping import build
from demo_adapter.sources import TABLES, capture, digest
from demo_adapter.workbook import read_workbook
from adapter_fixtures import HEADERS, plan_row, snapshot, workbook


class AdapterTests(unittest.TestCase):
    def setUp(self):
        self.source = snapshot()
        self.plans, self.skills = read_workbook(workbook())

    def convert(self, **kwargs):
        provenance = dict(mes_revision=self.source["revision"], mes_sha256=digest(self.source), workbook_sha256="a" * 64)
        return build(self.source, self.plans, self.skills, provenance, **kwargs)

    def fails(self, code, action):
        with self.assertRaises(ImportFailure) as context:
            action()
        self.assertEqual(code, context.exception.diagnostic["code"])

    def test_appended_rows_after_blank_rows_and_stale_dimension(self):
        rows, _ = read_workbook(workbook(extra_row=plan_row("OP3")))
        self.assertEqual(set(rows), {"OP1", "OP3"})
        added = deepcopy(self.source["records"]["operations"][1])
        added.update(id="OP3", sequence=30)
        added["machine_options"] = deepcopy(self.source["records"]["operations"][0]["machine_options"])
        self.source["records"]["operations"].append(added)
        self.plans = rows
        problem, _ = self.convert()
        self.assertEqual([task["id"] for task in problem["tasks"]], ["OP1", "OP2", "OP3"])

    def test_reordered_headers_and_new_qualification_rows(self):
        rows, skills = read_workbook(workbook(headers=list(reversed(HEADERS)), rows=[list(reversed(plan_row()))],
            qualifications=[["P3", "Yes", "No", "No", "No", "No", "No"]]))
        self.assertEqual(rows["OP1"]["machine_id"], "M1")
        self.assertTrue(skills["P3"]["Setup"])
        self.source["records"]["personnel"].append(dict(id="P3", shift_id="DAY", attendance="Present"))
        self.skills.update(skills)
        problem, _ = self.convert()
        self.assertEqual(len(problem["tasks"][0]["modes"]), 6)

    def test_duplicate_unknown_and_misjoined_excel_ids(self):
        self.fails("DUPLICATE_ID", lambda: read_workbook(workbook(rows=[plan_row(), plan_row()])))
        self.plans["OTHER"] = self.plans.pop("OP1")
        self.fails("REFERENCE", self.convert)
        self.plans["OP1"] = self.plans.pop("OTHER")
        self.plans["OP1"]["lot_id"] = "OTHER"
        self.fails("EXCEL_JOIN", self.convert)

    def test_missing_or_duplicate_headers_and_formula_inputs(self):
        self.fails("HEADERS", lambda: read_workbook(workbook(headers=HEADERS[:-1])))
        self.fails("HEADERS", lambda: read_workbook(workbook(headers=HEADERS + ["Workplace"])))
        self.fails("EXCEL_INPUT", lambda: read_workbook(workbook(formula=True)))

    def test_unknown_columns_do_not_hide_supported_rows(self):
        plans, _ = read_workbook(workbook(headers=HEADERS + ["Local note"], rows=[plan_row() + ["Added column"]]))
        self.assertIn("OP1", plans)

    def test_new_unassigned_row_uses_mes_times_without_implicit_fixation(self):
        row = ["OP1", "LOT1", "ORDER1", 10, "", "", "", "", "", "", "No", "", "New operation"]
        self.plans, self.skills = read_workbook(workbook(rows=[row]))
        problem, _ = self.convert()
        self.assertEqual(len(problem["tasks"][0]["modes"]), 4)
        self.assertEqual(problem["locks"], [])
        row[8] = 10
        self.fails("ALLOWANCE_MACHINE", lambda: read_workbook(workbook(rows=[row])))

    def test_unplanned_mes_operations_have_all_qualified_modes(self):
        problem, report = self.convert()
        self.assertEqual(report["operations_without_excel"], ["OP2"])
        self.assertEqual(len(problem["tasks"][0]["modes"]), 4)
        self.assertEqual(problem["locks"], [])
        self.assertEqual(problem["dependencies"], [dict(before="OP1", after="OP2")])

    def test_fixed_is_mode_and_time_but_proposal_is_not_a_lock(self):
        self.plans["OP1"]["fixed"] = True
        problem, _ = self.convert()
        self.assertEqual(problem["locks"], [dict(kind="mode", task="OP1", mode="M1/P1"), dict(kind="start", task="OP1", at=16200)])
        self.skills["P1"]["Setup"] = False
        self.fails("FIXED_MODE", self.convert)

    def test_malformed_fixed_row_and_stale_start_fail(self):
        self.fails("FIXED_INPUT", lambda: read_workbook(workbook(rows=[plan_row(fixed="Yes", person=None)])))
        self.plans["OP1"].update(fixed=True, start="2026-10-05T09:00")
        self.fails("FIXED_START", self.convert)

    def test_calendars_subtract_overlapping_exceptions_and_skip_cancelled(self):
        self.source["records"]["downtime"] = [dict(id="D1", resource_id="M1", start="2026-10-05T11:00", end="2026-10-05T12:00"),
            dict(id="D2", resource_id="M1", start="2026-10-05T11:30", end="2026-10-05T13:00"),
            dict(id="D3", resource_id="M1", start="2026-10-05T15:00", end="2026-10-05T16:00", cancelled=True)]
        problem, _ = self.convert()
        machine = next(r for r in problem["resources"] if r["id"] == "M1")
        self.assertEqual(machine["calendar"][:2], [dict(start=0, end=18000), dict(start=25200, end=57600)])
        person = next(r for r in problem["resources"] if r["id"] == "P1")
        self.assertEqual(person["calendar"][:2], [dict(start=0, end=14400), dict(start=16200, end=28800)])

    def test_completed_scrap_reduces_downstream_quantity_and_time(self):
        first = self.source["records"]["operations"][0]
        first.update(status="Complete", completed_quantity=8, scrap_quantity=2,
                     actual_start="2026-10-05T08:00", actual_end="2026-10-05T08:30")
        problem, report = self.convert()
        self.assertEqual(report["counts"]["closed_operations"], 1)
        self.assertEqual(problem["tasks"][0]["id"], "OP2")
        self.assertEqual(problem["tasks"][0]["quantity"], 8)
        self.assertEqual(problem["tasks"][0]["modes"][0]["phases"][1]["work"], 480)

    def make_running(self):
        self.source["records"]["operations"][0].update(status="Running", completed_quantity=3,
            resource_id="M1", person_id="P1", actual_start="2026-10-05T09:45")
        self.source["records"]["materials"][0]["on_hand"] = 97

    def test_running_preserves_history_and_does_not_double_consume(self):
        self.make_running()
        problem, report = self.convert()
        first = problem["tasks"][0]
        self.assertEqual(first["consume"], {})
        self.assertEqual(problem["inventory"]["RAW"], 90)
        self.assertEqual(report["running_material_reserved"]["RAW"], 7)
        self.assertEqual(first["execution"]["remaining_work"], {"setup": 0, "run": 900})
        self.assertEqual(first["execution"]["actual"]["start"], 13500)
        self.assertEqual(len(first["modes"]), 1)

    def test_running_conflicting_fixation_or_exhausted_estimate_fails(self):
        self.make_running()
        self.plans["OP1"]["fixed"] = True
        self.fails("FIXED_EXECUTION", self.convert)
        self.plans["OP1"]["fixed"] = False
        self.source["records"]["operations"][0]["actual_start"] = "2026-10-05T08:00"
        self.fails("RUNNING_ESTIMATE", self.convert)

    def test_receipts_already_received_not_counted_twice(self):
        self.source["records"]["receipts"] = [dict(id="R1", material_id="RAW", quantity=12, status="Received", available_at="2026-10-05T09:00"),
            dict(id="R2", material_id="RAW", quantity=20, status="Delayed", available_at="2026-10-06T10:00"),
            dict(id="R3", material_id="RAW", quantity=30, status="Confirmed", available_at="2026-10-05T09:00")]
        problem, report = self.convert()
        self.assertEqual(problem["inventory"]["RAW"], 100)
        self.assertEqual(problem["receipts"], [dict(item="RAW", at=100800, amount=20)])
        self.assertEqual(report["warnings"][0]["code"], "OVERDUE_RECEIPT")

    def test_quality_hold_and_bad_attendance_are_explicit_errors(self):
        self.source["records"]["lots"][0]["quality_status"] = "Hold"
        self.fails("QUALITY_HOLD", self.convert)
        self.source["records"]["lots"][0]["quality_status"] = "Released"
        self.plans["OP1"]["attendance_minutes"] = 30
        self.fails("ATTENDANCE", self.convert)

    def test_timezones_dst_and_explicit_horizon(self):
        clock = Clock(self.source["factory"])
        self.assertEqual(clock.seconds("2026-10-05T08:00Z"), 14400)
        self.fails("LOCAL_TIME", lambda: clock.date("2026-10-25T02:30"))
        self.fails("LOCAL_TIME", lambda: clock.date("2026-03-29T02:30"))
        self.assertEqual(clock.date(datetime(2026, 10, 5, 10)).hour, 10)
        self.fails("HORIZON", lambda: self.convert(horizon_end="2026-10-05T09:00"))

    def test_capture_pagination_retry_and_source_hash(self):
        source = self.source
        source["records"]["confirmations"] = [dict(id=f"C{i:03d}") for i in range(203)]
        calls, metadata_calls = [], 0
        def fetch(url):
            nonlocal metadata_calls
            calls.append(url)
            if url.endswith("/api/meta"):
                metadata_calls += 1
                return dict(factory=source["factory"], revision=1 if metadata_calls == 1 else 2,
                            catalog={k: {} for k in TABLES}, counts={k: len(v) for k, v in source["records"].items()})
            from urllib.parse import parse_qs, urlparse
            parsed = urlparse(url)
            rows = source["records"][parsed.path.rsplit("/", 1)[1]]
            offset = int(parse_qs(parsed.query)["offset"][0])
            return dict(rows=rows[offset:offset+200], total=len(rows))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.xlsx"
            path.write_bytes(workbook())
            result, content, provenance = capture("http://localhost:8788", path, fetch=fetch)
            self.assertEqual(result["revision"], 2)
            self.assertEqual(len(result["records"]["confirmations"]), 203)
            self.assertTrue(any("offset=200" in call for call in calls))
            self.assertEqual(provenance["mes_sha256"], digest(result))
            self.assertEqual(content, path.read_bytes())

    def test_capture_rejects_continuously_changing_sources(self):
        source, revision = self.source, 0
        def fetch(url):
            nonlocal revision
            if url.endswith("/api/meta"):
                revision += 1
                return dict(factory=source["factory"], revision=revision,
                            catalog={k: {} for k in TABLES}, counts={k: len(v) for k, v in source["records"].items()})
            entity = url.split("/api/tables/")[1].split("?")[0]
            return dict(rows=source["records"][entity], total=len(source["records"][entity]))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source.xlsx"
            path.write_bytes(workbook())
            self.fails("SOURCE_CHANGED", lambda: capture("http://localhost", path, fetch=fetch))

    def test_http_cli_from_other_directory_and_failed_export_is_atomic(self):
        source, requests = self.source, []
        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_GET(self):
                requests.append(self.path)
                if self.path == "/api/meta":
                    result = dict(factory=source["factory"], revision=source["revision"],
                                  catalog={k: {} for k in TABLES}, counts={k: len(v) for k, v in source["records"].items()})
                else:
                    entity = self.path.split("/api/tables/")[1].split("?")[0]
                    result = dict(rows=source["records"][entity], total=len(source["records"][entity]))
                body = json.dumps(result).encode()
                self.send_response(200)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)
        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            with tempfile.TemporaryDirectory(prefix="demo adapter with spaces ") as directory:
                root = Path(directory)
                content = workbook()
                (root / "source.xlsx").write_bytes(content)
                (root / "sources.json").write_text(json.dumps(dict(mes_url=f"http://127.0.0.1:{server.server_port}", planning_workbook="source.xlsx")))
                script = Path(__file__).resolve().parents[1] / "adapter/import_demo.py"
                command = [sys.executable, "-B", str(script), "--config", "sources.json", "--output", "export"]
                if os.environ.get("APEX_TEST_BINARY"):
                    command += ["--apex", os.environ["APEX_TEST_BINARY"]]
                env = {k: v for k, v in os.environ.items() if k not in ("DEMO_MES_URL", "DEMO_PLANNING_WORKBOOK")}
                result = subprocess.run(command, cwd=root, env=env, capture_output=True, text=True)
                self.assertEqual(result.returncode, 0, result.stderr)
                problem = json.loads((root / "export/problem.json").read_text())
                scenario = json.loads((root / "export/scenario.json").read_text())
                self.assertEqual(problem, scenario["content"]["facts"])
                summary = scenario["content"]["source_summary"]
                self.assertEqual(summary["counts"]["operations_without_excel"], 1)
                self.assertEqual(summary["counts"]["planned_operations"], len(problem["tasks"]))
                self.assertEqual(summary["sources"][0]["revision"], str(source["revision"]))
                self.assertLess(len(json.dumps(summary)), 4096)
                self.assertEqual((root / "source.xlsx").read_bytes(), content)
                self.assertEqual((root / "export/planning-source.xlsx").read_bytes(), content)
                self.assertNotIn("/api/plan", requests)
                self.assertNotIn("/api/skills", requests)
                old = (root / "export/problem.json").read_bytes()
                again = subprocess.run(command, cwd=root, env=env, capture_output=True, text=True)
                self.assertNotEqual(again.returncode, 0)
                self.assertEqual((root / "export/problem.json").read_bytes(), old)
                (root / "source.xlsx").write_bytes(workbook(rows=[plan_row("UNKNOWN")]))
                command[command.index("export")] = "invalid-export"
                failed = subprocess.run(command, cwd=root, env=env, capture_output=True, text=True)
                self.assertNotEqual(failed.returncode, 0)
                self.assertFalse((root / "invalid-export").exists())
                self.assertEqual(json.loads(failed.stderr)["error"]["code"], "REFERENCE")
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    @unittest.skipUnless(os.environ.get("APEX_TEST_BINARY"), "Set APEX_TEST_BINARY to a freshly built apex executable")
    def test_real_engine_readiness_plan_validation_and_corruption(self):
        binary = os.environ["APEX_TEST_BINARY"]
        self.make_running()
        problem, _ = self.convert()
        check_engine(binary, problem)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, result = root / "problem.json", root / "schedule.json"
            source.write_text(json.dumps(problem), encoding="utf-8")
            planned = subprocess.run([binary, "plan", str(source), "--out", str(result)], capture_output=True, text=True)
            self.assertEqual(planned.returncode, 0, planned.stderr)
            valid = subprocess.run([binary, "validate", str(source), str(result)], capture_output=True, text=True)
            self.assertEqual(valid.returncode, 0, valid.stderr)
            schedule = json.loads(result.read_text())
            schedule["assignments"][0]["end"] += 1
            result.write_text(json.dumps(schedule), encoding="utf-8")
            corrupt = subprocess.run([binary, "validate", str(source), str(result)], capture_output=True, text=True)
            self.assertNotEqual(corrupt.returncode, 0)


if __name__ == "__main__":
    unittest.main()
