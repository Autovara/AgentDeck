//! In-memory shapes that mirror the `attention_items` row.

use agentdeck_adapter::{Confidence, SessionStatus};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::action::RecommendedAction;
use crate::reason::AttentionReason;
use crate::severity::AttentionSeverity;

/// One row of `attention_items`.
///
/// Field order matches the SQL column order in
/// `agentdeck-storage/migrations/0001_initial.sql` so the repository read
/// path is a flat top-to-bottom copy.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionItem {
    pub id: Uuid,
    pub session_id: Uuid,
    pub reason: AttentionReason,
    pub severity: AttentionSeverity,
    pub message: String,
    /// Free-form provenance, e.g. `"session-status:rate_limited"`. Used
    /// by the diagnostics card and by Telegram replies.
    pub source: String,
    pub confidence: Confidence,
    pub recommended_actions: Vec<RecommendedAction>,
    pub created_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub muted_until: Option<DateTime<Utc>>,
}

impl AttentionItem {
    /// Whether the item is currently muted at `now`. Items whose
    /// `muted_until` is `None` are never muted.
    pub fn is_muted_at(&self, now: DateTime<Utc>) -> bool {
        self.muted_until.is_some_and(|t| t > now)
    }
}

/// Engine-side proposal for an attention item, before it has been
/// persisted (so no `id`, `created_at`, `resolved_at`, or `muted_until`).
///
/// Produced by [`crate::rules::propose_from_session`]; consumed by
/// [`crate::engine::AttentionEngine::apply`].
#[derive(Debug, Clone, PartialEq)]
pub struct ProposedAttention {
    pub session_id: Uuid,
    pub reason: AttentionReason,
    pub severity: AttentionSeverity,
    pub message: String,
    pub source: String,
    pub confidence: Confidence,
    pub recommended_actions: Vec<RecommendedAction>,
}

impl ProposedAttention {
    /// Build an [`AttentionItem`] from a proposal. The engine uses this
    /// when inserting a new row.
    pub(crate) fn into_item(self, id: Uuid, now: DateTime<Utc>) -> AttentionItem {
        AttentionItem {
            id,
            session_id: self.session_id,
            reason: self.reason,
            severity: self.severity,
            message: self.message,
            source: self.source,
            confidence: self.confidence,
            recommended_actions: self.recommended_actions,
            created_at: now,
            resolved_at: None,
            muted_until: None,
        }
    }

    /// Whether the proposal differs from an existing item in any
    /// engine-managed field. Used to decide if an UPDATE is needed.
    pub(crate) fn differs_from(&self, item: &AttentionItem) -> bool {
        self.severity != item.severity
            || self.message != item.message
            || self.source != item.source
            || self.confidence != item.confidence
            || self.recommended_actions != item.recommended_actions
    }
}

/// Lightweight summary attached to an open attention item when the
/// dashboard renders the list, joined from `sessions`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRef {
    pub id: Uuid,
    pub agent_name: String,
    pub adapter_name: String,
    pub status: SessionStatus,
    pub pid: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 14, 0, 0).unwrap()
    }

    fn item() -> AttentionItem {
        AttentionItem {
            id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            reason: AttentionReason::RateLimit,
            severity: AttentionSeverity::Warn,
            message: "rate-limited".into(),
            source: "session-status:rate_limited".into(),
            confidence: Confidence::Medium,
            recommended_actions: vec![RecommendedAction::OpenDashboard],
            created_at: ts(),
            resolved_at: None,
            muted_until: None,
        }
    }

    #[test]
    fn is_muted_at_returns_false_without_mute() {
        let i = item();
        assert!(!i.is_muted_at(ts()));
    }

    #[test]
    fn is_muted_at_handles_future_and_past_windows() {
        let mut i = item();
        i.muted_until = Some(ts() + chrono::Duration::hours(1));
        assert!(i.is_muted_at(ts()));
        assert!(!i.is_muted_at(ts() + chrono::Duration::hours(2)));
    }

    #[test]
    fn proposal_into_item_initialises_defaults() {
        let p = ProposedAttention {
            session_id: Uuid::new_v4(),
            reason: AttentionReason::RateLimit,
            severity: AttentionSeverity::Warn,
            message: "rate-limited".into(),
            source: "session-status:rate_limited".into(),
            confidence: Confidence::Medium,
            recommended_actions: vec![RecommendedAction::OpenDashboard],
        };
        let id = Uuid::new_v4();
        let now = ts();
        let item = p.into_item(id, now);
        assert_eq!(item.id, id);
        assert_eq!(item.created_at, now);
        assert!(item.resolved_at.is_none());
        assert!(item.muted_until.is_none());
    }

    #[test]
    fn proposal_differs_from_detects_each_field() {
        let i = item();
        let baseline = ProposedAttention {
            session_id: i.session_id,
            reason: i.reason,
            severity: i.severity,
            message: i.message.clone(),
            source: i.source.clone(),
            confidence: i.confidence,
            recommended_actions: i.recommended_actions.clone(),
        };
        assert!(!baseline.differs_from(&i));

        let mut diff = baseline.clone();
        diff.severity = AttentionSeverity::Urgent;
        assert!(diff.differs_from(&i));

        let mut diff = baseline.clone();
        diff.message = "different".into();
        assert!(diff.differs_from(&i));

        let mut diff = baseline.clone();
        diff.recommended_actions = vec![RecommendedAction::Stop];
        assert!(diff.differs_from(&i));
    }
}
