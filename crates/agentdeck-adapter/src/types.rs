//! Enum types shared by the adapter trait, the registry, and the monitor
//! core. Every value here round-trips through the corresponding SQL CHECK
//! constraint in `agentdeck-storage/migrations/0001_initial.sql`.

use serde::{Deserialize, Serialize};

/// Adapter capability tier from `planning/alpha-build-plan.md` §7.
///
/// An adapter must report the highest tier it can support honestly; lower
/// tiers are valid alpha targets and the diagnostics card labels each
/// adapter with its declared level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityLevel {
    /// Detects the process; reports running / completed only.
    Presence,
    /// Adds waiting / idle / stalled / errored / rate-limited classification.
    Status,
    /// Adds token and cost extraction.
    Usage,
    /// Adds safe control actions (stop, mute, continue, approve).
    Control,
}

impl CapabilityLevel {
    /// 1..=4 encoding used by the `sessions.adapter_level` and
    /// `adapter_diagnostics.capability_level` SQL CHECK constraints.
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Presence => 1,
            Self::Status => 2,
            Self::Usage => 3,
            Self::Control => 4,
        }
    }

    /// Round-trip from the SQL integer encoding.
    pub fn from_u8(n: u8) -> Option<Self> {
        match n {
            1 => Some(Self::Presence),
            2 => Some(Self::Status),
            3 => Some(Self::Usage),
            4 => Some(Self::Control),
            _ => None,
        }
    }
}

/// Session status enum mirroring `sessions.status` from
/// `agentdeck-storage/migrations/0001_initial.sql`.
///
/// Concrete classification rules live in
/// `planning/alpha-build-plan.md` §9. Level 1 (Presence) adapters can only
/// emit `Running`; `Completed` is set by the session state machine when a
/// previously-seen PID disappears, not by the adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Running,
    Idle,
    WaitingForInput,
    RateLimited,
    Stalled,
    Errored,
    Completed,
    Unknown,
}

impl SessionStatus {
    /// Exact string used by the `sessions.status` CHECK constraint.
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Idle => "idle",
            Self::WaitingForInput => "waiting_for_input",
            Self::RateLimited => "rate_limited",
            Self::Stalled => "stalled",
            Self::Errored => "errored",
            Self::Completed => "completed",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "running" => Some(Self::Running),
            "idle" => Some(Self::Idle),
            "waiting_for_input" => Some(Self::WaitingForInput),
            "rate_limited" => Some(Self::RateLimited),
            "stalled" => Some(Self::Stalled),
            "errored" => Some(Self::Errored),
            "completed" => Some(Self::Completed),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// Confidence label attached to every status classification and diagnostic.
///
/// Mirrors `sessions.status_confidence` and
/// `adapter_diagnostics.confidence`. The alpha defaults to `Medium` for
/// Level 1 ("the process is alive; the rest is best-effort"); higher
/// levels lift to `High` when the signal is concrete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    High,
    Medium,
    Low,
    Unknown,
}

impl Confidence {
    pub const fn as_db_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "high" => Some(Self::High),
            "medium" => Some(Self::Medium),
            "low" => Some(Self::Low),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_level_u8_round_trip() {
        for c in [
            CapabilityLevel::Presence,
            CapabilityLevel::Status,
            CapabilityLevel::Usage,
            CapabilityLevel::Control,
        ] {
            assert_eq!(CapabilityLevel::from_u8(c.as_u8()), Some(c));
        }
        assert_eq!(CapabilityLevel::from_u8(0), None);
        assert_eq!(CapabilityLevel::from_u8(5), None);
        assert_eq!(CapabilityLevel::from_u8(255), None);
    }

    #[test]
    fn capability_level_serde_snake() {
        let json = serde_json::to_string(&CapabilityLevel::Presence).unwrap();
        assert_eq!(json, "\"presence\"");
        let parsed: CapabilityLevel = serde_json::from_str("\"control\"").unwrap();
        assert_eq!(parsed, CapabilityLevel::Control);
    }

    #[test]
    fn session_status_db_str_round_trip() {
        for s in [
            SessionStatus::Running,
            SessionStatus::Idle,
            SessionStatus::WaitingForInput,
            SessionStatus::RateLimited,
            SessionStatus::Stalled,
            SessionStatus::Errored,
            SessionStatus::Completed,
            SessionStatus::Unknown,
        ] {
            assert_eq!(SessionStatus::from_db_str(s.as_db_str()), Some(s));
        }
        assert_eq!(SessionStatus::from_db_str(""), None);
        assert_eq!(SessionStatus::from_db_str("nope"), None);
    }

    #[test]
    fn session_status_serde_snake() {
        let json = serde_json::to_string(&SessionStatus::WaitingForInput).unwrap();
        assert_eq!(json, "\"waiting_for_input\"");
        let parsed: SessionStatus = serde_json::from_str("\"rate_limited\"").unwrap();
        assert_eq!(parsed, SessionStatus::RateLimited);
    }

    #[test]
    fn confidence_db_str_round_trip() {
        for c in [
            Confidence::High,
            Confidence::Medium,
            Confidence::Low,
            Confidence::Unknown,
        ] {
            assert_eq!(Confidence::from_db_str(c.as_db_str()), Some(c));
        }
        assert_eq!(Confidence::from_db_str(""), None);
    }

    #[test]
    fn confidence_serde_lowercase() {
        let json = serde_json::to_string(&Confidence::Medium).unwrap();
        assert_eq!(json, "\"medium\"");
    }
}
