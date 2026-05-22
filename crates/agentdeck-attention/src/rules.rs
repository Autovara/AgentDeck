//! Pure rule functions over [`Session`] -> [`ProposedAttention`].
//!
//! Every rule is a small, pure function; the engine composes them. Keep
//! the rules deterministic and side-effect-free so they can be tested in
//! isolation.

use agentdeck_adapter::{CapabilityLevel, SessionStatus};
use agentdeck_session::Session;

use crate::action::RecommendedAction;
use crate::model::ProposedAttention;
use crate::reason::AttentionReason;
use crate::severity::AttentionSeverity;

/// Map a session to its attention proposal, if any.
///
/// Returns `None` when the session is in a normal state (`Running`,
/// `Idle`, `Completed`, `Unknown`) or when the session's adapter level is
/// too low to support the matched reason. See
/// `planning/alpha-build-plan.md` §10 TUI caveat for why `waiting_for_input`
/// is gated to `>= Status`.
pub fn propose_from_session(session: &Session) -> Option<ProposedAttention> {
    let session_id = session.id;
    let confidence = session.status_confidence;
    let agent = session.agent_name.as_str();

    match session.status {
        SessionStatus::WaitingForInput => {
            // Hard gate: only adapters that have moved past Level 1 may emit
            // waiting-for-input attention. Level 1 (presence-only) cannot
            // distinguish "the TUI is thinking" from "the TUI is waiting for
            // me" — see build-plan §10.
            if session.adapter_level == CapabilityLevel::Presence {
                return None;
            }
            Some(ProposedAttention {
                session_id,
                reason: AttentionReason::WaitingForInput,
                severity: AttentionSeverity::Info,
                message: format!("{agent} is waiting for input"),
                source: source_from_status(SessionStatus::WaitingForInput),
                confidence,
                recommended_actions: vec![
                    RecommendedAction::OpenDashboard,
                    RecommendedAction::Mute,
                    RecommendedAction::NotifyLater,
                ],
            })
        }
        SessionStatus::RateLimited => Some(ProposedAttention {
            session_id,
            reason: AttentionReason::RateLimit,
            severity: AttentionSeverity::Warn,
            message: format!("{agent} is rate-limited by its provider"),
            source: source_from_status(SessionStatus::RateLimited),
            confidence,
            recommended_actions: vec![
                RecommendedAction::OpenDashboard,
                RecommendedAction::Mute,
                RecommendedAction::NotifyLater,
            ],
        }),
        SessionStatus::Stalled => Some(ProposedAttention {
            session_id,
            reason: AttentionReason::StalledSession,
            severity: AttentionSeverity::Info,
            message: format!("{agent} has been idle long enough to look stalled"),
            source: source_from_status(SessionStatus::Stalled),
            confidence,
            recommended_actions: vec![
                RecommendedAction::OpenDashboard,
                RecommendedAction::Mute,
                RecommendedAction::Stop,
            ],
        }),
        SessionStatus::Errored => Some(ProposedAttention {
            session_id,
            reason: AttentionReason::CommandFailed,
            severity: AttentionSeverity::Warn,
            message: format!("{agent} reported an error"),
            source: source_from_status(SessionStatus::Errored),
            confidence,
            recommended_actions: vec![
                RecommendedAction::OpenDashboard,
                RecommendedAction::Mute,
                RecommendedAction::Stop,
            ],
        }),
        SessionStatus::Running
        | SessionStatus::Idle
        | SessionStatus::Completed
        | SessionStatus::Unknown => None,
    }
}

/// `attention_items.source` value used by status-derived proposals.
fn source_from_status(status: SessionStatus) -> String {
    format!("session-status:{}", status.as_db_str())
}

/// The set of [`SessionStatus`] values for which `propose_from_session`
/// can fire. Kept as a constant for tests and for diagnostics.
pub const ATTENTION_SOURCE_STATUSES: &[SessionStatus] = &[
    SessionStatus::WaitingForInput,
    SessionStatus::RateLimited,
    SessionStatus::Stalled,
    SessionStatus::Errored,
];

