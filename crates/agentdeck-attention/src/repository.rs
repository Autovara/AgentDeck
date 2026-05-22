//! SQL helpers over `attention_items`.
//!
//! Every function takes a `&Connection` (which a `&Transaction` deref-coerces
//! to) and returns `rusqlite::Result<_>` so callers can use the helpers
//! inside [`agentdeck_storage::Storage::with_conn_mut`]. The engine wraps
//! a tick in a single transaction; user actions (mute, resolve) run as
//! one-shot statements.

use agentdeck_adapter::{Confidence, SessionStatus};
use chrono::{DateTime, Utc};
use rusqlite::{params, types::Type, Connection, Error as SqlError, Row};
use uuid::Uuid;

use crate::action::RecommendedAction;
use crate::model::{AttentionItem, ProposedAttention, SessionRef};
use crate::reason::AttentionReason;
use crate::severity::AttentionSeverity;

const ITEM_COLUMNS: &str = "id, session_id, reason, severity, message, source, confidence, \
     recommended_actions, created_at, resolved_at, muted_until";

const OPEN_WITH_SESSION_SQL: &str = "SELECT \
        a.id, a.session_id, a.reason, a.severity, a.message, a.source, a.confidence, \
        a.recommended_actions, a.created_at, a.resolved_at, a.muted_until, \
        s.agent_name, s.adapter_name, s.status, s.pid, s.repo_path, s.project_tag \
     FROM attention_items a \
     INNER JOIN sessions s ON s.id = a.session_id \
     WHERE a.resolved_at IS NULL \
     ORDER BY \
        CASE a.severity \
            WHEN 'urgent' THEN 0 \
            WHEN 'warn'   THEN 1 \
            WHEN 'info'   THEN 2 \
            ELSE 3 \
        END ASC, \
        a.created_at ASC";

/// Every open (`resolved_at IS NULL`) attention item, oldest first.
pub(crate) fn list_open(conn: &Connection) -> rusqlite::Result<Vec<AttentionItem>> {
    let query = format!(
        "SELECT {ITEM_COLUMNS} FROM attention_items \
         WHERE resolved_at IS NULL \
         ORDER BY created_at ASC",
    );
    let mut stmt = conn.prepare(&query)?;
    let iter = stmt.query_map([], item_from_row)?;
    iter.collect()
}

/// Open items joined with the parent session (for the dashboard).
pub(crate) fn list_open_with_session(
    conn: &Connection,
) -> rusqlite::Result<Vec<(AttentionItem, SessionRef)>> {
    let mut stmt = conn.prepare(OPEN_WITH_SESSION_SQL)?;
    let iter = stmt.query_map([], |row| {
        let item = item_from_row(row)?;
        let session_status_str: String = row.get("status")?;
        let status = SessionStatus::from_db_str(&session_status_str).ok_or_else(|| {
            SqlError::FromSqlConversionFailure(
                0,
                Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown session status {session_status_str:?}"),
                )),
            )
        })?;
        let pid: Option<u32> = row
            .get::<_, Option<i64>>("pid")?
            .and_then(|n| u32::try_from(n).ok());
        let session = SessionRef {
            id: item.session_id,
            agent_name: row.get("agent_name")?,
            adapter_name: row.get("adapter_name")?,
            status,
            pid,
            repo_path: row.get("repo_path")?,
            project_tag: row.get("project_tag")?,
        };
        Ok((item, session))
    })?;
    iter.collect()
}

/// Look up the open item, if any, for a given `(session_id, reason)`.
#[allow(dead_code)] // wired in once the diagnostics card needs it
pub(crate) fn find_open_by_session_reason(
    conn: &Connection,
    session_id: Uuid,
    reason: AttentionReason,
) -> rusqlite::Result<Option<AttentionItem>> {
    let query = format!(
        "SELECT {ITEM_COLUMNS} FROM attention_items \
         WHERE session_id = ?1 AND reason = ?2 AND resolved_at IS NULL \
         LIMIT 1",
    );
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query(params![session_id.to_string(), reason.as_db_str()])?;
    if let Some(row) = rows.next()? {
        Ok(Some(item_from_row(row)?))
    } else {
        Ok(None)
    }
}

/// Fetch a row by id, returning `None` when it does not exist.
pub(crate) fn fetch(conn: &Connection, id: Uuid) -> rusqlite::Result<Option<AttentionItem>> {
    let query = format!("SELECT {ITEM_COLUMNS} FROM attention_items WHERE id = ?1 LIMIT 1",);
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query(params![id.to_string()])?;
    if let Some(row) = rows.next()? {
        Ok(Some(item_from_row(row)?))
    } else {
        Ok(None)
    }
}

