//! In-memory pairing-code state.
//!
//! Pairing codes are intentionally not persisted: they are short-lived
//! single-use secrets, and recovering from a crash mid-pairing means
//! the user generates a new code rather than us replaying a stale one.
//!
//! Format: 6 alphanumeric chars, drawn from an unambiguous alphabet
//! (`23456789ABCDEFGHJKLMNPQRSTUVWXYZ` — no 0/O, 1/I, etc.). Easy to
//! read off a screen on mobile.

use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};
use rand::Rng;

/// Alphabet picked to be unambiguous on small mobile fonts.
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// Length in characters of a generated pairing code.
pub const CODE_LENGTH: usize = 6;

/// Default time window during which a generated code is valid.
pub const DEFAULT_EXPIRY: Duration = Duration::minutes(10);

/// Public representation of a generated code, returned to the
/// dashboard so it can be rendered and copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCode {
    pub code: String,
    pub expires_at: DateTime<Utc>,
}

/// In-memory pairing slot. Mutex<Option<...>> rather than RwLock —
/// the code is touched twice per pairing (start + consume) and never
/// in hot loops, so contention is a non-issue.
#[derive(Debug, Default)]
pub struct PairingState {
    pending: Mutex<Option<PendingPairing>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingPairing {
    code: String,
    expires_at: DateTime<Utc>,
}

/// Outcome of [`PairingState::try_consume`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TryConsume {
    /// The slot was empty; the user has not generated a code yet (or
    /// it was already consumed).
    NoPending,
    /// A code was waiting but its expiry was in the past.
    Expired,
    /// A code was waiting and not expired but the supplied string
    /// did not match.
    Mismatch,
    /// The supplied code matched. The slot is now empty (single-use).
    Matched,
}

impl PairingState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a new pending pairing. Replaces any previous code without
    /// asking — the user clicking "Generate code" twice in a row
    /// should always get a fresh code.
    pub fn start(&self, code: &str, expires_at: DateTime<Utc>) {
        let mut guard = self
            .pending
            .lock()
            .expect("pairing-state mutex poisoned");
        *guard = Some(PendingPairing {
            code: code.to_string(),
            expires_at,
        });
    }

    /// Drop any pending pairing. Used by the "Cancel" button and
    /// after a successful pair (cleanup is also done inline in
    /// [`try_consume`]).
    pub fn clear(&self) {
        let mut guard = self
            .pending
            .lock()
            .expect("pairing-state mutex poisoned");
        *guard = None;
    }

    /// What's currently pending, for display in the dashboard.
    /// Returns `None` when nothing is pending or the pending code
    /// has expired (we evict eagerly on read so the UI stays clean).
    pub fn snapshot(&self, now: DateTime<Utc>) -> Option<PairingCode> {
        let mut guard = self
            .pending
            .lock()
            .expect("pairing-state mutex poisoned");
        match guard.as_ref() {
            Some(p) if p.expires_at > now => Some(PairingCode {
                code: p.code.clone(),
                expires_at: p.expires_at,
            }),
            Some(_) => {
                *guard = None;
                None
            }
            None => None,
        }
    }

    /// Try to consume `code`. On `Matched` the slot is cleared
    /// before the result is returned, enforcing single-use.
    pub fn try_consume(&self, code: &str, now: DateTime<Utc>) -> TryConsume {
        let mut guard = self
            .pending
            .lock()
            .expect("pairing-state mutex poisoned");
        let Some(pending) = guard.as_ref() else {
            return TryConsume::NoPending;
        };
        if pending.expires_at <= now {
            *guard = None;
            return TryConsume::Expired;
        }
        if pending.code != code {
            return TryConsume::Mismatch;
        }
        *guard = None;
        TryConsume::Matched
    }
}

/// Generate a fresh pairing code using the unambiguous alphabet.
pub fn generate_pairing_code() -> String {
    let mut rng = rand::rng();
    (0..CODE_LENGTH)
        .map(|_| {
            let idx = rng.random_range(0..CODE_ALPHABET.len());
            CODE_ALPHABET[idx] as char
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(secs: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 15, 0, 0).unwrap() + Duration::seconds(secs)
    }

    #[test]
    fn generated_codes_have_correct_length_and_alphabet() {
        for _ in 0..50 {
            let code = generate_pairing_code();
            assert_eq!(code.len(), CODE_LENGTH);
            for ch in code.chars() {
                assert!(
                    CODE_ALPHABET.contains(&(ch as u8)),
                    "char {ch:?} is not in the unambiguous alphabet"
                );
            }
        }
    }

    #[test]
    fn generated_codes_vary() {
        // Not a perfect randomness test — just a smoke check that
        // we are not generating the same code every time.
        let a = generate_pairing_code();
        let b = generate_pairing_code();
        let c = generate_pairing_code();
        assert!(a != b || b != c, "generator is suspiciously deterministic");
    }

    #[test]
    fn empty_state_returns_no_pending() {
        let s = PairingState::new();
        assert_eq!(s.try_consume("ABCDEF", t(0)), TryConsume::NoPending);
        assert!(s.snapshot(t(0)).is_none());
    }

    #[test]
    fn matched_code_clears_slot_and_is_single_use() {
        let s = PairingState::new();
        s.start("ABCDEF", t(60));
        assert_eq!(s.try_consume("ABCDEF", t(0)), TryConsume::Matched);
        assert_eq!(s.try_consume("ABCDEF", t(0)), TryConsume::NoPending);
    }

    #[test]
    fn expired_code_is_evicted() {
        let s = PairingState::new();
        s.start("ABCDEF", t(60));
        assert_eq!(s.try_consume("ABCDEF", t(120)), TryConsume::Expired);
        assert_eq!(s.try_consume("ABCDEF", t(120)), TryConsume::NoPending);
    }

    #[test]
    fn mismatched_code_does_not_evict() {
        let s = PairingState::new();
        s.start("ABCDEF", t(60));
        assert_eq!(s.try_consume("WRONG1", t(0)), TryConsume::Mismatch);
        // The correct code still works on the next attempt.
        assert_eq!(s.try_consume("ABCDEF", t(0)), TryConsume::Matched);
    }

    #[test]
    fn start_twice_replaces_previous_code() {
        let s = PairingState::new();
        s.start("AAAAAA", t(60));
        s.start("BBBBBB", t(60));
        assert_eq!(s.try_consume("AAAAAA", t(0)), TryConsume::Mismatch);
        assert_eq!(s.try_consume("BBBBBB", t(0)), TryConsume::Matched);
    }

    #[test]
    fn clear_drops_pending() {
        let s = PairingState::new();
        s.start("ABCDEF", t(60));
        s.clear();
        assert_eq!(s.try_consume("ABCDEF", t(0)), TryConsume::NoPending);
    }

    #[test]
    fn snapshot_evicts_expired_codes() {
        let s = PairingState::new();
        s.start("ABCDEF", t(60));
        assert!(s.snapshot(t(0)).is_some());
        assert!(s.snapshot(t(120)).is_none());
        // And the slot is empty now even at an earlier time.
        assert!(s.snapshot(t(0)).is_none());
    }
}
