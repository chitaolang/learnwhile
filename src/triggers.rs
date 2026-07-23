//! The open-Trigger set (ADR-0005) and its expiry (ADR-0006).
//!
//! The developer is Waiting exactly while this set is non-empty.

use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};

use crate::frame::TriggerKey;

/// How the set's emptiness changed as a result of an operation. The Runtime surfaces a card on
/// `BecameWaiting` and clears it on `BecameIdle`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitingEdge {
    BecameWaiting,
    BecameIdle,
    Unchanged,
}

pub struct OpenTriggers {
    /// key -> when the Trigger opened. Expiry is measured from open and deliberately not
    /// refreshed (ADR-0006): with only open and close frames defined, there is no mid-turn
    /// traffic to refresh from.
    entries: HashMap<TriggerKey, DateTime<Utc>>,
    expiry: Duration,
}

impl OpenTriggers {
    pub fn new(expiry: Duration) -> Self {
        Self {
            entries: HashMap::new(),
            expiry,
        }
    }

    pub fn is_waiting(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert. A duplicate open for the same key is idempotent: it keeps the original opened-at,
    /// so re-firing a hook cannot extend a Trigger's life past its expiry.
    pub fn open(&mut self, key: TriggerKey, at: DateTime<Utc>) -> WaitingEdge {
        let was_waiting = self.is_waiting();
        self.entries.entry(key).or_insert(at);
        self.edge(was_waiting)
    }

    /// Remove. A close for an unknown key is ignored.
    pub fn close(&mut self, key: &TriggerKey) -> WaitingEdge {
        let was_waiting = self.is_waiting();
        self.entries.remove(key);
        self.edge(was_waiting)
    }

    /// Drop every entry older than the expiry. Driven by its own timer, never by frame arrival —
    /// a phantom open has no subsequent traffic to drain it (ADR-0006).
    pub fn sweep(&mut self, now: DateTime<Utc>) -> WaitingEdge {
        let was_waiting = self.is_waiting();
        let expiry = self.expiry;
        self.entries
            .retain(|_, opened_at| now.signed_duration_since(*opened_at) < expiry);
        self.edge(was_waiting)
    }

    fn edge(&self, was_waiting: bool) -> WaitingEdge {
        match (was_waiting, self.is_waiting()) {
            (false, true) => WaitingEdge::BecameWaiting,
            (true, false) => WaitingEdge::BecameIdle,
            _ => WaitingEdge::Unchanged,
        }
    }
}
