use std::sync::Mutex;

use agentdeck_core::Clock;
use chrono::{DateTime, Duration, Utc};

/// A clock whose time can be advanced explicitly.
///
/// Use this in adapter tests and inside the replay engine to step
/// deterministically across fixture timestamps.
#[derive(Debug)]
pub struct MockClock {
    inner: Mutex<DateTime<Utc>>,
}

impl MockClock {
    pub fn new(start: DateTime<Utc>) -> Self {
        Self {
            inner: Mutex::new(start),
        }
    }

    pub fn advance_ms(&self, ms: u64) {
        let mut guard = self.inner.lock().expect("clock mutex poisoned");
        *guard += Duration::milliseconds(ms as i64);
    }

    pub fn set(&self, t: DateTime<Utc>) {
        let mut guard = self.inner.lock().expect("clock mutex poisoned");
        *guard = t;
    }
}

impl Clock for MockClock {
    fn now(&self) -> DateTime<Utc> {
        *self.inner.lock().expect("clock mutex poisoned")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn advance_moves_now_forward() {
        let t0 = Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap();
        let clock = MockClock::new(t0);
        assert_eq!(clock.now(), t0);

        clock.advance_ms(1500);
        let expected = t0 + Duration::milliseconds(1500);
        assert_eq!(clock.now(), expected);
    }

    #[test]
    fn set_overrides_current_time() {
        let t0 = Utc.with_ymd_and_hms(2026, 5, 21, 12, 0, 0).unwrap();
        let t1 = Utc.with_ymd_and_hms(2027, 1, 1, 0, 0, 0).unwrap();
        let clock = MockClock::new(t0);
        clock.set(t1);
        assert_eq!(clock.now(), t1);
    }
}
