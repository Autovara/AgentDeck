//! Recommended actions surfaced alongside an attention item.
//!
//! The list is serialised as a JSON array into
//! `attention_items.recommended_actions`. UIs render each action as a
//! button; Telegram quotes the labels verbatim.

use serde::{Deserialize, Serialize};

/// The user-facing actions an attention item may suggest.
///
/// Per `planning/alpha-build-plan.md` §10 the alpha menu is intentionally
/// short. `Approve` / `Continue` are *not* shippable until at least one
/// adapter can perform them reliably; do not add variants for them here
/// without a matching adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendedAction {
    /// Open the AgentDeck dashboard focused on the session.
    OpenDashboard,
    /// Mute future attention items for this `(session, reason)` for a
    /// user-chosen window. Default mute window is 1 hour.
    Mute,
    /// Stop the underlying process. The adapter performs the termination
    /// dispatch; the engine only suggests the action.
    Stop,
    /// Re-raise this attention item in the future (the Telegram backend
    /// will set a reminder).
    NotifyLater,
}

impl RecommendedAction {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::OpenDashboard => "open_dashboard",
            Self::Mute => "mute",
            Self::Stop => "stop",
            Self::NotifyLater => "notify_later",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "open_dashboard" => Some(Self::OpenDashboard),
            "mute" => Some(Self::Mute),
            "stop" => Some(Self::Stop),
            "notify_later" => Some(Self::NotifyLater),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_str_round_trip() {
        for a in [
            RecommendedAction::OpenDashboard,
            RecommendedAction::Mute,
            RecommendedAction::Stop,
            RecommendedAction::NotifyLater,
        ] {
            assert_eq!(RecommendedAction::from_db_str(a.as_db_str()), Some(a));
        }
        assert_eq!(RecommendedAction::from_db_str(""), None);
    }

    #[test]
    fn serde_snake_case() {
        let json = serde_json::to_string(&RecommendedAction::NotifyLater).unwrap();
        assert_eq!(json, "\"notify_later\"");
        let v: Vec<RecommendedAction> =
            vec![RecommendedAction::OpenDashboard, RecommendedAction::Mute];
        let arr = serde_json::to_string(&v).unwrap();
        assert_eq!(arr, "[\"open_dashboard\",\"mute\"]");
    }
}
