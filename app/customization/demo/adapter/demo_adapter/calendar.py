"""Normalize local plant time and derive finite resource calendars."""
from datetime import datetime, timedelta, timezone
from zoneinfo import ZoneInfo

from .common import require


class Clock:
    def __init__(self, factory, horizon_end=None):
        self.zone = ZoneInfo(factory["timezone"])
        self.epoch = self.date(factory["plan_start"])
        self.as_of = self.seconds(factory["as_of"])
        self.end = self.date(horizon_end or factory["horizon_end"])
        self.horizon = self.seconds(self.end)
        require(0 <= self.as_of < self.horizon, "HORIZON", "factory",
                "Planning horizon must extend beyond the factory snapshot; supply --horizon-end if needed")

    def date(self, value):
        if isinstance(value, str):
            value = datetime.fromisoformat(value)
        require(isinstance(value, datetime), "DATETIME", str(value), "Expected a date and time")
        if value.tzinfo is not None:
            return value.astimezone(self.zone)
        first, second = (value.replace(tzinfo=self.zone, fold=n) for n in (0, 1))
        require(first.utcoffset() == second.utcoffset(), "LOCAL_TIME", value.isoformat(),
                "Ambiguous or nonexistent local time; provide an explicit UTC offset")
        require(first.astimezone(timezone.utc).astimezone(self.zone).replace(tzinfo=None) == value,
                "LOCAL_TIME", value.isoformat(), "Nonexistent local time")
        return first

    def seconds(self, value):
        return round((self.date(value).astimezone(timezone.utc) - self.epoch.astimezone(timezone.utc)).total_seconds())

    def windows(self, shift, exceptions):
        spans = []
        day = self.epoch.date()
        while day <= self.end.date():
            if day.weekday() < 5:
                points = [self.seconds(f"{day}T{shift[key]}") for key in ("start", "break_start", "break_end", "end")]
                require(points == sorted(points), "SHIFT", shift["id"], "Expected same-day shift and ordered break")
                for start, end in ((points[0], points[1]), (points[2], points[3])):
                    if max(0, start) < min(end, self.horizon):
                        spans.append((max(0, start), min(end, self.horizon)))
            day += timedelta(days=1)
        for period in exceptions:
            if period.get("cancelled", False):
                continue
            start, end = self.seconds(period["start"]), self.seconds(period["end"])
            require(start < end, "INTERVAL", period["id"], "Exception must have positive duration")
            spans = [(a, b) for left, right in spans
                     for a, b in ((left, min(right, start)), (max(left, end), right)) if a < b]
        return [dict(start=a, end=b) for a, b in spans]
