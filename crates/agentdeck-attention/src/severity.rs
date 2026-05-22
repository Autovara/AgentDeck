//! Severity tier for attention items.
//!
//! Must round-trip through the `attention_items.severity` CHECK
//! constraint in `agentdeck-storage/migrations/0001_initial.sql`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AttentionSeverity {
    Info,
    Warn,
    Urgent,
}

impl AttentionSeverity {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Urgent => "urgent",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "urgent" => Some(Self::Urgent),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn db_str_round_trip() {
        for s in [
            AttentionSeverity::Info,
            AttentionSeverity::Warn,
            AttentionSeverity::Urgent,
        ] {
            assert_eq!(AttentionSeverity::from_db_str(s.as_db_str()), Some(s));
        }
    }

    #[test]
    fn from_db_str_rejects_unknown() {
        assert_eq!(AttentionSeverity::from_db_str(""), None);
        assert_eq!(AttentionSeverity::from_db_str("URGENT"), None);
    }

    #[test]
    fn serde_lowercase() {
        let json = serde_json::to_string(&AttentionSeverity::Urgent).unwrap();
        assert_eq!(json, "\"urgent\"");
    }
}
