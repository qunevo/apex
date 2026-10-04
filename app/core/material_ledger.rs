//! Construction-time stock reservations; independent validation rebuilds its own timeline.
use crate::model::{Diagnostic, Problem, Time};
use std::collections::BTreeMap;

pub(crate) struct Ledger {
    opening: BTreeMap<String, f64>,
    events: BTreeMap<String, BTreeMap<Time, f64>>,
}

impl Ledger {
    pub(crate) fn new(problem: &Problem) -> Self {
        let mut ledger = Self {
            opening: problem.inventory.clone(),
            events: BTreeMap::new(),
        };
        for receipt in &problem.receipts {
            ledger.book(&receipt.item, receipt.at, receipt.amount);
        }
        ledger
    }

    pub(crate) fn book(&mut self, item: &str, at: Time, amount: f64) {
        *self
            .events
            .entry(item.into())
            .or_default()
            .entry(at)
            .or_default() += amount;
    }

    /// Find a start whose consumption preserves all previously reserved future work.
    /// Availability from this point onward must cover the additional consumption.
    pub(crate) fn earliest(
        &self,
        task: &str,
        consume: &BTreeMap<String, f64>,
        mut start: Time,
    ) -> Result<Time, Diagnostic> {
        for (item, need) in consume {
            let mut balance = *self.opening.get(item).unwrap_or(&0.0);
            let mut insufficient = balance + 1e-8 < *need;
            if let Some(events) = self.events.get(item) {
                for (&at, &amount) in events {
                    if insufficient {
                        start = start.max(at);
                    }
                    balance += amount;
                    insufficient = balance + 1e-8 < *need;
                }
            }
            if insufficient {
                return Err(Diagnostic::new(
                    "MATERIAL_SHORTAGE",
                    task,
                    format!("Insufficient {item}"),
                ));
            }
        }
        Ok(start)
    }
}
