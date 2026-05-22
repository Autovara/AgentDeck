//! `/stop` dispatcher trait, outcome type, and the per-user
//! confirmation slot.
//!
//! Build-plan §11.4 requires that every `/stop` go through a
//! confirmation step before any signal is sent. The alpha flow:
//!
//! 1. User sends `/stop <id>`. The bot looks up the session, writes a
//!    `remote_commands` row with status `pending`, and stashes a
//!    [`PendingStop`] in [`StopConfirmationState`] keyed by the
//!    Telegram user id. The bot replies asking for `STOP <id>` within
//!    the expiry window.
//! 2. User sends `STOP <id>`. The bot consumes the pending slot,
//!    dispatches to [`StopDispatcher::stop`], updates `remote_commands`
//!    to `executed`, and replies with the outcome.
//!
//! The dispatcher itself is a trait so this crate stays OS-agnostic:
//! the Tauri shell provides a `PlatformStopDispatcher` that shells out
//! to `kill` / `taskkill`.

use std::collections::HashMap;
use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Time window during which a `/stop` pending confirmation is valid.
pub const STOP_CONFIRMATION_WINDOW: Duration = Duration::seconds(60);

/// Outcome reported by [`StopDispatcher::stop`].
///
/// The bot uses the variant tag to pick a reply formatter; the Tauri
/// shell uses it to write the right `audit_log` row and update the
/// `remote_commands` lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum StopOutcome {
    /// Signal was successfully delivered to the underlying process.
    /// `mechanism` records what we used (`"SIGTERM"`, `"taskkill"`).
    Success { mechanism: String },
    /// No session row matches the requested id.
    SessionNotFound,
    /// The session already transitioned to `completed` before the
    /// confirmation arrived. No signal was sent.
    AlreadyCompleted,
    /// The dispatcher cannot map a stop to a safe platform action
    /// (e.g. the session row has no PID). `reason` is shown to the
    /// user.
    Unsupported { reason: String },
    /// The platform call returned an error.
    Failed { reason: String },
}

impl StopOutcome {
    /// Was the signal actually delivered?
    pub fn is_success(&self) -> bool {
        matches!(self, StopOutcome::Success { .. })
    }

    /// Was the dispatcher unable to even attempt the stop?
    pub fn is_unsupported(&self) -> bool {
        matches!(
            self,
            StopOutcome::Unsupported { .. } | StopOutcome::AlreadyCompleted | StopOutcome::SessionNotFound
        )
    }
}

/// Per-user pending stop slot. Held in memory; never persisted.
/// Pairing `PairingState` chose `Mutex<Option<...>>` because it is
/// process-global. `/stop` is per Telegram user id, so we key the
/// hashmap by `i64`.
#[derive(Debug, Default)]
pub struct StopConfirmationState {
    pending: Mutex<HashMap<i64, PendingStop>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingStop {
    pub session_id: Uuid,
    /// The 6-char short id we surfaced in the confirmation prompt.
    /// Stored lowercase so comparison is case-insensitive.
    pub short_id: String,
    /// `remote_commands.id` so the dispatcher can update the right
    /// row on success/failure.
    pub command_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

/// Outcome of [`StopConfirmationState::try_consume`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TryConsumeStop {
    NoPending,
    Expired,
    Mismatch,
    /// Confirmation matched. The slot has been cleared.
    Matched(PendingStop),
}

impl StopConfirmationState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Begin a pending stop for `user_id`. Any previous pending slot
    /// for the same user is overwritten — the user clicking `/stop`
    /// twice with a different id should always get a fresh prompt.
    pub fn start(&self, user_id: i64, pending: PendingStop) {
        let mut guard = self
            .pending
            .lock()
            .expect("stop-confirmation mutex poisoned");
        guard.insert(user_id, pending);
    }

    /// Drop any pending stop for `user_id`. No-op when absent.
    pub fn clear(&self, user_id: i64) {
        let mut guard = self
            .pending
            .lock()
            .expect("stop-confirmation mutex poisoned");
        guard.remove(&user_id);
    }

    /// Peek at the user's pending stop without consuming it. Returns
    /// `None` when the slot is empty or already expired (the slot is
    /// evicted eagerly on read).
    pub fn snapshot(&self, user_id: i64, now: DateTime<Utc>) -> Option<PendingStop> {
        let mut guard = self
            .pending
            .lock()
            .expect("stop-confirmation mutex poisoned");
        match guard.get(&user_id) {
            Some(p) if p.expires_at > now => Some(p.clone()),
            Some(_) => {
                guard.remove(&user_id);
                None
            }
            None => None,
        }
    }

    /// Try to consume the confirmation for `user_id` against `code`.
    /// Matching is case-insensitive (the slot stores lowercase).
    pub fn try_consume(
        &self,
        user_id: i64,
        code: &str,
        now: DateTime<Utc>,
    ) -> TryConsumeStop {
        let mut guard = self
            .pending
            .lock()
            .expect("stop-confirmation mutex poisoned");
        let Some(pending) = guard.get(&user_id) else {
            return TryConsumeStop::NoPending;
        };
        if pending.expires_at <= now {
            guard.remove(&user_id);
            return TryConsumeStop::Expired;
        }
        if pending.short_id != code.trim().to_ascii_lowercase() {
            return TryConsumeStop::Mismatch;
        }
        // Match: pop the entry and return it.
        let owned = guard.remove(&user_id).expect("we just confirmed presence");
        TryConsumeStop::Matched(owned)
    }
}

