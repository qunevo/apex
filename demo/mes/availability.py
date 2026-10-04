"""Equipment availability from explicit exceptions in local plant time."""


def normalize_machine(row):
    """Preserve legacy global blocks without inventing an end date."""
    row = dict(row)
    old_status = row.pop("status", "Available")
    row.setdefault("permanently_unavailable", old_status != "Available")
    return row


def availability_at(machine, periods, at):
    """Return status and the active/next continuous block; intervals are [start, end)."""
    if machine["permanently_unavailable"]:
        return dict(status="Unavailable", unavailable_from="", unavailable_until="",
                    unavailability_reason="Permanently unavailable")
    spans = sorted((p for p in periods if p["resource_id"] == machine["id"]
                    and not p.get("cancelled", False) and p["end"] > at), key=lambda p: p["start"])
    if not spans:
        return dict(status="Available", unavailable_from="", unavailable_until="", unavailability_reason="")
    start, end = spans[0]["start"], spans[0]["end"]
    reasons = [spans[0]["reason"]]
    for period in spans[1:]:
        if period["start"] > end:
            break
        end = max(end, period["end"])
        if period["reason"] not in reasons:
            reasons.append(period["reason"])
    return dict(status="Unavailable" if start <= at else "Available", unavailable_from=start,
                unavailable_until=end, unavailability_reason="; ".join(reasons))