pub(crate) fn insert(conn: &Connection, item: &AttentionItem) -> rusqlite::Result<()> {
    let actions_json = encode_actions(&item.recommended_actions);
    let inserted = conn.execute(
        "INSERT INTO attention_items \
             (id, session_id, reason, severity, message, source, confidence, \
              recommended_actions, created_at, resolved_at, muted_until) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            item.id.to_string(),
            item.session_id.to_string(),
            item.reason.as_db_str(),
            item.severity.as_db_str(),
            &item.message,
            &item.source,
            item.confidence.as_db_str(),
            actions_json,
            format_ts(item.created_at),
            item.resolved_at.map(format_ts),
            item.muted_until.map(format_ts),
        ],
    )?;
    debug_assert_eq!(
        inserted, 1,
        "INSERT into attention_items must affect one row"
    );
    Ok(())
}

/// Apply a fresh proposal to an existing open item. `created_at`,
/// `resolved_at`, and `muted_until` are preserved.
pub(crate) fn update_proposal(
    conn: &Connection,
    id: Uuid,
    p: &ProposedAttention,
) -> rusqlite::Result<()> {
    let actions_json = encode_actions(&p.recommended_actions);
    let updated = conn.execute(
        "UPDATE attention_items SET \
            severity = ?1, message = ?2, source = ?3, confidence = ?4, \
            recommended_actions = ?5 \
         WHERE id = ?6 AND resolved_at IS NULL",
        params![
            p.severity.as_db_str(),
            &p.message,
            &p.source,
            p.confidence.as_db_str(),
            actions_json,
            id.to_string(),
        ],
    )?;
    debug_assert_eq!(updated, 1, "UPDATE on an open item must affect one row");
    Ok(())
}

/// Mark an open item resolved at `resolved_at`. Returns the number of
/// rows affected (0 when the id does not exist or the row was already
/// resolved).
pub(crate) fn mark_resolved(
    conn: &Connection,
    id: Uuid,
    resolved_at: DateTime<Utc>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE attention_items SET resolved_at = ?1 \
         WHERE id = ?2 AND resolved_at IS NULL",
        params![format_ts(resolved_at), id.to_string()],
    )
}

/// Set `muted_until` on an open item. Returns the number of rows
/// affected (0 when the id does not exist or the row is already
/// resolved).
pub(crate) fn set_muted_until(
    conn: &Connection,
    id: Uuid,
    until: Option<DateTime<Utc>>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE attention_items SET muted_until = ?1 \
         WHERE id = ?2 AND resolved_at IS NULL",
        params![until.map(format_ts), id.to_string()],
    )
}

fn encode_actions(actions: &[RecommendedAction]) -> String {
    // Vec<RecommendedAction> serialises to a JSON array of snake_case
    // strings; unit-enum encoding cannot fail at runtime.
    serde_json::to_string(actions)
        .expect("RecommendedAction is a unit enum and cannot fail to serialise")
}

fn item_from_row(row: &Row<'_>) -> rusqlite::Result<AttentionItem> {
    let id = uuid_col(row, "id")?;
    let session_id = uuid_col(row, "session_id")?;

    let reason_str: String = row.get("reason")?;
    let reason =
        AttentionReason::from_db_str(&reason_str).ok_or_else(|| enum_err("reason", &reason_str))?;

    let severity_str: String = row.get("severity")?;
    let severity = AttentionSeverity::from_db_str(&severity_str)
        .ok_or_else(|| enum_err("severity", &severity_str))?;

    let confidence_str: String = row.get("confidence")?;
    let confidence = Confidence::from_db_str(&confidence_str)
        .ok_or_else(|| enum_err("confidence", &confidence_str))?;

    let actions_json: Option<String> = row.get("recommended_actions")?;
    let recommended_actions: Vec<RecommendedAction> = match actions_json.as_deref() {
        None | Some("") => Vec::new(),
        Some(json) => serde_json::from_str(json)
            .map_err(|err| SqlError::FromSqlConversionFailure(0, Type::Text, Box::new(err)))?,
    };

    let created_at_str: String = row.get("created_at")?;
    let resolved_at_str: Option<String> = row.get("resolved_at")?;
    let muted_until_str: Option<String> = row.get("muted_until")?;

    Ok(AttentionItem {
        id,
        session_id,
        reason,
        severity,
        message: row.get("message")?,
        source: row.get("source")?,
        confidence,
        recommended_actions,
        created_at: parse_ts(&created_at_str)?,
        resolved_at: resolved_at_str.map(|s| parse_ts(&s)).transpose()?,
        muted_until: muted_until_str.map(|s| parse_ts(&s)).transpose()?,
    })
}

fn uuid_col(row: &Row<'_>, name: &str) -> rusqlite::Result<Uuid> {
    let s: String = row.get(name)?;
    Uuid::parse_str(&s)
        .map_err(|err| SqlError::FromSqlConversionFailure(0, Type::Text, Box::new(err)))
}

fn enum_err(field: &'static str, value: &str) -> SqlError {
    SqlError::FromSqlConversionFailure(
        0,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unknown {field} {value:?}"),
        )),
    )
}

fn format_ts(ts: DateTime<Utc>) -> String {
    // Millisecond precision RFC-3339 with `Z`, matching
    // `agentdeck_session::timestamp::format_ts` so columns sort
    // lexicographically across the two tables.
    ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn parse_ts(s: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| SqlError::FromSqlConversionFailure(0, Type::Text, Box::new(err)))
}
