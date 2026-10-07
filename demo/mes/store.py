"""Transactional demo state with explicit validation and optimistic revisions."""
from datetime import datetime
from contextlib import contextmanager
import json
import re
import sqlite3

from .availability import availability_at, normalize_machine
from .catalog import CATALOG
from . import execution, workplans
from .upgrade import install


class Conflict(ValueError):
    pass


class Store:
    def __init__(self, path):
        self.path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        with self.connect() as db:
            db.executescript("""
                CREATE TABLE IF NOT EXISTS records (entity TEXT, id TEXT, data TEXT NOT NULL,
                    version INTEGER NOT NULL DEFAULT 1, PRIMARY KEY(entity,id));
                CREATE TABLE IF NOT EXISTS metadata (key TEXT PRIMARY KEY, value TEXT NOT NULL);
                CREATE TABLE IF NOT EXISTS audit (id INTEGER PRIMARY KEY, at TEXT NOT NULL,
                    entity TEXT NOT NULL, record_id TEXT NOT NULL, detail TEXT NOT NULL);
            """)
            # Upgrade only equipment fields; retain business edits and audit history.
            for row in db.execute("SELECT id,data FROM records WHERE entity='machines'").fetchall():
                before = json.loads(row["data"])
                after = normalize_machine(before)
                if before != after:
                    db.execute("UPDATE records SET data=?,version=version+1 WHERE entity='machines' AND id=?",
                               (json.dumps(after), row["id"]))

    @contextmanager
    def connect(self):
        db = sqlite3.connect(self.path, timeout=10)
        db.row_factory = sqlite3.Row
        try:
            with db:
                yield db
        finally:
            db.close()

    @staticmethod
    def all(db, entity):
        rows = [dict(json.loads(row["data"]), _version=row["version"])
                for row in db.execute("SELECT data,version FROM records WHERE entity=? ORDER BY id", (entity,))]
        if entity in ("downtime", "absences"):
            rows = [dict(row, cancelled=bool(row.get("cancelled", False))) for row in rows]
        if entity == "machines":
            at = Store.meta(db, "factory")["as_of"]
            periods = Store.all(db, "downtime")
            rows = [dict(row, **availability_at(row, periods, at)) for row in rows]
        if entity == "routings":
            rows = [dict(row, steps=" / ".join(s["name"] for s in workplans.steps_for(db, row["id"]))) for row in rows]
        if entity == "materials":
            stock = execution.balances(db, Store.meta(db, "factory")["as_of"])
            rows = [dict(row, on_hand=stock[row["id"]]) for row in rows]
        if entity == "operations":
            orders = {r["id"]: r["order_id"] for r in execution.rows(db, "lots")}
            rows = [dict(row, order_id=orders[row["lot_id"]]) for row in rows]
        if entity == "lots":
            active = {}
            for op in sorted(execution.rows(db, "operations"), key=lambda r: r["sequence"]):
                if op["status"] not in ("Complete", "Skipped"):
                    active.setdefault(op["lot_id"], op)
            rows = [dict(row, current_operation=active.get(row["id"], {}).get("name", ""),
                         resource_id=active[row["id"]].get("resource_id", "")
                         if active.get(row["id"], {}).get("status") == "Running" else "") for row in rows]
        return rows

    @staticmethod
    def meta(db, key):
        row = db.execute("SELECT value FROM metadata WHERE key=?", (key,)).fetchone()
        return json.loads(row[0]) if row else None

    @staticmethod
    def put_meta(db, key, value):
        db.execute("INSERT OR REPLACE INTO metadata VALUES (?,?)", (key, json.dumps(value)))

    @staticmethod
    def insert(db, entity, row):
        db.execute("INSERT INTO records(entity,id,data) VALUES (?,?,?)", (entity, row["id"], json.dumps(row)))

    def seed(self, data, reset=False, before_commit=None):
        with self.connect() as db:
            db.execute("BEGIN IMMEDIATE")
            if self.meta(db, "factory") and not reset:
                install(self, db, data, existing=True)
                return False
            db.execute("DELETE FROM records")
            db.execute("DELETE FROM metadata")
            db.execute("DELETE FROM audit")
            for entity, rows in data["records"].items():
                for row in rows:
                    if entity == "machines":
                        row = normalize_machine(row)
                    self.insert(db, entity, row)
            for key in ["factory", "qualifications", "plan"]:
                self.put_meta(db, key, data[key])
            self.put_meta(db, "revision", 0)
            install(self, db, data, existing=False)
            if before_commit is not None:
                before_commit()
            return True

    @staticmethod
    def validate(db, entity, data):
        schema = CATALOG[entity]
        fields = {col["key"]: col for col in schema["columns"]}
        unknown = set(data) - set(fields)
        if unknown:
            raise ValueError("Unknown fields: " + ", ".join(sorted(unknown)))
        for key, col in fields.items():
            value = data.get(key)
            if col["required"] and (value is None or value == ""):
                raise ValueError(f"{col['label']} is required")
            if value in (None, ""):
                continue
            if col["type"] == "json":
                if not isinstance(value, list):
                    raise ValueError(f"{col['label']} must be a list")
            elif col["type"] == "boolean":
                if not isinstance(value, bool):
                    raise ValueError(f"{col['label']} must be true or false")
            elif col["type"] in ("number", "integer"):
                if isinstance(value, bool) or not isinstance(value, (float, int)) or not 0 <= value <= 1_000_000:
                    raise ValueError(f"{col['label']} must be between 0 and 1,000,000")
                if col["type"] == "integer" and int(value) != value:
                    raise ValueError(f"{col['label']} must be a whole number")
                if key in ("quantity", "lot_size") and value <= 0:
                    raise ValueError(f"{col['label']} must be positive")
            elif not isinstance(value, str) or len(value) > 2000:
                raise ValueError(f"{col['label']} must be text of at most 2,000 characters")
            if col["type"] == "datetime":
                if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}", value):
                    raise ValueError(f"{col['label']} must use local date and time")
                datetime.fromisoformat(value)
            if col["type"] == "time":
                if not re.fullmatch(r"(?:[01]\d|2[0-3]):[0-5]\d", value):
                    raise ValueError(f"Invalid time for {col['label']}")
            if col["choices"] and value not in col["choices"]:
                raise ValueError(f"Unknown {col['label']}")
            if col["ref"] and not db.execute("SELECT 1 FROM records WHERE entity=? AND id=?", (col["ref"], value)).fetchone():
                raise ValueError(f"Unknown reference for {col['label']}: {value}")
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]{0,49}", data["id"]):
            raise ValueError("ID must contain 1-50 letters, numbers, dots, hyphens or underscores")
        if entity in ("downtime", "absences") and data["end"] <= data["start"]:
            raise ValueError("The end must be after the start")
        if entity == "shifts" and not data["start"] < data["end"]:
            raise ValueError("Use a same-day shift with end after start")
        if entity == "shifts" and not data["start"] <= data["break_start"] <= data["break_end"] <= data["end"]:
            raise ValueError("The break must be inside the shift")

    def change(self, entity, payload, record_id=None):
        if entity not in CATALOG:
            raise ValueError("Unknown table")
        with self.connect() as db:
            db.execute("BEGIN IMMEDIATE")
            data = dict(payload.get("data", {}))
            if record_id:
                old = db.execute("SELECT data,version FROM records WHERE entity=? AND id=?", (entity, record_id)).fetchone()
                if old is None:
                    raise ValueError("Record not found")
                if payload.get("expected_version") != old["version"]:
                    raise Conflict("This row changed. Reload it before saving.")
                fields = {c["key"] for c in CATALOG[entity]["columns"] if c["editable"]}
                if set(data) - fields:
                    raise ValueError("The request changes a read-only field")
                result = json.loads(old["data"])
                previous = result.copy()
                result.update(data)
                self.validate(db, entity, result)
                workplans.guard_master_edit(db, entity, result, previous)
                db.execute("UPDATE records SET data=?,version=version+1 WHERE entity=? AND id=?", (json.dumps(result), entity, record_id))
                version = old["version"] + 1
            else:
                if not CATALOG[entity]["create"]:
                    raise ValueError("Create this record through its owning workflow")
                if entity == "routings":
                    if data.get("status", "Draft") != "Draft":
                        raise ValueError("New workplans start as Draft")
                    data["status"] = "Draft"
                if any(c["computed"] and c["key"] in data for c in CATALOG[entity]["columns"]):
                    raise ValueError("Computed values cannot be supplied")
                result = self.create_order(db, data) if entity == "orders" else data
                self.validate(db, entity, result)
                workplans.guard_master_edit(db, entity, result)
                self.insert(db, entity, result)
                if entity == "items":
                    self.insert(db, "item_routings", dict(id=f"LINK-{self.meta(db, 'revision')+1}",
                                item_id=result["id"], routing_id=result["routing_id"], active=True))
                record_id, version = result["id"], 1
            if entity in ("materials", "receipts"):
                execution.check_stock(db)
            if entity == "lots" and result.get("quality_status") == "Hold" and not result.get("hold_reason", "").strip():
                raise ValueError("A quality hold needs a reason")
            revision = self.meta(db, "revision") + 1
            self.put_meta(db, "revision", revision)
            db.execute("INSERT INTO audit(at,entity,record_id,detail) VALUES (?,?,?,?)",
                       (datetime.now().isoformat(timespec="seconds"), entity, record_id, json.dumps(data)))
            return dict(result, _version=version, _revision=revision)

    def create_order(self, db, data):
        allowed = {"id", "customer", "item_id", "quantity", "due", "priority", "note", "lot_size", "routing_id"}
        if set(data) - allowed:
            raise ValueError("Unknown order field")
        row = db.execute("SELECT data FROM records WHERE entity='items' AND id=?", (data.get("item_id"),)).fetchone()
        if not row:
            raise ValueError("Select an existing article")
        item = json.loads(row[0])
        lot_size = data.pop("lot_size", item["lot_size"])
        if isinstance(lot_size, bool) or not isinstance(lot_size, int) or not 1 <= lot_size <= 100:
            raise ValueError("Lot size must be a whole number between 1 and 100")
        data["status"] = "Released"
        data.update(good_quantity=0, scrap_quantity=0)
        data.setdefault("note", "")
        self.validate(db, "orders", data)
        if data["quantity"] > 2000:
            raise ValueError("Demo orders are limited to 2,000 pieces")
        start = max((int(x["id"][1:]) for x in self.all(db, "lots")), default=0) + 1
        factory = self.meta(db, "factory")
        if data["due"] < factory["as_of"]:
            raise ValueError("A new order must be due at or after the demo snapshot")
        lots, ops = workplans.release_order(db, data, item, lot_size, factory["as_of"], start)
        for entity, rows in [("lots", lots), ("operations", ops)]:
            for row in rows:
                self.insert(db, entity, row)
        return data

    def record_action(self, db, entity, ident, payload):
        self.put_meta(db, "revision", self.meta(db, "revision") + 1)
        db.execute("INSERT INTO audit(at,entity,record_id,detail) VALUES (?,?,?,?)",
                   (datetime.now().isoformat(timespec="seconds"), entity, ident, json.dumps(payload)))

    def check_version(self, db, entity, ident, payload):
        row = db.execute("SELECT version FROM records WHERE entity=? AND id=?", (entity, ident)).fetchone()
        if row is None:
            raise ValueError("Record not found")
        if payload.get("expected_version") != row[0]:
            raise Conflict("This record changed. Reload it before saving.")

    def report(self, operation_id, payload):
        with self.connect() as db:
            db.execute("BEGIN IMMEDIATE")
            self.check_version(db, "operations", operation_id, payload)
            result = execution.report(self, db, operation_id, payload)
            self.record_action(db, "operations", operation_id, payload)
            return result

    def routing_action(self, ident, action, payload):
        with self.connect() as db:
            db.execute("BEGIN IMMEDIATE")
            self.check_version(db, "routings", ident, payload)
            route = workplans.get(db, "routings", ident)
            if action == "revise":
                result = workplans.revise(self, db, route, payload)
            elif action == "release":
                if route["status"] != "Draft":
                    raise ValueError("Only draft workplans can be released")
                workplans.validate_route(db, ident)
                route["status"] = "Released"
                workplans.update(db, "routings", route)
                result = route
            else:
                raise ValueError("Unknown workplan action")
            self.record_action(db, "routings", ident, dict(action=action, **payload))
            return result

    def advance_clock(self, payload):
        raise Conflict("The demo snapshot is fixed. Its date and time cannot be changed.")
