"""Transactional demo state with explicit validation and optimistic revisions."""
from datetime import datetime
from contextlib import contextmanager
import json
import re
import sqlite3

from .catalog import CATALOG, ROUTES
from .seed import expand_order, refresh_progress


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
        return [dict(json.loads(row["data"]), _version=row["version"])
                for row in db.execute("SELECT data,version FROM records WHERE entity=? ORDER BY id", (entity,))]

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

    def seed(self, data, reset=False):
        with self.connect() as db:
            db.execute("BEGIN IMMEDIATE")
            if self.meta(db, "factory") and not reset:
                return False
            db.execute("DELETE FROM records")
            db.execute("DELETE FROM metadata")
            db.execute("DELETE FROM audit")
            for entity, rows in data["records"].items():
                for row in rows:
                    self.insert(db, entity, row)
            for key in ["factory", "qualifications", "plan"]:
                self.put_meta(db, key, data[key])
            self.put_meta(db, "revision", 0)
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
            if col["type"] in ("number", "integer"):
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
            raise ValueError("ID must contain 1–50 letters, numbers, dots, hyphens or underscores")
        if entity == "downtime" and data["end"] <= data["start"]:
            raise ValueError("The end must be after the start")
        if entity == "shifts" and not data["start"] < data["end"]:
            raise ValueError("Use a same-day shift with end after start")
        if entity == "shifts" and not data["start"] <= data["break_start"] <= data["break_end"] <= data["end"]:
            raise ValueError("The break must be inside the shift")
        if entity == "items" and {"Distributor": "RT-D", "Regulator": "RT-R", "Sensor": "RT-S"}[data["variant"]] != data["routing_id"]:
            raise ValueError("The workplan must match the product variant")

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
                result.update(data)
                self.validate(db, entity, result)
                db.execute("UPDATE records SET data=?,version=version+1 WHERE entity=? AND id=?", (json.dumps(result), entity, record_id))
                version = old["version"] + 1
            else:
                if not CATALOG[entity]["create"]:
                    raise ValueError("Create this record through its owning workflow")
                result = self.create_order(db, data) if entity == "orders" else data
                self.validate(db, entity, result)
                self.insert(db, entity, result)
                record_id, version = result["id"], 1
            revision = self.meta(db, "revision") + 1
            self.put_meta(db, "revision", revision)
            db.execute("INSERT INTO audit(at,entity,record_id,detail) VALUES (?,?,?,?)",
                       (datetime.now().isoformat(timespec="seconds"), entity, record_id, json.dumps(data)))
            return dict(result, _version=version, _revision=revision)

    def create_order(self, db, data):
        allowed = {"id", "customer", "item_id", "quantity", "due", "priority", "note", "lot_size"}
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
        data.setdefault("note", "")
        self.validate(db, "orders", data)
        if data["quantity"] > 2000:
            raise ValueError("Demo orders are limited to 2,000 pieces")
        start = max((int(x["id"][1:]) for x in self.all(db, "lots")), default=0) + 1
        factory = self.meta(db, "factory")
        if data["due"] < factory["as_of"]:
            raise ValueError("A new order must be due at or after the demo snapshot")
        lots, ops = expand_order(data, item, lot_size=lot_size, release=factory["as_of"], first_lot=start)
        for entity, rows in [("lots", lots), ("operations", ops)]:
            for row in rows:
                self.insert(db, entity, row)
        return data

    def report(self, operation_id, payload):
        with self.connect() as db:
            db.execute("BEGIN IMMEDIATE")
            records = {entity: self.all(db, entity) for entity in ("operations", "lots", "orders")}
            operation = next((op for op in records["operations"] if op["id"] == operation_id), None)
            if operation is None:
                raise ValueError("Operation not found")
            if payload.get("expected_version") != operation["_version"]:
                raise Conflict("This operation changed. Reload it before booking progress.")
            lot = next(lot for lot in records["lots"] if lot["id"] == operation["lot_id"])
            qty = payload.get("completed_quantity")
            if isinstance(qty, bool) or not isinstance(qty, int) or not operation["completed_quantity"] <= qty <= lot["quantity"]:
                raise ValueError("Good quantity must be a whole number between the booked and lot quantities")
            if any(op["lot_id"] == lot["id"] and op["sequence"] < operation["sequence"] and op["status"] != "Complete" for op in records["operations"]):
                raise ValueError("Complete the previous operation first")
            from .seed import eligible
            machine = next((m for m in self.all(db, "machines") if m["id"] == payload.get("resource_id")), None)
            item = next(a for a in self.all(db, "items") if a["id"] == lot["item_id"])
            if machine is None or not eligible(machine, item, operation) or machine["status"] != "Available":
                raise ValueError("Select an available, eligible resource")
            operation.update(completed_quantity=qty, status="Complete" if qty == lot["quantity"] else "Running", resource_id=machine["id"])
            refresh_progress(records)
            for entity, rows in records.items():
                for record in rows:
                    version = record.pop("_version")
                    encoded = json.dumps(record)
                    old = db.execute("SELECT data FROM records WHERE entity=? AND id=?", (entity, record["id"])).fetchone()[0]
                    if old != encoded:
                        db.execute("UPDATE records SET data=?,version=? WHERE entity=? AND id=?", (encoded, version+1, entity, record["id"]))
            self.put_meta(db, "revision", self.meta(db, "revision")+1)
            db.execute("INSERT INTO audit(at,entity,record_id,detail) VALUES (?,?,?,?)",
                       (datetime.now().isoformat(timespec="seconds"), "operations", operation_id, json.dumps(payload)))
            return {"saved": True}
