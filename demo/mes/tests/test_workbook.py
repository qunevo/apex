"""Reset the persistent MES and shared workbook together, including failures."""
from contextlib import contextmanager
from http.client import HTTPConnection
import json
import os
from pathlib import Path
import sqlite3
import tempfile
import threading
import unittest
from unittest.mock import patch

from demo.mes.seed import DEMO, create_records
from demo.mes.server import Handler, ThreadingHTTPServer
from demo.mes.store import Conflict, Store
from demo.mes.workbook import NAME, reset_demo


class WorkbookResetTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        config = json.loads((DEMO / "data/factory.json").read_text())
        config.update(orders=2, lots_per_order=1)
        records, skills = create_records(config)
        cls.seed = dict(factory=config, records=records, qualifications=skills, plan=[])
        cls.baseline = DEMO / "planning" / NAME

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)
        self.workbook = self.directory / NAME
        self.workbook.write_bytes(b"Synthetic edited workbook")
        self.store = Store(self.directory / "mes.sqlite")
        self.store.seed(self.seed)
        self.store.change("machines", {"expected_version": 1, "data": {"name": "Edited machine"}}, "CNC-01")

    def machine(self):
        with self.store.connect() as db:
            return next(row for row in self.store.all(db, "machines") if row["id"] == "CNC-01")

    def assert_preserved(self):
        self.assertEqual(self.workbook.read_bytes(), b"Synthetic edited workbook")
        self.assertEqual(self.machine()["name"], "Edited machine")
        self.assertEqual(list(self.directory.glob(".workbook-reset-*")), [])

    def test_http_reset_restores_existing_workbook_and_mes_repeatedly(self):
        server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        server.store, server.seed, server.directory = self.store, self.seed, self.directory
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            # A reused installation must not restore the baseline just by starting.
            self.assertFalse(self.store.seed(self.seed))
            self.assert_preserved()
            for _ in range(2):
                self.workbook.write_bytes(b"Another synthetic edit")
                conn = HTTPConnection("127.0.0.1", server.server_port)
                conn.request("POST", "/api/reset", json.dumps({"confirmation": "RESET DEMO"}),
                             {"Content-Type": "application/json"})
                response = conn.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(json.loads(response.read()), {"reset": True, "workbook_reset": True})
                conn.close()
                self.assertEqual(self.workbook.read_bytes(), self.baseline.read_bytes())
                self.assertNotEqual(self.machine()["name"], "Edited machine")
                conn = HTTPConnection("127.0.0.1", server.server_port)
                conn.request("GET", "/downloads/production-planning.xlsx")
                response = conn.getresponse()
                self.assertEqual(response.status, 200)
                self.assertEqual(response.read(), self.baseline.read_bytes())
                conn.close()
        finally:
            server.shutdown()
            server.server_close()
            thread.join()

    def test_open_excel_rejects_reset_without_changing_either_source(self):
        lock = self.directory / f"~${NAME}"
        lock.write_bytes(b"Synthetic Excel owner file")
        with self.assertRaisesRegex(Conflict, "Close the shared Excel"):
            reset_demo(self.store, self.seed, self.directory, self.baseline)
        self.assert_preserved()

    def test_replace_denied_rolls_back_mes_changes(self):
        with patch("demo.mes.workbook.os.replace", side_effect=PermissionError("Synthetic lock")):
            with self.assertRaisesRegex(Conflict, "Could not reset"):
                reset_demo(self.store, self.seed, self.directory, self.baseline)
        self.assert_preserved()

    def test_database_commit_failure_restores_previous_workbook(self):
        connect = self.store.connect

        @contextmanager
        def failed_commit():
            with connect() as db:
                yield db
                raise sqlite3.OperationalError("Synthetic commit failure")

        with patch.object(self.store, "connect", failed_commit):
            with self.assertRaisesRegex(Conflict, "Could not reset"):
                reset_demo(self.store, self.seed, self.directory, self.baseline)
        self.assert_preserved()

    def test_missing_baseline_does_not_reset_mes(self):
        with self.assertRaisesRegex(Conflict, "baseline is unavailable"):
            reset_demo(self.store, self.seed, self.directory, self.directory / "missing.xlsx")
        self.assert_preserved()

    def test_failed_recovery_keeps_backup_for_manual_restore(self):
        seed, replace = self.store.seed, os.replace

        def fail_after_replace(*args, **kwargs):
            def replace_then_fail():
                kwargs["before_commit"]()
                raise sqlite3.OperationalError("Synthetic commit failure")
            return seed(args[0], reset=True, before_commit=replace_then_fail)

        with patch.object(self.store, "seed", fail_after_replace):
            with patch("demo.mes.workbook.os.replace") as mocked:
                # Perform the first replacement; deny restoration of the backup.
                def replace_once(source, target):
                    if mocked.call_count == 1:
                        return replace(source, target)
                    raise PermissionError("Synthetic lock")
                mocked.side_effect = replace_once
                with self.assertRaisesRegex(Conflict, "recovery could not finish"):
                    reset_demo(self.store, self.seed, self.directory, self.baseline)
        self.assertEqual(self.machine()["name"], "Edited machine")
        backups = list(self.directory.glob(".workbook-reset-*"))
        self.assertEqual(len(backups), 1)
        self.assertEqual(backups[0].read_bytes(), b"Synthetic edited workbook")

    def test_cleanup_failure_does_not_report_committed_reset_as_failed(self):
        with patch("demo.mes.workbook.Path.unlink", side_effect=PermissionError("Synthetic cleanup failure")):
            with self.assertLogs("demo.mes.workbook", level="WARNING"):
                result = reset_demo(self.store, self.seed, self.directory, self.baseline)
        self.assertEqual(result, {"reset": True, "workbook_reset": True})
        self.assertEqual(self.workbook.read_bytes(), self.baseline.read_bytes())
        self.assertNotEqual(self.machine()["name"], "Edited machine")


if __name__ == "__main__":
    unittest.main()
