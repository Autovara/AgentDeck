//! CSV + JSON export of AgentDeck sessions.
//!
//! Build-plan §15 step 18 calls for project/client export. The alpha
//! ships sessions-only exports — `usage_records` is empty until Level 3
//! adapters land, and the audit log is reserved for a future surface.
//!
//! Both formats expose the same fields:
//!
//! - `id`, `agent_name`, `adapter_name`, `adapter_level`, `pid`
//! - `command`, `cwd`, `repo_path`, `project_tag`
//! - `status`, `status_confidence`
//! - `estimated_cost`, `cost_kind`
//! - `start_time`, `last_seen_time`, `created_at`, `updated_at`
//!
//! Field naming follows the SQL column names so the export is
//! self-documenting against the schema; the CSV header row matches the
//! JSON object keys.
//!
//! CSV is hand-rolled (sessions have a fixed shape; pulling in the
//! `csv` crate is unnecessary).

mod error;

use std::sync::Arc;

use agentdeck_session::{list_all, Session};
use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use serde::Serialize;

pub use error::ExportError;

/// One row of a sessions export. The shape is the same for both
/// formats; the JSON encoder uses serde and the CSV encoder reads the
/// same fields manually.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionExportRow {
    pub id: String,
    pub agent_name: String,
    pub adapter_name: String,
    pub adapter_level: u8,
    pub pid: Option<u32>,
    pub command: String,
    pub cwd: Option<String>,
    pub repo_path: Option<String>,
    pub project_tag: Option<String>,
    pub status: String,
    pub status_confidence: String,
    pub estimated_cost: Option<f64>,
    pub cost_kind: String,
    pub start_time: String,
    pub last_seen_time: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<&Session> for SessionExportRow {
    fn from(s: &Session) -> Self {
        Self {
            id: s.id.to_string(),
            agent_name: s.agent_name.clone(),
            adapter_name: s.adapter_name.clone(),
            adapter_level: s.adapter_level.as_u8(),
            pid: s.pid,
            command: s.command.clone(),
            cwd: s.cwd.clone(),
            repo_path: s.repo_path.clone(),
            project_tag: s.project_tag.clone(),
            status: s.status.as_db_str().to_string(),
            status_confidence: s.status_confidence.as_db_str().to_string(),
            estimated_cost: s.estimated_cost,
            cost_kind: s.cost_kind.as_db_str().to_string(),
            start_time: format_ts(s.start_time),
            last_seen_time: format_ts(s.last_seen_time),
            created_at: format_ts(s.created_at),
            updated_at: format_ts(s.updated_at),
        }
    }
}

/// Field names in stable column order — used by both the CSV header
/// and the docstrings.
pub const COLUMNS: &[&str] = &[
    "id",
    "agent_name",
    "adapter_name",
    "adapter_level",
    "pid",
    "command",
    "cwd",
    "repo_path",
    "project_tag",
    "status",
    "status_confidence",
    "estimated_cost",
    "cost_kind",
    "start_time",
    "last_seen_time",
    "created_at",
    "updated_at",
];

/// Collect every session in the DB (active and completed alike) and
/// serialise to a pretty-printed JSON array. Callers typically write
/// the returned string to a file.
pub fn export_sessions_json(storage: &Arc<Storage>) -> Result<String, ExportError> {
    let rows = load_all_sessions(storage)?;
    let serialised: Vec<SessionExportRow> = rows.iter().map(SessionExportRow::from).collect();
    Ok(serde_json::to_string_pretty(&serialised)?)
}

/// Collect every session and serialise to CSV (RFC-4180 quoting:
/// fields with commas, quotes, or newlines are wrapped in `"`, internal
/// `"` are doubled).
pub fn export_sessions_csv(storage: &Arc<Storage>) -> Result<String, ExportError> {
    let rows = load_all_sessions(storage)?;
    let mut out = String::new();
    write_csv_header(&mut out);
    for s in &rows {
        write_csv_row(&mut out, &SessionExportRow::from(s));
    }
    Ok(out)
}

fn load_all_sessions(storage: &Arc<Storage>) -> Result<Vec<Session>, ExportError> {
    list_all(storage).map_err(ExportError::from)
}

