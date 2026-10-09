"""Business acceptance checks independent of the synthetic planning heuristic."""
from copy import deepcopy
from datetime import datetime
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from demo.mes.seed import DEMO, create_records
from demo.mes.store import Conflict, Store


class MesWorkflowTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.store = Store(Path(self.temp.name) / "mes.sqlite")
        config = json.loads((DEMO / "data/factory.json").read_text())
        config.update(orders=2, lots_per_order=1)
        records, skills = create_records(config)
        self.seed = dict(factory=config, records=records, qualifications=skills, plan=[])
        self.store.seed(self.seed)

    def tearDown(self):
        self.temp.cleanup()

    def all(self, entity):
        with self.store.connect() as db:
            return self.store.all(db, entity)

    def row(self, entity, ident):
        return next(r for r in self.all(entity) if r["id"] == ident)

    def change(self, entity, ident, **data):
        return self.store.change(entity, dict(expected_version=self.row(entity, ident)["_version"], data=data), ident)

    def new_order(self, **extra):
        data = dict(id="SO-NEW", customer="Synthetic OEM", item_id="D-C-AL", quantity=10, lot_size=10,
                    due="2026-10-09T16:00", priority="Normal", **extra)
        self.store.change("orders", dict(data=data))
        lot = next(l for l in self.all("lots") if l["order_id"] == data["id"])
        return sorted((o for o in self.all("operations") if o["lot_id"] == lot["id"]), key=lambda o: o["sequence"])

    def draft(self):
        return self.store.routing_action("RT-D", "revise", dict(expected_version=1, id="RT-D-B", revision="B"))

    def book(self, ident="L00001-010", **extra):
        op = self.row("operations", ident)
        qty = self.row("lots", op["lot_id"])["quantity"]
        payload = dict(expected_version=op["_version"], completed_quantity=qty, scrap_quantity=0,
                       resource_id="CNC-01", person_id="P01", actual_start="2026-10-05T06:00", actual_end="2026-10-05T07:00")
        payload.update(extra)
        return self.store.report(ident, payload)

    def test_alternative_route_changes_flow_and_keeps_remainder(self):
        ops = self.new_order(routing_id="RT-D-SPLIT")
        self.assertEqual([o["name"] for o in ops[:2]], ["Rough machining", "Finish drilling & threads"])
        self.assertEqual({o["resource_id"] for o in ops[0]["machine_options"]}, {"CNC-03", "CNC-05"})
        self.assertNotEqual(*[o["minutes_per_unit"] for o in ops[0]["machine_options"]])
        self.assertEqual(ops[0]["material_requirements"][0]["material_id"], "BODY-AL")
        self.assertEqual(ops[1]["material_requirements"], [])

    def test_released_master_and_snapshot_are_immutable(self):
        with self.assertRaisesRegex(ValueError, "locked"):
            self.change("routing_steps", "RT-D-S10", name="Changed")
        before = self.row("operations", "L00001-010")
        self.change("items", "D-C-AL", material="Stainless steel")
        self.assertEqual(before, self.row("operations", "L00001-010"))
        # The original aluminium release still accepts CNC-01 and consumes BODY-AL.
        self.book()
        self.assertEqual(self.all("material_issues")[0]["material_id"], "BODY-AL")
        with self.assertRaisesRegex(ValueError, "read-only"):
            self.change("operations", "L00001-010", machine_options=[])

    def test_revision_edit_release_and_article_approval(self):
        self.draft()
        self.change("routing_steps", "RT-D-B-S10", instruction="Inspect the new fixture")
        self.change("routing_modes", "RT-D-B-S10-M1", minutes_per_unit=3.5)
        with self.assertRaisesRegex(ValueError, "released workplan"):
            self.new_order(routing_id="RT-D-B")
        self.store.routing_action("RT-D-B", "release", dict(expected_version=1))
        with self.assertRaisesRegex(ValueError, "not approved"):
            self.new_order(routing_id="RT-D-B")
        self.store.change("item_routings", dict(data=dict(id="D-B", item_id="D-C-AL", routing_id="RT-D-B", active=True)))
        ops = self.new_order(routing_id="RT-D-B")
        self.assertEqual(ops[0]["instruction"], "Inspect the new fixture")
        self.assertEqual(ops[0]["machine_options"][0]["minutes_per_unit"], 3.5)
        self.assertEqual(self.row("lots", ops[0]["lot_id"])["routing_revision"], "B")
        with self.assertRaisesRegex(ValueError, "locked"):
            self.change("routing_modes", "RT-D-B-S10-M1", minutes_per_unit=9)

    def test_invalid_workplans_cannot_be_released(self):
        self.draft()
        self.change("routing_steps", "RT-D-B-S20", sequence=10)
        with self.assertRaisesRegex(ValueError, "unique"):
            self.store.routing_action("RT-D-B", "release", dict(expected_version=1))
        self.change("routing_steps", "RT-D-B-S20", sequence=20)
        with self.assertRaisesRegex(ValueError, "resource group"):
            self.change("routing_modes", "RT-D-B-S10-M1", resource_id="AS-01")
        for mode in self.all("routing_modes"):
            if mode["step_id"] == "RT-D-B-S10":
                self.change("routing_modes", mode["id"], active=False)
        with self.assertRaisesRegex(ValueError, "at least one"):
            self.store.routing_action("RT-D-B", "release", dict(expected_version=1))

    def test_scrap_conserves_downstream_quantities_and_posts_once(self):
        qty = self.row("lots", "L00001")["quantity"]
        self.book(completed_quantity=qty-1, scrap_quantity=1, scrap_reason="Damaged thread")
        self.assertEqual(self.all("material_issues")[0]["quantity"], qty)
        with self.assertRaisesRegex(ValueError, "available input"):
            self.book("L00001-020", completed_quantity=qty, resource_id="DB-01", person_id="P04", actual_start="2026-10-05T07:00", actual_end="2026-10-05T08:00")
        self.book("L00001-020", completed_quantity=qty-1, resource_id="DB-01", person_id="P04", actual_start="2026-10-05T07:00", actual_end="2026-10-05T08:00")
        self.assertEqual(self.row("lots", "L00001")["scrap_quantity"], 1)
        count = len(self.all("confirmations"))
        with self.assertRaisesRegex(ValueError, "closed"):
            self.book()
        self.assertEqual(len(self.all("confirmations")), count)

    def test_total_scrap_closes_empty_following_steps(self):
        qty = self.row("lots", "L00001")["quantity"]
        self.book(completed_quantity=0, scrap_quantity=qty, scrap_reason="Wrong blank")
        lot = self.row("lots", "L00001")
        self.assertEqual((lot["status"], lot["good_quantity"], lot["scrap_quantity"]), ("Complete", 0, qty))
        self.assertTrue(all(o["status"] == "Skipped" for o in self.all("operations") if o["lot_id"] == lot["id"] and o["sequence"] > 10))

    def test_partial_confirmation_issues_only_deltas_and_rejects_stale_replay(self):
        qty = self.row("lots", "L00001")["quantity"]
        self.book(completed_quantity=1, actual_end="")
        with self.assertRaises(Conflict):
            self.book(expected_version=1)
        with self.assertRaisesRegex(ValueError, "in-progress confirmation"):
            self.book(completed_quantity=qty)
        self.book(completed_quantity=qty, actual_end="2026-10-05T10:00")
        self.assertEqual(sum(i["quantity"] for i in self.all("material_issues")), qty)
        with self.assertRaisesRegex(ValueError, "immutable"):
            self.book(completed_quantity=qty)

    def test_stock_shortage_rolls_back_and_only_received_supply_counts(self):
        self.change("materials", "BODY-AL", stock=0)
        self.store.change("receipts", dict(data=dict(id="IN-NOW", material_id="BODY-AL", quantity=100,
                          available_at="2026-10-05T06:00", status="Confirmed", note="Synthetic delivery")))
        with self.assertRaisesRegex(ValueError, "Not enough"):
            self.book()
        self.assertEqual(self.all("confirmations"), [])
        self.assertEqual(self.all("material_issues"), [])
        self.assertEqual(self.row("operations", "L00001-010")["status"], "Waiting")
        self.change("receipts", "IN-NOW", status="Received")
        self.book()
        with self.assertRaisesRegex(ValueError, "negative"):
            self.change("receipts", "IN-NOW", status="Confirmed")

    def test_execution_rejects_wrong_skill_break_absence_and_downtime(self):
        fixture = deepcopy(self.seed)
        fixture["factory"]["as_of"] = "2026-10-05T11:00"
        self.store.seed(fixture, reset=True)
        for extra in [dict(person_id="P04"), dict(actual_start="2026-10-05T05:00"),
                      dict(actual_start="2026-10-05T09:55", actual_end="2026-10-05T10:20")]:
            with self.assertRaises(ValueError):
                self.book(**extra)
        self.store.change("absences", dict(data=dict(id="A-1", person_id="P01", start="2026-10-05T06:00", end="2026-10-05T08:00", reason="Training")))
        with self.assertRaisesRegex(ValueError, "absent"):
            self.book()
        self.change("absences", "A-1", cancelled=True)
        self.store.change("downtime", dict(data=dict(id="D-1", resource_id="CNC-01", start="2026-10-05T06:30", end="2026-10-05T08:00", reason="Fault")))
        with self.assertRaisesRegex(ValueError, "unavailable"):
            self.book()
        self.change("downtime", "D-1", cancelled=True)
        self.book()

    def test_holds_and_scrap_reason_are_required(self):
        with self.assertRaisesRegex(ValueError, "reason"):
            self.change("lots", "L00001", quality_status="Hold")
        self.change("lots", "L00001", quality_status="Hold", hold_reason="Awaiting inspection")
        with self.assertRaisesRegex(ValueError, "quality hold"):
            self.book()
        self.change("lots", "L00001", quality_status="Released")
        qty = self.row("lots", "L00001")["quantity"]
        with self.assertRaisesRegex(ValueError, "scrap reason"):
            self.book(completed_quantity=qty-1, scrap_quantity=1)

    def test_machine_and_operator_cannot_overlap(self):
        self.book(resource_id="CNC-03")
        with self.assertRaisesRegex(ValueError, "equipment already"):
            self.book("L00002-010", resource_id="CNC-03", person_id="P02",
                      actual_start="2026-10-05T06:30", actual_end="2026-10-05T07:30")
        with self.assertRaisesRegex(ValueError, "operator already"):
            self.book("L00002-010", resource_id="CNC-04", person_id="P01",
                      actual_start="2026-10-05T06:05", actual_end="2026-10-05T07:00")
        self.book("L00002-010", resource_id="CNC-04", person_id="P02",
                  actual_start="2026-10-05T06:30", actual_end="2026-10-05T07:30")

    def test_material_requirements_are_owned_by_released_steps(self):
        before = self.row("operations", "L00001-040")["material_requirements"]
        self.draft()
        self.change("routing_materials", "RT-D-B-S40-B1", quantity_per_unit=2)
        self.store.routing_action("RT-D-B", "release", dict(expected_version=1))
        self.store.change("item_routings", dict(data=dict(id="D-B", item_id="D-C-AL", routing_id="RT-D-B", active=True)))
        ops = self.new_order(routing_id="RT-D-B")
        self.assertEqual(ops[3]["material_requirements"], [dict(material_id="SEAL-KIT", quantity_per_unit=2, quantity=20)])
        self.assertEqual(before, self.row("operations", "L00001-040")["material_requirements"])

    def test_fixed_snapshot_survives_edits_wall_clock_and_reopen(self):
        before = {entity: self.all(entity) for entity in ("orders", "receipts", "downtime")}
        for at in ("2026-10-05T10:00", "2026-10-05T14:00", "2035-10-05T10:00"):
            with self.assertRaisesRegex(Conflict, "snapshot is fixed"):
                self.store.advance_clock(dict(expected_as_of="2026-10-05T10:00", as_of=at))
        with patch("demo.mes.store.datetime", wraps=datetime) as wall_clock:
            wall_clock.now.return_value = datetime(2035, 10, 5, 14)
            self.change("items", "D-C-AL", name="Synthetic edit in a later real year")
        self.store = Store(self.store.path)
        self.assertFalse(self.store.seed(self.seed))
        with self.store.connect() as db:
            self.assertEqual(self.store.meta(db, "factory"), self.seed["factory"])
            self.assertTrue(db.execute("SELECT at FROM audit").fetchone()[0].startswith("2035-"))
        self.assertEqual({entity: self.all(entity) for entity in before}, before)

    def test_historical_stock_uses_its_fixed_snapshot(self):
        fixture = deepcopy(self.seed)
        fixture["factory"]["as_of"] = "2026-10-05T14:00"
        self.store.seed(fixture, reset=True)
        self.change("materials", "BODY-AL", stock=0)
        self.store.change("receipts", dict(data=dict(id="LATE", material_id="BODY-AL", quantity=100,
            available_at="2026-10-05T12:00", status="Received", note="Later supply")))
        with self.assertRaisesRegex(ValueError, "negative"):
            self.book()

    def test_open_confirmation_cannot_invent_instant_output_or_start_during_break(self):
        with self.assertRaisesRegex(ValueError, "finish after start"):
            self.book(completed_quantity=1, actual_start="2026-10-05T10:00", actual_end="")
        with self.assertRaisesRegex(ValueError, "excluding breaks"):
            self.book(completed_quantity=0, actual_start="2026-10-05T10:00", actual_end="")

    def test_upgrade_preserves_edits_and_is_idempotent(self):
        legacy = deepcopy(self.seed)
        with self.store.connect() as db:
            db.execute("DELETE FROM records")
            db.execute("DELETE FROM metadata WHERE key='mes_schema'")
            for entity, data in legacy["records"].items():
                for row in data:
                    if entity == "orders":
                        row["note"] = "Keep this local edit"
                    self.store.insert(db, entity, row)
            self.store.put_meta(db, "revision", 7)
        self.store.seed(self.seed)
        self.assertEqual(self.row("orders", "SO-26001")["note"], "Keep this local edit")
        before = {k: self.all(k) for k in ("orders", "lots", "operations", "routings")}
        self.store.seed(self.seed)
        self.assertEqual(before, {k: self.all(k) for k in before})
        with self.store.connect() as db:
            self.assertEqual(self.store.meta(db, "revision"), 7)


if __name__ == "__main__":
    unittest.main()
