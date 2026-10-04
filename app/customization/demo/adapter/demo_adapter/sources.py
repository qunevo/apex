"""Revision-checked public MES extraction and immutable workbook capture."""
import hashlib
import json
from pathlib import Path
from urllib.parse import urlparse
from urllib.request import urlopen

from .common import ImportFailure, indexed, require

TABLES = ("orders", "lots", "operations", "machines", "personnel", "shifts",
          "absences", "materials", "receipts", "downtime", "material_issues", "confirmations")


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, separators=(",", ":"),
                                    allow_nan=False).encode()).hexdigest()


def get_json(url):
    with urlopen(url, timeout=30) as response:
        return json.load(response)


def capture(mes_url, workbook, *, fetch=get_json, attempts=3):
    """Retry the entire extraction if MES revision or saved workbook changes."""
    parsed = urlparse(mes_url)
    require(parsed.scheme in ("http", "https") and parsed.hostname and not parsed.username
            and not parsed.password and not parsed.query and not parsed.fragment,
            "SOURCE_URL", "mes_url", "Expected an HTTP(S) base URL without credentials or query")
    base = mes_url.rstrip("/")
    path = Path(workbook)
    for _ in range(attempts):
        before = fetch(base + "/api/meta")
        require(isinstance(before.get("revision"), int), "MES_SCHEMA", "meta", "Missing MES revision")
        workbook_bytes = path.read_bytes()
        records = {}
        consistent = True
        for entity in TABLES:
            require(entity in before.get("catalog", {}), "MES_SCHEMA", entity, "Required MES table is missing")
            rows, offset = [], 0
            expected = before["counts"][entity]
            while offset < expected:
                page = fetch(f"{base}/api/tables/{entity}?limit=200&offset={offset}&sort=id")
                batch = page.get("rows")
                require(isinstance(batch, list), "MES_SCHEMA", entity, "Expected paged records")
                if page.get("total") != expected or not batch:
                    consistent = False
                    break
                rows.extend(batch)
                offset += len(batch)
            if len(rows) != expected:
                consistent = False
            records[entity] = rows
        after = fetch(base + "/api/meta")
        if (consistent and before == after and path.read_bytes() == workbook_bytes):
            for entity, rows in records.items():
                indexed(rows, entity)
            snapshot = dict(factory=before["factory"], revision=before["revision"], records=records)
            provenance = dict(mes_revision=before["revision"], mes_sha256=digest(snapshot),
                              workbook_sha256=hashlib.sha256(workbook_bytes).hexdigest(),
                              workbook_name=path.name)
            return snapshot, workbook_bytes, provenance
    raise ImportFailure("SOURCE_CHANGED", "sources", "MES or workbook changed during extraction; retry after saving")
