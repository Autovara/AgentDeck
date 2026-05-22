//! Low-level SQL helpers over the `sessions` and `session_events` tables.
//!
//! Every function takes a `&Connection` (which a `&Transaction` derefs to)
//! and returns `rusqlite::Result<_>`. The state machine wraps all the
//! calls for a single tick inside one transaction and translates the
//! storage-level errors into [`crate::SessionError`] at its boundary.

use agentdeck_adapter::{AdapterMatch, CapabilityLevel, Confidence, CostKind, SessionStatus};
use chrono::{DateTime, Utc};
use rusqlite::{params, types::Type, Connection, Error as SqlError, Row};
use uuid::Uuid;

use crate::model::Session;
use crate::timestamp::{format_ts, parse_ts};

const SESSION_COLUMNS: &str = "id, agent_name, adapter_name, adapter_level, pid, command, cwd, \
     repo_path, project_tag, status, status_confidence, attention_reason, \
     start_time, last_seen_time, last_activity_time, estimated_cost, \
     cost_kind, created_at, updated_at";

/// Every non-completed session in the database.
///
/// Used by the state machine at the start of every tick to build the
/// `(adapter_name, pid)` lookup. The alpha simply does a full scan;
/// adding an index can wait until we have measurable contention.
pub(crate) fn list_active(conn: &Connection) -> rusqlite::Result<Vec<Session>> {
    let query = format!(
        "SELECT {SESSION_COLUMNS} FROM sessions WHERE status != 'completed' ORDER BY created_at ASC",
    );
    let mut stmt = conn.prepare(&query)?;
    let iter = stmt.query_map([], session_from_row)?;
    iter.collect()
}

/// Insert a brand new session row.
pub(crate) fn insert(conn: &Connection, session: &Session) -> rusqlite::Result<()> {
    let inserted = conn.execute(
        "INSERT INTO sessions \
         (id, agent_name, adapter_name, adapter_level, pid, command, cwd, repo_path, \
          project_tag, status, status_confidence, attention_reason, start_time, \
          last_seen_time, last_activity_time, estimated_cost, cost_kind, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            session.id.to_string(),
            &session.agent_name,
            &session.adapter_name,
            session.adapter_level.as_u8() as i64,
            session.pid.map(|p| p as i64),
            &session.command,
            session.cwd.as_deref(),
            session.repo_path.as_deref(),
            session.project_tag.as_deref(),
            session.status.as_db_str(),
            session.status_confidence.as_db_str(),
            session.attention_reason.as_deref(),
            format_ts(session.start_time),
            format_ts(session.last_seen_time),
            session.last_activity_time.map(format_ts),
            session.estimated_cost,
            session.cost_kind.as_db_str(),
            format_ts(session.created_at),
            format_ts(session.updated_at),
        ],
    )?;
    debug_assert_eq!(inserted, 1, "INSERT into sessions must affect one row");
    Ok(())
}

/// Touch the row for `(adapter_name, pid)` with a fresh match.
///
/// Updates `command`, `cwd`, `repo_path`, `status`, `status_confidence`,
/// `last_seen_time`, and `updated_at`. `start_time` is preserved so the
/// session's age stays accurate across long runs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_from_match(
    conn: &Connection,
    session_id: Uuid,
    m: &AdapterMatch,
    snapshot_time: DateTime<Utc>,
    now: DateTime<Utc>,
) -> rusqlite::Result<()> {
    let updated = conn.execute(
        "UPDATE sessions SET \
            command = ?1, cwd = ?2, repo_path = ?3, \
            status = ?4, status_confidence = ?5, \
            last_seen_time = ?6, updated_at = ?7 \
         WHERE id = ?8",
        params![
            &m.command,
            m.cwd.as_deref(),
            m.repo_path.as_deref(),
            m.status.as_db_str(),
            m.status_confidence.as_db_str(),
            format_ts(snapshot_time),
            format_ts(now),
            session_id.to_string(),
        ],
    )?;
    debug_assert_eq!(
        updated, 1,
        "UPDATE on an active session should affect exactly one row"
    );
    Ok(())
}

