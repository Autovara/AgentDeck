use chrono::{DateTime, Utc};

/// Source of wall-clock time used by all of AgentDeck.
///
/// Adapters and the monitor must take time from this trait, never from
/// `std::time::SystemTime::now()` or `chrono::Utc::now()` directly. The
/// indirection is what makes the dev-time replay harness deterministic.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// A clock backed by the operating system's wall clock.
///
/// Use this in production. Use a [`Clock`] implementation from the
/// harness in tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}