/// Contract a Tauri-shell-provided dispatcher must satisfy.
///
/// Kept synchronous on purpose: a single `kill` / `taskkill`
/// invocation completes in milliseconds, so an async signature would
/// add complexity (and an `async_trait` dep) for no benefit. If a
/// future implementation needs to perform a graceful SIGINT→SIGTERM
/// dance with a multi-second wait, it should spawn its own task and
/// still return synchronously from this method (with `Success` once
/// the *initial* signal is sent).
pub trait StopDispatcher: Send + Sync {
    fn stop(&self, session_id: Uuid) -> StopOutcome;
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn t(secs: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 17, 0, 0).unwrap() + Duration::seconds(secs)
    }

    fn pending(short: &str) -> PendingStop {
        PendingStop {
            session_id: Uuid::new_v4(),
            short_id: short.to_string(),
            command_id: Uuid::new_v4(),
            expires_at: t(60),
        }
    }

    #[test]
    fn empty_returns_no_pending() {
        let s = StopConfirmationState::new();
        assert_eq!(s.try_consume(1, "abc123", t(0)), TryConsumeStop::NoPending);
        assert!(s.snapshot(1, t(0)).is_none());
    }

    #[test]
    fn matched_consumes_slot_single_use() {
        let s = StopConfirmationState::new();
        let p = pending("abc123");
        s.start(1, p.clone());
        match s.try_consume(1, "abc123", t(0)) {
            TryConsumeStop::Matched(matched) => assert_eq!(matched.session_id, p.session_id),
            other => panic!("expected Matched, got {other:?}"),
        }
        assert_eq!(s.try_consume(1, "abc123", t(0)), TryConsumeStop::NoPending);
    }

    #[test]
    fn matched_is_case_insensitive() {
        let s = StopConfirmationState::new();
        s.start(1, pending("abc123"));
        assert!(matches!(
            s.try_consume(1, "ABC123", t(0)),
            TryConsumeStop::Matched(_)
        ));
    }

    #[test]
    fn expired_clears_slot() {
        let s = StopConfirmationState::new();
        s.start(1, pending("abc123"));
        assert_eq!(s.try_consume(1, "abc123", t(120)), TryConsumeStop::Expired);
        // Next attempt sees no pending (the expired slot is evicted).
        assert_eq!(s.try_consume(1, "abc123", t(120)), TryConsumeStop::NoPending);
    }

    #[test]
    fn mismatched_code_does_not_consume() {
        let s = StopConfirmationState::new();
        s.start(1, pending("abc123"));
        assert_eq!(s.try_consume(1, "wrong1", t(0)), TryConsumeStop::Mismatch);
        // Correct code still works after a mismatch.
        assert!(matches!(
            s.try_consume(1, "abc123", t(0)),
            TryConsumeStop::Matched(_)
        ));
    }

    #[test]
    fn start_replaces_previous_slot() {
        let s = StopConfirmationState::new();
        s.start(1, pending("aaaaaa"));
        s.start(1, pending("bbbbbb"));
        assert_eq!(s.try_consume(1, "aaaaaa", t(0)), TryConsumeStop::Mismatch);
        assert!(matches!(
            s.try_consume(1, "bbbbbb", t(0)),
            TryConsumeStop::Matched(_)
        ));
    }

    #[test]
    fn slots_are_per_user() {
        let s = StopConfirmationState::new();
        s.start(1, pending("abc123"));
        // User 2 has no slot.
        assert_eq!(s.try_consume(2, "abc123", t(0)), TryConsumeStop::NoPending);
        // User 1 still does.
        assert!(matches!(
            s.try_consume(1, "abc123", t(0)),
            TryConsumeStop::Matched(_)
        ));
    }

    #[test]
    fn snapshot_evicts_expired() {
        let s = StopConfirmationState::new();
        s.start(1, pending("abc123"));
        assert!(s.snapshot(1, t(0)).is_some());
        assert!(s.snapshot(1, t(120)).is_none());
        // And the slot is empty at earlier times too.
        assert!(s.snapshot(1, t(0)).is_none());
    }

    #[test]
    fn stop_outcome_is_success_only_for_success_variant() {
        assert!(StopOutcome::Success {
            mechanism: "SIGTERM".into()
        }
        .is_success());
        assert!(!StopOutcome::SessionNotFound.is_success());
        assert!(!StopOutcome::AlreadyCompleted.is_success());
        assert!(!StopOutcome::Unsupported {
            reason: "no pid".into()
        }
        .is_success());
        assert!(!StopOutcome::Failed {
            reason: "nope".into()
        }
        .is_success());
    }

    #[test]
    fn is_unsupported_covers_pre_attempt_failures() {
        assert!(!StopOutcome::Success {
            mechanism: "SIGTERM".into()
        }
        .is_unsupported());
        assert!(StopOutcome::SessionNotFound.is_unsupported());
        assert!(StopOutcome::AlreadyCompleted.is_unsupported());
        assert!(StopOutcome::Unsupported {
            reason: "no pid".into()
        }
        .is_unsupported());
        assert!(!StopOutcome::Failed {
            reason: "kill exited".into()
        }
        .is_unsupported());
    }
}
