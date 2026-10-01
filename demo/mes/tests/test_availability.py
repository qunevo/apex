"""Equipment exception boundaries, persistence and production-booking checks."""
import json
from pathlib import Path
import tempfile
import unittest

from demo.mes.availability import availability_at
from demo.mes.seed import DEMO, create_records, make_baseline
from demo.mes.store import Store


class EquipmentAvailabilityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.path = Path(self.temp.name) / "mes.sqlite"
        self.store = Store(self.path)
        self.config = json.loads((DEMO / "data/factory.json").read_text())
        self.config.update(orders=2, lots_per_order=1)
        records, skills = create_records(self.config)
        self.seed = dict(factory=self.config, records=records, qualifications=skills, plan=[])
        self.store.seed(self.seed)

    def tearDown(self):
        self.temp.cleanup()

    def machine(self, ident="CNC-01"):
        with self.store.connect() as db:
            return next(row for row in self.store.all(db, "machines") if row["id"] == ident)

    def period(self, ident="DOWN-1", start="2026-10-05T10:00", end="2026-10-05T12:00", **extra):
        data = dict(id=ident, resource_id="CNC-01", start=start, end=end, reason="Service", **extra)
        return self.store.change("downtime", {"data": data})

    def test_half_open_boundaries_and_overlapping_periods(self):
        periods = [self.period(), self.period("DOWN-2", "2026-10-05T11:00", "2026-10-05T13:00"),
                   self.period("DOWN-3", "2026-10-05T13:00", "2026-10-05T14:00")]
        machine = self.machine()
        self.assertEqual(machine["status"], "Unavailable")
        self.assertEqual(machine["unavailable_until"], "2026-10-05T14:00")
        before = availability_at(machine, periods, "2026-10-05T09:59")
        self.assertEqual(before["status"], "Available")
        self.assertEqual(before["unavailable_from"], "2026-10-05T10:00")
        self.assertEqual(availability_at(machine, periods, "2026-10-05T10:00")["status"], "Unavailable")
        self.assertEqual(availability_at(machine, periods, "2026-10-05T13:59")["status"], "Unavailable")
        self.assertEqual(availability_at(machine, periods, "2026-10-05T14:00")["status"], "Available")

    def test_cancel_or_edit_period_restores_availability(self):
        self.period()
        self.store.change("downtime", {"expected_version": 1, "data": {"cancelled": True}}, "DOWN-1")
        self.assertEqual(self.machine()["status"], "Available")
        with self.store.connect() as db:
            periods = sorted(self.store.all(db, "downtime"), key=lambda row: row["cancelled"])
            self.assertTrue(periods[-1]["cancelled"])
            self.assertIs(periods[0]["cancelled"], False)
        self.store.change("downtime", {"expected_version": 2, "data": {
            "cancelled": False, "start": "2026-10-05T11:00"}}, "DOWN-1")
        self.assertEqual(self.machine()["status"], "Available")
        self.assertEqual(self.machine()["unavailable_from"], "2026-10-05T11:00")

    def test_permanent_exception_is_explicit_and_does_not_erase_periods(self):
        self.period()
        self.store.change("machines", {"expected_version": 1, "data": {"permanently_unavailable": True}}, "CNC-01")
        self.assertEqual(self.machine()["unavailability_reason"], "Permanently unavailable")
        self.store.change("machines", {"expected_version": 2, "data": {"permanently_unavailable": False}}, "CNC-01")
        self.assertEqual(self.machine()["status"], "Unavailable")
        self.assertEqual(self.machine()["unavailable_until"], "2026-10-05T12:00")
        for data in ({"status": "Unavailable"}, {"permanently_unavailable": "false"}):
            with self.assertRaises(ValueError):
                self.store.change("machines", {"expected_version": 3, "data": data}, "CNC-01")

    def test_invalid_period_cannot_change_equipment_availability(self):
        for end in ("", "2026-10-05T09:00", "2026-10-05T10:00"):
            with self.assertRaises(ValueError):
                self.period(end=end)
        self.assertEqual(self.machine()["status"], "Available")
        with self.store.connect() as db:
            self.assertEqual(self.store.meta(db, "revision"), 0)

    def test_booking_honors_execution_period_but_not_future_period(self):
        op, lot = self.seed["records"]["operations"][0], self.seed["records"]["lots"][0]
        self.period(start="2026-10-05T09:00")
        booking = dict(expected_version=1, completed_quantity=lot["quantity"], resource_id="CNC-01",
                       person_id="P01", actual_start="2026-10-05T08:00", actual_end="2026-10-05T10:00")
        with self.assertRaises(ValueError):
            self.store.report(op["id"], booking)
        self.store.change("downtime", {"expected_version": 1, "data": {"start": "2026-10-05T11:00"}}, "DOWN-1")
        self.store.report(op["id"], booking)
        with self.store.connect() as db:
            updated = next(row for row in self.store.all(db, "operations") if row["id"] == op["id"])
            self.assertEqual(updated["status"], "Complete")

    def test_existing_global_status_is_preserved_without_reset(self):
        self.period()
        with self.store.connect() as db:
            row = db.execute("SELECT data FROM records WHERE entity='machines' AND id='CNC-01'").fetchone()
            legacy = json.loads(row[0])
            legacy.pop("permanently_unavailable")
            legacy["status"] = "Maintenance"
            db.execute("UPDATE records SET data=? WHERE entity='machines' AND id='CNC-01'", (json.dumps(legacy),))
        self.store = Store(self.path)
        self.assertTrue(self.machine()["permanently_unavailable"])
        self.assertEqual(self.machine()["_version"], 2)
        with self.store.connect() as db:
            self.assertEqual(len(self.store.all(db, "orders")), 2)
            self.assertEqual(self.store.meta(db, "revision"), 1)
            self.assertTrue(any(p["id"] == "DOWN-1" for p in self.store.all(db, "downtime")))
        self.store = Store(self.path)
        self.assertEqual(self.machine()["_version"], 2)

    def test_baseline_excludes_permanent_and_active_timed_blocks(self):
        records = self.seed["records"]
        next(m for m in records["machines"] if m["id"] == "CNC-01")["permanently_unavailable"] = True
        records["downtime"].append(dict(id="TEST", resource_id="CNC-02", start="2026-10-05T06:00",
                                         end="2026-10-05T22:00", reason="Test exception"))
        plan = make_baseline(self.config, records, self.seed["qualifications"])
        self.assertFalse(any(p["machine_id"] == "CNC-01" for p in plan))
        self.assertFalse(any(p["machine_id"] == "CNC-02" and p["start"] < "2026-10-05T22:00" for p in plan))


if __name__ == "__main__":
    unittest.main()
