//! An injected clock.
//!
//! Not optional, per the v1 spec: FSRS is time-dependent and Trigger expiry is a deadline, so
//! behaviour tested against the real clock is non-deterministic. Everything that asks "what time
//! is it" goes through here.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Duration, Utc};

pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// The real clock. Used by the host; never by tests.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// A clock tests advance explicitly. Time never moves on its own.
pub struct TestClock {
    now: Mutex<DateTime<Utc>>,
}

impl TestClock {
    pub fn new(start: DateTime<Utc>) -> Arc<Self> {
        Arc::new(Self {
            now: Mutex::new(start),
        })
    }

    pub fn advance(&self, by: Duration) {
        let mut now = self.now.lock().expect("clock poisoned");
        *now += by;
    }
}

impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().expect("clock poisoned")
    }
}
