use crate::{compile::Compiled, model::*};
use std::collections::BTreeMap;
use std::ops::Bound::{Excluded, Unbounded};

/// Piecewise occupancy: predecessor lookup and range updates avoid scanning all past work.
#[derive(Default)]
pub struct Book {
    pub profiles: Vec<BTreeMap<Time, f64>>,
}
impl Book {
    pub fn new(n: usize) -> Self {
        Self {
            profiles: vec![BTreeMap::from([(0, 0.0)]); n],
        }
    }
    pub fn used(&self, r: usize, t: Time) -> f64 {
        self.profiles[r]
            .range(..=t)
            .next_back()
            .map_or(0.0, |(_, v)| *v)
    }
    pub fn next(&self, r: usize, t: Time, horizon: Time) -> Time {
        self.profiles[r]
            .range((Excluded(t), Unbounded))
            .next()
            .map_or(horizon, |(&k, _)| k)
    }
    pub fn add(&mut self, r: usize, start: Time, end: Time, amount: f64) {
        if end <= start {
            return;
        }
        let a = self.used(r, start);
        let b = self.used(r, end);
        self.profiles[r].entry(start).or_insert(a);
        self.profiles[r].entry(end).or_insert(b);
        for (_, v) in self.profiles[r].range_mut(start..end) {
            *v += amount;
        }
    }
    pub fn reservations(&mut self, c: &Compiled<'_>, rs: &[Reservation], sign: f64) {
        for r in rs {
            self.add(
                c.resources[r.resource.as_str()],
                r.start,
                r.end,
                sign * r.amount,
            );
        }
    }
}
pub fn window_at(windows: &[Window], t: Time) -> Option<&Window> {
    let i = windows.partition_point(|w| w.start <= t);
    i.checked_sub(1)
        .and_then(|i| windows.get(i))
        .filter(|w| t < w.end)
}
pub fn boundary(windows: &[Window], t: Time, horizon: Time) -> Time {
    if let Some(w) = window_at(windows, t) {
        w.end
    } else {
        windows
            .iter()
            .find(|w| w.start > t)
            .map_or(horizon, |w| w.start)
    }
}
pub fn hold_capacity(r: &Resource, t: Time) -> f64 {
    if r.retention_calendar.is_empty() {
        r.capacity
    } else {
        window_at(&r.retention_calendar, t).map_or(0.0, |w| w.capacity)
    }
}

pub fn fits(c: &Compiled<'_>, book: &Book, r: &Reservation) -> bool {
    let id = c.resources[r.resource.as_str()];
    let resource = &c.problem.resources[id];
    let mut t = r.start;
    while t < r.end {
        if book.used(id, t) + r.amount > hold_capacity(resource, t) + 1e-8 {
            return false;
        }
        t = book
            .next(id, t, r.end)
            .min(boundary(&resource.retention_calendar, t, r.end))
            .min(r.end);
    }
    true
}

fn phase(
    c: &Compiled<'_>,
    book: &Book,
    ph: &Phase,
    earliest: Time,
) -> Option<(Vec<Segment>, Vec<Reservation>, Time)> {
    let horizon = c.problem.horizon;
    let work = ph.work?;
    if work == 0.0 {
        return Some((vec![], vec![], earliest));
    }
    let mut t = earliest;
    let mut remaining = work;
    let mut segments = Vec::new();
    while t < horizon {
        let mut next = horizon;
        let mut available = true;
        let mut occupied = false;
        let mut rate = 1.0;
        for req in &ph.requirements {
            let id = c.resources[req.resource.as_str()];
            let resource = &c.problem.resources[id];
            let w = window_at(&resource.calendar, t);
            next = next
                .min(boundary(&resource.calendar, t, horizon))
                .min(boundary(&resource.retention_calendar, t, horizon))
                .min(book.next(id, t, horizon));
            let capacity = w
                .map_or(0.0, |w| w.capacity)
                .min(hold_capacity(resource, t));
            if book.used(id, t) + req.amount > capacity + 1e-8 {
                available = false;
                if capacity >= req.amount {
                    occupied = true;
                }
            }
            if ph.rate_resource.as_ref() == Some(&req.resource) {
                rate = w.map_or(0.0, |w| w.rate);
            }
        }
        if available && rate > 0.0 {
            let duration = ((remaining / rate - 1e-10).ceil() as Time)
                .max(1)
                .min(next - t);
            let done = remaining.min(duration as f64 * rate);
            segments.push(Segment {
                phase: ph.id.clone(),
                start: t,
                end: t + duration,
                work: done,
            });
            remaining -= done;
            t += duration;
            if remaining < 1e-8 {
                let start = segments[0].start;
                let reservations: Vec<_> = ph
                    .requirements
                    .iter()
                    .flat_map(|req| {
                        let intervals: Vec<_> = if req.retain {
                            vec![(start, t)]
                        } else {
                            segments.iter().map(|s| (s.start, s.end)).collect()
                        };
                        intervals.into_iter().map(|(start, end)| Reservation {
                            resource: req.resource.clone(),
                            start,
                            end,
                            amount: req.amount,
                        })
                    })
                    .collect();
                if reservations.iter().all(|r| fits(c, book, r)) {
                    return Some((segments, reservations, t));
                }
                // A retained resource is blocked inside the pause. Restart after this attempt.
                remaining = work;
                segments.clear();
            }
        } else {
            if occupied || ph.interruption == Interrupt::NonInterruptible {
                remaining = work;
                segments.clear();
            }
            t = next;
        }
    }
    None
}

pub fn place(
    c: &Compiled<'_>,
    book: &mut Book,
    task: &str,
    id: &str,
    role: &str,
    mode: &Mode,
    earliest: Time,
) -> Option<Activity> {
    let mut requested = earliest;
    while requested < c.problem.horizon {
        let mut cursor = requested;
        let mut segments = Vec::new();
        let mut reservations = Vec::new();
        let mut retry = None;
        for ph in &mode.phases {
            let Some((ss, rs, end)) = phase(c, book, ph, cursor) else {
                book.reservations(c, &reservations, -1.0);
                return None;
            };
            if mode.contiguous
                && !segments.is_empty()
                && ss.first().is_some_and(|s| s.start > cursor)
            {
                retry = Some(requested + ss[0].start - cursor);
                break;
            }
            book.reservations(c, &rs, 1.0);
            reservations.extend(rs);
            segments.extend(ss);
            cursor = end;
        }
        book.reservations(c, &reservations, -1.0);
        if let Some(next) = retry {
            requested = next;
            continue;
        }
        let start = segments.first().map_or(requested, |s| s.start);
        let amount = mode.phases[0]
            .requirements
            .iter()
            .find(|r| r.resource == mode.primary)?
            .amount;
        reservations.retain(|r| r.resource != mode.primary);
        if cursor > start {
            reservations.push(Reservation {
                resource: mode.primary.clone(),
                start,
                end: cursor,
                amount,
            });
        }
        if !reservations.iter().all(|r| fits(c, book, r)) {
            return None;
        }
        return Some(Activity {
            id: id.into(),
            task: task.into(),
            role: role.into(),
            mode: mode.id.clone(),
            start,
            end: cursor,
            segments,
            reservations,
        });
    }
    None
}
