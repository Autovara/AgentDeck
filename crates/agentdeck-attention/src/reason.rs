//! Reason taxonomy for attention items.
//!
//! Stored verbatim in `attention_items.reason`; the column has no CHECK
//! constraint so this enum is the authoritative whitelist. Telegram and
//! the dashboard render whatever string the engine wrote.

use serde::{Deserialize, Serialize};

/// Why an attention item exists. The variants line up with
/// `planning/alpha-build-plan.md` §10.
///
/// Only the variants returned by [`crate::rules::propose_from_session`]
/// fire in the alpha; the rest are reserved so adapters can adopt them
/// without a schema change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionReason {
    WaitingForInput,
    ApprovalRequired,
    RateLimit,
    AuthenticationRequired,
    ContextLimit,
    StalledSession,
    CommandFailed,
    ProcessCrashed,
    BudgetThreshold,
}

impl AttentionReason {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::WaitingForInput => "waiting_for_input",
            Self::ApprovalRequired => "approval_required",
            Self::RateLimit => "rate_limit",
            Self::AuthenticationRequired => "authentication_required",
            Self::ContextLimit => "context_limit",
            Self::StalledSession => "stalled_session",
            Self::CommandFailed => "command_failed",
            Self::ProcessCrashed => "process_crashed",
            Self::BudgetThreshold => "budget_threshold",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "waiting_for_input" => Some(Self::WaitingForInput),
            "approval_required" => Some(Self::ApprovalRequired),
            "rate_limit" => Some(Self::RateLimit),
            "authentication_required" => Some(Self::AuthenticationRequired),
            "context_limit" => Some(Self::ContextLimit),
            "stalled_session" => Some(Self::StalledSession),
            "command_failed" => Some(Self::CommandFailed),
            "process_crashed" => Some(Self::ProcessCrashed),
            "budget_threshold" => Some(Self::BudgetThreshold),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_str_round_trips_every_variant() {
        for r in [
            AttentionReason::WaitingForInput,
            AttentionReason::ApprovalRequired,
            AttentionReason::RateLimit,
            AttentionReason::AuthenticationRequired,
            AttentionReason::ContextLimit,
            AttentionReason::StalledSession,
            AttentionReason::CommandFailed,
            AttentionReason::ProcessCrashed,
            AttentionReason::BudgetThreshold,
        ] {
            assert_eq!(AttentionReason::from_db_str(r.as_db_str()), Some(r));
        }
    }

    #[test]
    fn from_db_str_rejects_unknown() {
        assert_eq!(AttentionReason::from_db_str(""), None);
        assert_eq!(AttentionReason::from_db_str("nope"), None);
    }

    #[test]
    fn serde_uses_snake_case() {
        let json = serde_json::to_string(&AttentionReason::WaitingForInput).unwrap();
        assert_eq!(json, "\"waiting_for_input\"");
        let parsed: AttentionReason = serde_json::from_str("\"rate_limit\"").unwrap();
        assert_eq!(parsed, AttentionReason::RateLimit);
    }
}