fn format_ts(ts: DateTime<Utc>) -> String {
    ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

// --- CSV writer ----------------------------------------------------------

fn write_csv_header(out: &mut String) {
    let mut first = true;
    for col in COLUMNS {
        if !first {
            out.push(',');
        }
        first = false;
        out.push_str(col);
    }
    out.push_str("\r\n");
}

fn write_csv_row(out: &mut String, row: &SessionExportRow) {
    write_csv_field(out, &row.id);
    csv_comma(out);
    write_csv_field(out, &row.agent_name);
    csv_comma(out);
    write_csv_field(out, &row.adapter_name);
    csv_comma(out);
    out.push_str(&row.adapter_level.to_string());
    csv_comma(out);
    if let Some(p) = row.pid {
        out.push_str(&p.to_string());
    }
    csv_comma(out);
    write_csv_field(out, &row.command);
    csv_comma(out);
    if let Some(c) = row.cwd.as_deref() {
        write_csv_field(out, c);
    }
    csv_comma(out);
    if let Some(r) = row.repo_path.as_deref() {
        write_csv_field(out, r);
    }
    csv_comma(out);
    if let Some(t) = row.project_tag.as_deref() {
        write_csv_field(out, t);
    }
    csv_comma(out);
    write_csv_field(out, &row.status);
    csv_comma(out);
    write_csv_field(out, &row.status_confidence);
    csv_comma(out);
    if let Some(cost) = row.estimated_cost {
        // Cost is in USD; render with cent precision so spreadsheets
        // do not get clever with rounding.
        out.push_str(&format!("{cost:.4}"));
    }
    csv_comma(out);
    write_csv_field(out, &row.cost_kind);
    csv_comma(out);
    write_csv_field(out, &row.start_time);
    csv_comma(out);
    write_csv_field(out, &row.last_seen_time);
    csv_comma(out);
    write_csv_field(out, &row.created_at);
    csv_comma(out);
    write_csv_field(out, &row.updated_at);
    out.push_str("\r\n");
}

fn csv_comma(out: &mut String) {
    out.push(',');
}

/// Quote a CSV field per RFC-4180 only when necessary.
fn write_csv_field(out: &mut String, value: &str) {
    let needs_quotes = value
        .chars()
        .any(|c| c == ',' || c == '"' || c == '\n' || c == '\r');
    if !needs_quotes {
        out.push_str(value);
        return;
    }
    out.push('"');
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
            out.push('"');
        } else {
            out.push(ch);
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentdeck_adapter::{CapabilityLevel, Confidence, CostKind, SessionStatus};
    use chrono::TimeZone;
    use uuid::Uuid;

    fn sample_session() -> Session {
        let id = Uuid::parse_str("11111111-1111-4111-8111-111111111111").unwrap();
        let ts = Utc.with_ymd_and_hms(2026, 5, 22, 12, 0, 0).unwrap();
        Session {
            id,
            agent_name: "aider".into(),
            adapter_name: "aider".into(),
            adapter_level: CapabilityLevel::Presence,
            pid: Some(1234),
            command: "aider /repo, file.rs".into(), // commas to test CSV quoting
            cwd: Some("/repo".into()),
            repo_path: Some("/repo".into()),
            project_tag: Some("billing".into()),
            status: SessionStatus::Running,
            status_confidence: Confidence::Medium,
            attention_reason: None,
            start_time: ts,
            last_seen_time: ts,
            last_activity_time: None,
            estimated_cost: Some(1.2345),
            cost_kind: CostKind::Estimated,
            created_at: ts,
            updated_at: ts,
        }
    }

    #[test]
    fn export_row_from_session_copies_fields() {
        let s = sample_session();
        let row = SessionExportRow::from(&s);
        assert_eq!(row.agent_name, "aider");
        assert_eq!(row.adapter_level, 1);
        assert_eq!(row.pid, Some(1234));
        assert_eq!(row.project_tag.as_deref(), Some("billing"));
        assert_eq!(row.cost_kind, "estimated");
        assert!(row.start_time.ends_with("Z"));
    }

    #[test]
    fn csv_header_lists_every_column() {
        let mut out = String::new();
        write_csv_header(&mut out);
        let header = out.trim_end();
        let cols: Vec<&str> = header.split(',').collect();
        assert_eq!(cols, COLUMNS);
    }

    #[test]
    fn csv_field_unquoted_when_safe() {
        let mut out = String::new();
        write_csv_field(&mut out, "hello");
        assert_eq!(out, "hello");
    }

    #[test]
    fn csv_field_quoted_when_contains_comma() {
        let mut out = String::new();
        write_csv_field(&mut out, "a,b");
        assert_eq!(out, "\"a,b\"");
    }

    #[test]
    fn csv_field_doubles_internal_quotes() {
        let mut out = String::new();
        write_csv_field(&mut out, r#"say "hi""#);
        assert_eq!(out, r#""say ""hi""""#);
    }

    #[test]
    fn csv_field_quoted_when_contains_newline() {
        let mut out = String::new();
        write_csv_field(&mut out, "a\nb");
        assert_eq!(out, "\"a\nb\"");
    }

    #[test]
    fn csv_row_serialises_all_fields() {
        let mut out = String::new();
        write_csv_row(&mut out, &SessionExportRow::from(&sample_session()));
        let row = out.trim_end_matches("\r\n");
        // First field is the UUID.
        assert!(row.starts_with("11111111-1111-4111-8111-111111111111,"));
        // Command contains a comma → quoted in the field.
        assert!(row.contains("\"aider /repo, file.rs\""));
        // Cost rendered with four decimals.
        assert!(row.contains("1.2345"));
        // Tag preserved.
        assert!(row.contains("billing"));
    }

    #[test]
    fn empty_optionals_become_empty_csv_fields() {
        let mut s = sample_session();
        s.cwd = None;
        s.repo_path = None;
        s.project_tag = None;
        s.estimated_cost = None;
        s.pid = None;
        let mut out = String::new();
        write_csv_row(&mut out, &SessionExportRow::from(&s));
        // Should not contain any literal "None" strings.
        assert!(!out.contains("None"));
        // Should contain consecutive commas where the empty fields land.
        assert!(out.contains(",,"));
    }
}
