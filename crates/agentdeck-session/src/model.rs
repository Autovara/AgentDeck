//! In-memory shapes for the `sessions` and `session_events` tables.

use agentdeck_adapter::{AdapterMatch, CapabilityLevel, Confidence, CostKind, SessionStatus};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

/// `session_events.event_kind` value for the row appended on first
/// detection of a `(adapter_name, pid)` pair.
pub const EVENT_KIND_DETECTED: &str = "detected";

/// `session_events.event_kind` value for the row appended whenever
/// `sessions.status` changes between two ticks.
pub const EVENT_KIND_STATUS_CHANGED: &str = "status_changed";

/// `session_events.event_kind` value for the row appended when a session
/// transitions to [`SessionStatus::Completed`] because its PID is no
/// longer matched.
pub const EVENT_KIND_PROCESS_EXITED: &str = "process_exited";

/// One row in the `sessions` table.
///
/// Field order mirrors the SQL column order in
/// `agentdeck-storage/migrations/0001_initial.sql` so the repository read
/// path is a flat top-to-bottom copy.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: Uuid,
    pub agent_name: String,
    pub adapter_name: String,
    pub adapter_level: CapabilityLevel,
    pub pid: Option<u32>,
    pub command: String,
    pub cwd: Option<String>,
    pub repo_path: Option<String>,
    pub project_tag: Option<String>,
    pub status: SessionStatus,
    pub status_confidence: Confidence,
    pub attention_reason: Option<String>,
    pub start_time: DateTime<Utc>,
    pub last_seen_time: DateTime<Utc>,
    pub last_activity_time: Option<DateTime<Utc>>,
    pub estimated_cost: Option<f64>,
    pub cost_kind: CostKind,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Session {
    /// Build a fresh session row from the first match that named it.
    /// `snapshot_time` is the canonical "as of when" of the tick;
    /// `now` is the wall clock used for `created_at` / `updated_at`.
    pub(crate) fn from_match_initial(
        id: Uuid,
        m: &AdapterMatch,
        snapshot_time: DateTime<Utc>,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            agent_name: m.agent_name.clone(),
            adapter_name: m.adapter_name.clone(),
            adapter_level: m.capability_level,
            pid: Some(m.pid),
            command: m.command.clone(),
            cwd: m.cwd.clone(),
            repo_path: m.repo_path.clone(),
            project_tag: None,
            status: m.status,
            status_confidence: m.status_confidence,
            attention_reason: None,
            start_time: snapshot_time,
            last_seen_time: snapshot_time,
            last_activity_time: None,
            estimated_cost: None,
            cost_kind: CostKind::Unknown,
            created_at: now,
            updated_at: now,
        }
    }
}

/// One row in the `session_events` table.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEvent {
    pub id: i64,
    pub session_id: Uuid,
    pub event_kind: String,
    pub payload: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// Build the JSON payload written into `session_events.payload` for a
/// `detected` event. Kept as a free function so tests can assert the
/// schema without round-tripping through SQLite.
pub fn detected_event_payload(m: &AdapterMatch) -> serde_json::Value {
    json!({
        "pid": m.pid,
        "command": m.command,
        "cwd": m.cwd,
        "repo_path": m.repo_path,
        "status": m.status.as_db_str(),
        "confidence": m.status_confidence.as_db_str(),
        "capability_level": m.capability_level.as_u8(),
        "status_source": m.status_source,
    })
}

/// Build the JSON payload for a `status_changed` event.
pub fn status_changed_event_payload(
    from: SessionStatus,
    to: SessionStatus,
    confidence: Confidence,
    source: &str,
) -> serde_json::Value {
    json!({
        "from": from.as_db_str(),
        "to": to.as_db_str(),
        "confidence": confidence.as_db_str(),
        "source": source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap()
    }

    fn sample_match() -> AdapterMatch {
        AdapterMatch {
            adapter_name: "custom".into(),
            agent_name: "aider".into(),
            capability_level: CapabilityLevel::Presence,
            pid: 1234,
            command: "aider /repo".into(),
            cwd: Some("/repo".into()),
            repo_path: Some("/repo".into()),
            status: SessionStatus::Running,
            status_confidence: Confidence::Medium,
            status_source: "process-list".into(),
            observed_at: ts(),
        }
    }

    #[test]
    fn from_match_initial_copies_fields() {
        let id = Uuid::new_v4();
        let m = sample_match();
        let now = ts();
        let s = Session::from_match_initial(id, &m, ts(), now);
        assert_eq!(s.id, id);
        assert_eq!(s.adapter_name, "custom");
        assert_eq!(s.agent_name, "aider");
        assert_eq!(s.adapter_level, CapabilityLevel::Presence);
        assert_eq!(s.pid, Some(1234));
        assert_eq!(s.command, "aider /repo");
        assert_eq!(s.cwd.as_deref(), Some("/repo"));
        assert_eq!(s.repo_path.as_deref(), Some("/repo"));
        assert_eq!(s.status, SessionStatus::Running);
        assert_eq!(s.status_confidence, Confidence::Medium);
        assert_eq!(s.project_tag, None);
        assert_eq!(s.attention_reason, None);
        assert_eq!(s.start_time, ts());
        assert_eq!(s.last_seen_time, ts());
        assert!(s.last_activity_time.is_none());
        assert!(s.estimated_cost.is_none());
        assert_eq!(s.cost_kind, CostKind::Unknown);
        assert_eq!(s.created_at, now);
        assert_eq!(s.updated_at, now);
    }

    #[test]
    fn detected_payload_has_expected_fields() {
        let m = sample_match();
        let payload = detected_event_payload(&m);
        assert_eq!(payload["pid"], 1234);
        assert_eq!(payload["command"], "aider /repo");
        assert_eq!(payload["cwd"], "/repo");
        assert_eq!(payload["status"], "running");
        assert_eq!(payload["confidence"], "medium");
        assert_eq!(payload["capability_level"], 1);
        assert_eq!(payload["status_source"], "process-list");
    }

    #[test]
    fn status_changed_payload_renders_both_ends() {
        let p = status_changed_event_payload(
            SessionStatus::Running,
            SessionStatus::WaitingForInput,
            Confidence::High,
            "history-mtime",
        );
        assert_eq!(p["from"], "running");
        assert_eq!(p["to"], "waiting_for_input");
        assert_eq!(p["confidence"], "high");
        assert_eq!(p["source"], "history-mtime");
    }
}
