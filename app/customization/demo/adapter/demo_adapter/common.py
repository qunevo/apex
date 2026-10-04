"""Source diagnostics and strict scalar conversion."""
import math


class ImportFailure(ValueError):
    def __init__(self, code, location, message):
        self.diagnostic = dict(code=code, location=location, message=message)
        super().__init__(f"{code} at {location}: {message}")


def require(condition, code, location, message):
    if not condition:
        raise ImportFailure(code, location, message)


def number(value, location, *, positive=False, integer=False):
    require(not isinstance(value, bool) and isinstance(value, (int, float))
            and math.isfinite(value), "NUMBER", location, "Expected a finite number")
    require(value > 0 if positive else value >= 0, "NUMBER", location,
            "Expected a positive number" if positive else "Expected a nonnegative number")
    require(not integer or int(value) == value, "NUMBER", location, "Expected a whole number")
    return value


def identifier(value, location):
    require(isinstance(value, str) and bool(value.strip()), "ID", location, "Expected a nonempty text ID")
    return value.strip()


def indexed(rows, location, key="id"):
    result = {}
    for row in rows:
        ident = identifier(row.get(key), location)
        require(ident not in result, "DUPLICATE_ID", location, f"Duplicate ID: {ident}")
        result[ident] = row
    return result
