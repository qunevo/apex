"""Typed column filters shared by pagination, facets and CSV exports."""
from datetime import date
import json
import math

from .catalog import CATALOG


def filter_rows(entity, rows, query):
    columns = {c["key"]: c for c in CATALOG[entity]["columns"] if c["type"] != "json"}
    exact = query.get("filter_field")
    if exact:
        if exact not in columns:
            raise ValueError("Unknown filter field")
        rows = [r for r in rows if str(r.get(exact, "")) == query.get("filter_value", "")]
    # Facets span the entire drill-down scope, not just the visible page.
    facets = {key: sorted({str(r.get(key, "")) for r in rows}) for key, col in columns.items()
              if col["type"] not in ("integer", "number", "datetime")}
    try:
        filters = json.loads(query.get("filters", "{}"))
    except (ValueError, TypeError) as error:
        raise ValueError("Invalid column filters") from error
    if not isinstance(filters, dict) or len(filters) > len(columns):
        raise ValueError("Invalid column filters")
    for key, rule in filters.items():
        if key not in columns or not isinstance(rule, dict):
            raise ValueError("Unknown filter field")
        kind = columns[key]["type"]
        allowed = {"min", "max"} if kind in ("number", "integer", "datetime") else {"contains", "values"}
        if set(rule) - allowed:
            raise ValueError("Unsupported column filter")
        if "values" in rule:
            values = rule["values"]
            if not isinstance(values, list) or len(values) > 10000 or any(not isinstance(v, str) for v in values):
                raise ValueError("Filter selections must be a list of text values")
            if values:
                selected = set(values)
                rows = [r for r in rows if str(r.get(key, "")) in selected]
        if "contains" in rule:
            if not isinstance(rule["contains"], str):
                raise ValueError("Filter search must be text")
            needle = rule["contains"].casefold()
            rows = [r for r in rows if needle in str(r.get(key, "")).casefold()]
        bounds = {}
        for bound in ("min", "max"):
            if bound not in rule or rule[bound] == "":
                continue
            value = rule[bound]
            if kind == "datetime":
                try:
                    if not isinstance(value, str) or date.fromisoformat(value).isoformat() != value:
                        raise ValueError()
                except (ValueError, TypeError) as error:
                    raise ValueError("Date filters must use YYYY-MM-DD") from error
            elif isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value):
                raise ValueError("Quantity filters must be finite numbers")
            bounds[bound] = value
        if len(bounds) == 2 and bounds["min"] > bounds["max"]:
            raise ValueError("Filter end must not be before its start")
        for bound, value in bounds.items():
            def matches(row):
                actual = row.get(key)
                if actual is None or actual == "":
                    return False
                actual = actual[:10] if kind == "datetime" else actual
                return actual >= value if bound == "min" else actual <= value
            rows = [r for r in rows if matches(r)]
    search = query.get("q", "").casefold()
    if search:
        rows = [r for r in rows if any(search in str(v).casefold() for k, v in r.items() if not k.startswith("_"))]
    return rows, facets