/// Mark a session completed and pin its confidence to `High` (process
/// exit is unambiguous).
pub(crate) fn mark_completed(
    conn: &Connection,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> rusqlite::Result<()> {
    let updated = conn.execute(
        "UPDATE sessions SET \
            status = 'completed', status_confidence = 'high', updated_at = ?1 \
         WHERE id = ?2",
        params![format_ts(now), session_id.to_string()],
    )?;
    debug_assert_eq!(updated, 1, "mark_completed should affect one row");
    Ok(())
}

/// Append a row to `session_events`.
pub(crate) fn append_event(
    conn: &Connection,
    session_id: Uuid,
    event_kind: &str,
    payload: Option<&serde_json::Value>,
    now: DateTime<Utc>,
) -> rusqlite::Result<i64> {
    let payload_text: Option<String> = payload.map(|v| v.to_string());
    conn.execute(
        "INSERT INTO session_events (session_id, event_kind, payload, created_at) \
         VALUES (?1, ?2, ?3, ?4)",
        params![
            session_id.to_string(),
            event_kind,
            payload_text,
            format_ts(now),
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

fn session_from_row(row: &Row<'_>) -> rusqlite::Result<Session> {
    let id_str: String = row.get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|err| SqlError::FromSqlConversionFailure(0, Type::Text, Box::new(err)))?;

    let adapter_level_int: i64 = row.get("adapter_level")?;
    let adapter_level_u8 = u8::try_from(adapter_level_int)
        .map_err(|err| SqlError::FromSqlConversionFailure(0, Type::Integer, Box::new(err)))?;
    let adapter_level = CapabilityLevel::from_u8(adapter_level_u8).ok_or_else(|| {
        SqlError::FromSqlConversionFailure(
            0,
            Type::Integer,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown adapter_level {adapter_level_u8}"),
            )),
        )
    })?;

    let pid: Option<u32> = row
        .get::<_, Option<i64>>("pid")?
        .and_then(|n| u32::try_from(n).ok());

    let status_str: String = row.get("status")?;
    let status = SessionStatus::from_db_str(&status_str).ok_or_else(|| {
        SqlError::FromSqlConversionFailure(
            0,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown sessions.status {status_str:?}"),
            )),
        )
    })?;

    let status_confidence_str: String = row.get("status_confidence")?;
    let status_confidence = Confidence::from_db_str(&status_confidence_str).ok_or_else(|| {
        SqlError::FromSqlConversionFailure(
            0,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown status_confidence {status_confidence_str:?}"),
            )),
        )
    })?;

    let cost_kind_str: String = row.get("cost_kind")?;
    let cost_kind = CostKind::from_db_str(&cost_kind_str).ok_or_else(|| {
        SqlError::FromSqlConversionFailure(
            0,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown cost_kind {cost_kind_str:?}"),
            )),
        )
    })?;

    let start_time_str: String = row.get("start_time")?;
    let last_seen_time_str: String = row.get("last_seen_time")?;
    let last_activity_time_str: Option<String> = row.get("last_activity_time")?;
    let created_at_str: String = row.get("created_at")?;
    let updated_at_str: String = row.get("updated_at")?;

    Ok(Session {
        id,
        agent_name: row.get("agent_name")?,
        adapter_name: row.get("adapter_name")?,
        adapter_level,
        pid,
        command: row.get("command")?,
        cwd: row.get("cwd")?,
        repo_path: row.get("repo_path")?,
        project_tag: row.get("project_tag")?,
        status,
        status_confidence,
        attention_reason: row.get("attention_reason")?,
        start_time: parse_ts(&start_time_str)?,
        last_seen_time: parse_ts(&last_seen_time_str)?,
        last_activity_time: last_activity_time_str.map(|s| parse_ts(&s)).transpose()?,
        estimated_cost: row.get("estimated_cost")?,
        cost_kind,
        created_at: parse_ts(&created_at_str)?,
        updated_at: parse_ts(&updated_at_str)?,
    })
}