#[cfg(test)]
mod tests {
    use super::*;
    use agentdeck_adapter::{Confidence, CostKind};
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn session(status: SessionStatus, level: CapabilityLevel) -> Session {
        let now = Utc.with_ymd_and_hms(2026, 5, 22, 14, 0, 0).unwrap();
        Session {
            id: Uuid::new_v4(),
            agent_name: "aider".into(),
            adapter_name: "aider".into(),
            adapter_level: level,
            pid: Some(42),
            command: "aider /repo".into(),
            cwd: Some("/repo".into()),
            repo_path: Some("/repo".into()),
            project_tag: None,
            status,
            status_confidence: Confidence::Medium,
            attention_reason: None,
            start_time: now,
            last_seen_time: now,
            last_activity_time: None,
            estimated_cost: None,
            cost_kind: CostKind::Unknown,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn running_session_emits_no_proposal() {
        assert!(
            propose_from_session(&session(SessionStatus::Running, CapabilityLevel::Presence))
                .is_none()
        );
    }

    #[test]
    fn level_one_waiting_does_not_propose() {
        let s = session(SessionStatus::WaitingForInput, CapabilityLevel::Presence);
        assert!(
            propose_from_session(&s).is_none(),
            "Level 1 adapters cannot detect waiting reliably (build-plan §10 TUI caveat)"
        );
    }

    #[test]
    fn level_two_waiting_emits_info() {
        let s = session(SessionStatus::WaitingForInput, CapabilityLevel::Status);
        let p = propose_from_session(&s).expect("level 2 waiting should propose");
        assert_eq!(p.reason, AttentionReason::WaitingForInput);
        assert_eq!(p.severity, AttentionSeverity::Info);
        assert_eq!(p.source, "session-status:waiting_for_input");
        assert!(p.recommended_actions.contains(&RecommendedAction::Mute));
        assert!(p
            .recommended_actions
            .contains(&RecommendedAction::OpenDashboard));
    }

    #[test]
    fn rate_limited_is_warn_at_any_level() {
        for lvl in [
            CapabilityLevel::Presence,
            CapabilityLevel::Status,
            CapabilityLevel::Usage,
            CapabilityLevel::Control,
        ] {
            let p = propose_from_session(&session(SessionStatus::RateLimited, lvl))
                .expect("rate_limited always proposes");
            assert_eq!(p.reason, AttentionReason::RateLimit);
            assert_eq!(p.severity, AttentionSeverity::Warn);
        }
    }

    #[test]
    fn stalled_is_info_with_stop_action() {
        let p = propose_from_session(&session(SessionStatus::Stalled, CapabilityLevel::Presence))
            .unwrap();
        assert_eq!(p.reason, AttentionReason::StalledSession);
        assert_eq!(p.severity, AttentionSeverity::Info);
        assert!(p.recommended_actions.contains(&RecommendedAction::Stop));
    }

    #[test]
    fn errored_is_warn_command_failed() {
        let p = propose_from_session(&session(SessionStatus::Errored, CapabilityLevel::Status))
            .unwrap();
        assert_eq!(p.reason, AttentionReason::CommandFailed);
        assert_eq!(p.severity, AttentionSeverity::Warn);
        assert!(p.recommended_actions.contains(&RecommendedAction::Stop));
    }

    #[test]
    fn completed_emits_no_proposal() {
        let p = propose_from_session(&session(
            SessionStatus::Completed,
            CapabilityLevel::Presence,
        ));
        assert!(p.is_none(), "completed sessions never propose attention");
    }

    #[test]
    fn source_strings_round_trip_through_status_names() {
        for s in ATTENTION_SOURCE_STATUSES {
            assert_eq!(
                source_from_status(*s),
                format!("session-status:{}", s.as_db_str())
            );
        }
    }
}
