//! Per-user token-bucket rate limiter for Telegram command traffic.
//!
//! Build-plan §11.5 requires that commands be rate limited. The alpha
//! ships a per-user-id in-memory bucket. Defaults: **burst 10, refill
//! 30/min** (0.5 tokens / second). One token == one command; pairing is
//! intentionally not rate-limited (a user typing the wrong code five
//! times should not get locked out).
//!
//! The limiter is intentionally simple:
//!
//! - Token counts use `f64` so we can refill fractionally on every
//!   check rather than snapping to integer ticks.
//! - State lives in a single `Mutex<HashMap>`. Telegram message rates
//!   per user are tiny (single digits per minute at the very most),
//!   so the lock is never contended in practice.
//! - Buckets accumulate forever. A real implementation would evict
//!   idle users; for the alpha allowlist (≤10 users) the memory
//!   footprint is negligible.

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Utc};

/// Default token capacity (burst).
pub const DEFAULT_CAPACITY: u32 = 10;

/// Default refill rate per minute.
pub const DEFAULT_REFILL_PER_MIN: u32 = 30;

/// Outcome of [`RateLimiter::check`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitOutcome {
    /// One token consumed; the caller may proceed.
    Allowed,
    /// No tokens available; bucket will hold one in `retry_after_secs`.
    /// The value is rounded up to at least 1 second so the user gets
    /// a usable hint even when the wait is sub-second.
    Throttled { retry_after_secs: u32 },
}

/// Per-(telegram user id) token bucket.
pub struct RateLimiter {
    capacity: f64,
    refill_per_sec: f64,
    state: Mutex<HashMap<i64, BucketState>>,
}

#[derive(Debug, Clone, Copy)]
struct BucketState {
    tokens: f64,
    last_refill: DateTime<Utc>,
}

impl RateLimiter {
    /// Construct a limiter with explicit limits. `capacity` is the
    /// burst size; `refill_per_min` is the steady-state rate.
    pub fn new(capacity: u32, refill_per_min: u32) -> Self {
        assert!(capacity >= 1, "rate limiter capacity must be at least 1");
        assert!(refill_per_min >= 1, "rate limiter refill must be at least 1/min");
        Self {
            capacity: capacity as f64,
            refill_per_sec: refill_per_min as f64 / 60.0,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Default-tuned limiter: `(DEFAULT_CAPACITY, DEFAULT_REFILL_PER_MIN)`.
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_CAPACITY, DEFAULT_REFILL_PER_MIN)
    }

    /// Spend one token for `user_id` if available; otherwise report
    /// how long until the next token arrives.
    pub fn check(&self, user_id: i64, now: DateTime<Utc>) -> RateLimitOutcome {
        let mut state = self.state.lock().expect("rate limiter mutex poisoned");
        let entry = state.entry(user_id).or_insert(BucketState {
            tokens: self.capacity,
            last_refill: now,
        });

        // Refill since last visit. We cap at `capacity` (no overflow
        // savings for idle users).
        let elapsed_secs = (now - entry.last_refill).num_milliseconds() as f64 / 1000.0;
        if elapsed_secs > 0.0 {
            entry.tokens = (entry.tokens + elapsed_secs * self.refill_per_sec).min(self.capacity);
            entry.last_refill = now;
        }

        if entry.tokens >= 1.0 {
            entry.tokens -= 1.0;
            RateLimitOutcome::Allowed
        } else {
            let needed = 1.0 - entry.tokens;
            let wait_secs = (needed / self.refill_per_sec).ceil() as u32;
            RateLimitOutcome::Throttled {
                retry_after_secs: wait_secs.max(1),
            }
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, TimeZone};

    fn t(secs: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 16, 0, 0).unwrap() + Duration::seconds(secs)
    }

    #[test]
    fn fresh_bucket_allows_burst_then_throttles() {
        let r = RateLimiter::new(3, 60); // 1 token/sec, burst 3.
        let user = 1;
        for _ in 0..3 {
            assert_eq!(r.check(user, t(0)), RateLimitOutcome::Allowed);
        }
        match r.check(user, t(0)) {
            RateLimitOutcome::Throttled { retry_after_secs } => {
                assert!(retry_after_secs >= 1);
            }
            other => panic!("expected throttle, got {other:?}"),
        }
    }

    #[test]
    fn refill_returns_tokens_after_waiting() {
        let r = RateLimiter::new(2, 60); // 1 token/sec, burst 2.
        let user = 1;
        assert_eq!(r.check(user, t(0)), RateLimitOutcome::Allowed);
        assert_eq!(r.check(user, t(0)), RateLimitOutcome::Allowed);
        assert!(matches!(r.check(user, t(0)), RateLimitOutcome::Throttled { .. }));
        // Wait 2 seconds → 2 tokens refilled, capped at capacity.
        assert_eq!(r.check(user, t(2)), RateLimitOutcome::Allowed);
        assert_eq!(r.check(user, t(2)), RateLimitOutcome::Allowed);
    }

    #[test]
    fn buckets_are_per_user() {
        let r = RateLimiter::new(1, 60);
        assert_eq!(r.check(1, t(0)), RateLimitOutcome::Allowed);
        // User 1 exhausted, user 2 still has its token.
        assert!(matches!(r.check(1, t(0)), RateLimitOutcome::Throttled { .. }));
        assert_eq!(r.check(2, t(0)), RateLimitOutcome::Allowed);
    }

    #[test]
    fn long_idle_does_not_overfill_above_capacity() {
        let r = RateLimiter::new(2, 60);
        let user = 1;
        assert_eq!(r.check(user, t(0)), RateLimitOutcome::Allowed);
        assert_eq!(r.check(user, t(0)), RateLimitOutcome::Allowed);
        // Idle for an hour. Capacity caps to 2, not 60.
        assert_eq!(r.check(user, t(3600)), RateLimitOutcome::Allowed);
        assert_eq!(r.check(user, t(3600)), RateLimitOutcome::Allowed);
        assert!(matches!(
            r.check(user, t(3600)),
            RateLimitOutcome::Throttled { .. }
        ));
    }

    #[test]
    fn retry_after_secs_is_at_least_one() {
        let r = RateLimiter::new(1, 60);
        let user = 1;
        assert_eq!(r.check(user, t(0)), RateLimitOutcome::Allowed);
        let RateLimitOutcome::Throttled { retry_after_secs } = r.check(user, t(0)) else {
            panic!("expected throttle");
        };
        assert!(retry_after_secs >= 1);
    }

    #[test]
    fn defaults_pass_smoke_check() {
        let r = RateLimiter::with_defaults();
        for _ in 0..DEFAULT_CAPACITY {
            assert_eq!(r.check(7, t(0)), RateLimitOutcome::Allowed);
        }
        assert!(matches!(
            r.check(7, t(0)),
            RateLimitOutcome::Throttled { .. }
        ));
    }
}
