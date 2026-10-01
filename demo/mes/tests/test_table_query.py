"""Filter semantics, export parity and execution drill-down contracts."""
import csv
from http.client import HTTPConnection
import io
import json
from pathlib import Path
import tempfile
import threading
import unittest
from urllib.parse import urlencode

from demo.mes.seed import DEMO, create_records, make_baseline
from demo.mes.server import Handler, ThreadingHTTPServer
from demo.mes.store import Store
from demo.mes.table_query import filter_rows


class ColumnFilterTests(unittest.TestCase):
    rows = [
        dict(id="A", customer="OEM 01", quantity=25, due="2026-10-05T00:00", priority="High", status="In progress"),
        dict(id="B", customer="OEM 02", quantity=50, due="2026-10-05T23:59", priority="Urgent", status="Released"),
        dict(id="C", customer="OEM 01", quantity=75, due="2026-10-06T00:00", priority="Normal", status="Complete"),
    ]

    def select(self, filters, **query):
        return filter_rows("orders", self.rows, dict(filters=json.dumps(filters), **query))[0]

    def test_columns_are_and_selections_are_or_and_text_is_case_insensitive(self):
        result = self.select({"id":{"values":["A","B"]}, "customer":{"contains":"oem"},
                              "priority":{"values":["High","Urgent"]}, "quantity":{"min":30,"max":50}})
        self.assertEqual([r["id"] for r in result], ["B"])
        self.assertEqual(len(self.select({"id":{"values":[]}})), 3)

    def test_date_boundaries_include_the_whole_day_and_combine_with_drilldown(self):
        rules = {"due":{"min":"2026-10-05", "max":"2026-10-05"}}
        self.assertEqual([r["id"] for r in self.select(rules)], ["A","B"])
        self.assertEqual([r["id"] for r in self.select(rules, filter_field="customer", filter_value="OEM 01")], ["A"])
        self.assertEqual(self.select(rules, q="not found"), [])

    def test_invalid_filters_are_rejected(self):
        for rules in [[], {"missing":{}}, {"quantity":{"values":["25"]}}, {"status":{"min":0}},
                      {"id":{"values":"A"}}, {"id":{"contains":5}}, {"quantity":{"min":True}},
                      {"quantity":{"min":float("nan")}}, {"quantity":{"min":50,"max":25}},
                      {"due":{"min":"2026-02-30"}}, {"due":{"min":"2026-10-05T00:00"}}]:
            with self.subTest(rules=rules), self.assertRaises(ValueError):
                self.select(rules)

    def test_facets_ignore_paging_and_column_filters_but_keep_parent_scope(self):
        _, facets = filter_rows("orders",self.rows,dict(filter_field="customer",filter_value="OEM 01",
            filters=json.dumps({"id":{"values":["A"]}}),limit="1"))
        self.assertEqual(facets["id"], ["A","C"])


class TableHttpTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp = tempfile.TemporaryDirectory()
        cls.store = Store(Path(cls.temp.name)/"mes.sqlite")
        config=json.loads((DEMO/"data/factory.json").read_text(encoding="utf-8"))
        config.update(orders=18,lots_per_order=2)
        records,skills=create_records(config)
        plan=make_baseline(config,records,skills)
        cls.store.seed(dict(factory=config,records=records,qualifications=skills,plan=plan))
        cls.server=ThreadingHTTPServer(("127.0.0.1",0),Handler)
        cls.server.store=cls.store
        cls.thread=threading.Thread(target=cls.server.serve_forever,daemon=True)
        cls.thread.start()

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown();cls.server.server_close();cls.thread.join();cls.temp.cleanup()

    def request(self, entity, **params):
        conn=HTTPConnection("127.0.0.1",self.server.server_port)
        conn.request("GET",f"/api/tables/{entity}?{urlencode(params)}")
        response=conn.getresponse();body=response.read().decode("utf-8-sig");conn.close()
        return response.status,body

    def test_csv_matches_full_filtered_result_before_pagination(self):
        rules=json.dumps({"quantity":{"min":10},"priority":{"values":["High","Urgent"]}})
        status,body=self.request("orders",filters=rules,limit=1,facets=1)
        page=json.loads(body)
        self.assertEqual(status,200)
        self.assertGreater(page["total"],1)
        self.assertEqual(len(page["rows"]),1)
        self.assertEqual(len(page["facets"]["id"]),18)
        _,body=self.request("orders",filters=rules,limit=1,format="csv")
        exported=list(csv.DictReader(io.StringIO(body)))
        self.assertEqual(len(exported),page["total"])
        self.assertTrue(all(int(r["quantity"])>=10 and r["priority"] in ["High","Urgent"] for r in exported))
        for invalid in ["{", "[]", '{"id":{"values":4}}']:
            self.assertEqual(self.request("orders",filters=invalid)[0],400)

    def test_actual_machine_never_comes_from_a_waiting_excel_assignment(self):
        with self.store.connect() as db:
            ops=self.store.all(db,"operations")
            lots=self.store.all(db,"lots")
            revision=self.store.meta(db,"revision")
            self.assertTrue(any(op["status"]=="Running" for op in ops))
            for lot in lots:
                work=sorted([op for op in ops if op["lot_id"]==lot["id"]],key=lambda op:op["sequence"])
                running=next((op for op in work if op["status"]=="Running"),None)
                self.assertEqual(lot["resource_id"],running["resource_id"] if running else "")
                self.assertTrue(all(op["order_id"]==lot["order_id"] for op in work))
            order=next(op["order_id"] for op in ops if op["status"]=="Running")
        status,body=self.request("operations",filter_field="order_id",filter_value=order,limit=200)
        expected=[op["id"] for op in ops if op["order_id"]==order]
        self.assertEqual(status,200)
        self.assertEqual(sorted(r["id"] for r in json.loads(body)["rows"]),sorted(expected))
        with self.store.connect() as db:
            self.assertEqual(self.store.meta(db,"revision"),revision)


if __name__ == "__main__":
    unittest.main()
