"""Meaningful integration checks for the fictional source environment."""
from collections import defaultdict
from datetime import datetime
from http.client import HTTPConnection
import json
from pathlib import Path
import tempfile
import threading
import unittest

from demo.mes.catalog import CATALOG
from demo.mes.seed import DEMO, create_records, eligible, make_baseline
from demo.mes.server import Handler, ThreadingHTTPServer
from demo.mes.store import Conflict, Store


class DemoTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        config = json.loads((DEMO / "data/factory.json").read_text())
        config.update(orders=18, lots_per_order=2)
        records, skills = create_records(config)
        plan = make_baseline(config, records, skills)
        cls.seed = dict(factory=config, records=records, qualifications=skills, plan=plan)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.directory = Path(self.temp.name)
        self.store = Store(self.directory / "mes.sqlite")
        self.store.seed(self.seed)

    def tearDown(self):
        self.temp.cleanup()

    def test_seed_references_and_order_quantities(self):
        with self.store.connect() as db:
            for entity in CATALOG:
                for row in self.store.all(db, entity):
                    row.pop("_version")
                    self.store.validate(db, entity, row)
            for order in self.store.all(db, "orders"):
                self.assertEqual(order["quantity"], sum(lot["quantity"] for lot in self.store.all(db, "lots") if lot["order_id"] == order["id"]))

    def test_baseline_is_physically_consistent(self):
        verify_baseline(self, self.seed)

    def test_invalid_update_and_stale_revision_are_rejected(self):
        self.store.change("personnel", {"expected_version":1,"data":{"attendance":"Absent"}}, "P01")
        with self.assertRaises(Conflict):
            self.store.change("personnel", {"expected_version":1,"data":{"attendance":"Present"}}, "P01")
        with self.assertRaises(ValueError):
            self.store.change("personnel", {"expected_version":2,"data":{"shift_id":"MISSING"}}, "P01")
        with self.store.connect() as db:
            person = self.store.all(db, "personnel")[0]
            self.assertEqual(person["attendance"], "Absent")
            self.assertEqual(person["_version"], 2)

    def test_order_creation_expands_remainder_and_rolls_back_duplicate(self):
        data=dict(id="SO-NEW", customer="OEM 99", item_id="S-C-AL",quantity=43,lot_size=20,due="2026-10-16T16:00",priority="Urgent")
        self.store.change("orders", {"data":data.copy()})
        with self.store.connect() as db:
            lots=[lot for lot in self.store.all(db,"lots") if lot["order_id"]=="SO-NEW"]
            before=len(self.store.all(db,"lots"))
            self.assertEqual([lot["quantity"] for lot in lots],[20,20,3])
            self.assertEqual(sum(op["lot_id"] in {l["id"] for l in lots} for op in self.store.all(db,"operations")),24)
        import sqlite3
        with self.assertRaises(sqlite3.IntegrityError):
            self.store.change("orders", {"data":data.copy()})
        with self.store.connect() as db:
            self.assertEqual(len(self.store.all(db,"lots")),before)

    def test_released_workplan_stays_a_snapshot(self):
        with self.store.connect() as db:
            before=self.store.all(db,"operations")
        self.store.change("items",{"expected_version":1,"data":{"lot_size":15}},"D-C-AL")
        with self.store.connect() as db:
            self.assertEqual(self.store.all(db,"operations"),before)

    def test_progress_requires_precedence_and_valid_quantity(self):
        # A separate fixed snapshot exercises bookings in a later free shift.
        factory = dict(self.seed["factory"], as_of="2026-10-06T09:00")
        records, skills = create_records(factory)
        self.store.seed(dict(factory=factory, records=records, qualifications=skills, plan=[]), reset=True)
        with self.store.connect() as db:
            ops=self.store.all(db,"operations")
            first=next(o for o in ops if o["status"]=="Waiting" and o["sequence"]==10)
            second=next(o for o in ops if o["lot_id"]==first["lot_id"] and o["sequence"]==20)
            lot=next(l for l in self.store.all(db,"lots") if l["id"]==first["lot_id"])
        with self.assertRaises(ValueError):
            self.store.report(second["id"],dict(expected_version=1,completed_quantity=1,resource_id="DB-01"))
        with self.assertRaises(ValueError):
            self.store.report(first["id"],dict(expected_version=1,completed_quantity=lot["quantity"]+1,resource_id="CNC-03"))
        self.store.report(first["id"],dict(expected_version=1,completed_quantity=lot["quantity"],resource_id="CNC-03",
                          person_id="P01",actual_start="2026-10-06T08:00",actual_end="2026-10-06T09:00"))
        with self.store.connect() as db:
            updated=next(l for l in self.store.all(db,"lots") if l["id"]==lot["id"])
            self.assertEqual(updated["status"],"In progress")
            self.assertEqual(updated["location"],"Deburr")

    def test_http_filters_validation_and_origin(self):
        server=ThreadingHTTPServer(("127.0.0.1",0),Handler)
        server.store,server.seed,server.directory=self.store,self.seed,self.directory
        thread=threading.Thread(target=server.serve_forever,daemon=True);thread.start()
        def request(method,path,body=None,headers=None):
            conn=HTTPConnection("127.0.0.1",server.server_port)
            conn.request(method,path,json.dumps(body) if body else None,headers or {"Content-Type":"application/json"})
            response=conn.getresponse();status=response.status;data=json.loads(response.read());conn.close()
            return status,data
        try:
            status,data=request("GET","/api/tables/lots?filter_field=order_id&filter_value=SO-26001&limit=1")
            self.assertEqual((status,data["total"],len(data["rows"])),(200,2,1))
            before = request("GET", "/api/meta")[1]
            status, data = request("POST", "/api/clock", {"expected_as_of": before["factory"]["as_of"],
                                                          "as_of": "2035-10-05T10:00"})
            self.assertEqual(status, 409)
            self.assertIn("fixed", data["error"])
            self.assertEqual(request("GET", "/api/meta")[1], before)
            status,_=request("PATCH","/api/tables/orders/SO-26001",dict(expected_version=1,data={"quantity":9}))
            self.assertEqual(status,400)
            status,_=request("POST","/api/reset",{"confirmation":"wrong"})
            self.assertEqual(status,400)
            status,_=request("GET","/api/meta",headers={"Origin":"https://example.invalid"})
            self.assertEqual(status,400)
            status,_=request("GET","/api/meta",headers={"Host":"mes:8788"})
            self.assertEqual(status,400)
            server.allowed_hosts={"mes:8788", "localhost:18788"}
            status,_=request("GET","/api/meta",headers={"Host":"mes:8788"})
            self.assertEqual(status,200)
            status,_=request("GET","/api/meta",headers={"Host":"localhost:18788", "Origin":"http://localhost:18788"})
            self.assertEqual(status,200)
            status,_=request("GET","/api/meta",headers={"Host":"mes:8788", "Origin":"http://localhost:18788"})
            self.assertEqual(status,400)
            status,_=request("GET","/api/meta",headers={"Host":"mes:8788.evil.invalid"})
            self.assertEqual(status,400)
        finally:
            server.shutdown();server.server_close();thread.join()


def verify_baseline(test, seed):
    records, plan=seed["records"],seed["plan"]
    operations={o["id"]:o for o in records["operations"]}
    items={o["id"]:o for o in records["items"]}
    lots={o["id"]:o for o in records["lots"]}
    machines={o["id"]:o for o in records["machines"]}
    people={o["id"]:o for o in records["personnel"]}
    skills={o["person_id"]:o for o in seed["qualifications"]}
    occupied=defaultdict(list)
    by_lot=defaultdict(list)
    from datetime import timedelta
    for row in plan:
        op=operations[row["operation_id"]]
        start,end=map(datetime.fromisoformat,(row["start"],row["end"]))
        labor_end=start+timedelta(minutes=row["attendance_minutes"])
        test.assertTrue(eligible(machines[row["machine_id"]],items[lots[row["lot_id"]]["item_id"]],op))
        test.assertTrue(skills[row["person_id"]][op["skill"]])
        test.assertEqual((end-start).total_seconds()/60,row["setup_minutes"]+row["run_minutes"])
        test.assertLess(start.weekday(),5)
        test.assertEqual(start.date(),end.date())
        test.assertGreaterEqual(start.hour,6)
        test.assertLessEqual(end.hour+end.minute/60,22)
        windows=[(6,10),(10.5,14)] if people[row["person_id"]]["shift_id"]=="EARLY" else [(14,18),(18.5,22)]
        test.assertTrue(any(a<=start.hour+start.minute/60 and labor_end.hour+labor_end.minute/60<=b for a,b in windows))
        occupied[row["machine_id"]].append((start,end))
        occupied[row["person_id"]].append((start,labor_end))
        by_lot[row["lot_id"]].append((op["sequence"],start,end))
    for down in records["downtime"]:
        occupied[down["resource_id"]].append(tuple(map(datetime.fromisoformat,(down["start"],down["end"])) ))
    for key,spans in occupied.items():
        spans.sort()
        for previous,next_span in zip(spans,spans[1:]):
            test.assertLessEqual(previous[1],next_span[0],key)
    for spans in by_lot.values():
        spans.sort()
        for previous,next_span in zip(spans,spans[1:]):
            test.assertLessEqual(previous[2],next_span[1])
    test.assertEqual(set(operations),{p["operation_id"] for p in plan})
    # Independently replay material receipts and consumption by actual operation time.
    balances={m["id"]:m["stock"] for m in records["materials"]}
    events=[(r["available_at"],0,r["material_id"],r["quantity"]) for r in records["receipts"]]
    assembled=set()
    for row in sorted(plan,key=lambda p:p["start"]):
        op=operations[row["operation_id"]];lot=lots[row["lot_id"]];item=items[lot["item_id"]]
        needs=[]
        if op["group"]=="CNC":needs=["BODY-AL" if item["material"]=="Aluminium" else "BODY-SS"]
        if op["group"]=="Assembly" and lot["id"] not in assembled:
            needs=["SEAL-KIT"]+(["VALVE-KIT"] if item["variant"]!="Distributor" else [])
            assembled.add(lot["id"])
        if op["group"]=="Electronics":needs=["SENSOR-KIT"]
        events += [(row["start"],1,key,-lot["quantity"]) for key in needs]
    for _,_,key,quantity in sorted(events):
        balances[key]+=quantity
        test.assertGreaterEqual(balances[key],0,key)


if __name__=="__main__":
    unittest.main()
