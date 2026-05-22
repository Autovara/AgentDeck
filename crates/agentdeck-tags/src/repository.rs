//! SQL helpers over the `project_tags` table and the
//! `sessions.project_tag` column.

use chrono::{DateTime, Utc};
use rusqlite::{params, types::Type, Connection, Error as SqlError, Row};
use uuid::Uuid;

use crate::model::ProjectTag;
use crate::timestamp::{format_ts, parse_ts};

const COLUMNS: &str = "id, name, color, notes, created_at, updated_at";

pub(crate) fn list(conn: &Connection) -> rusqlite::Result<Vec<ProjectTag>> {
    let query = format!("SELECT {COLUMNS} FROM project_tags ORDER BY created_at ASC");
    let mut stmt = conn.prepare(&query)?;
    let iter = stmt.query_map([], tag_from_row)?;
    iter.collect()
}

pub(crate) fn get(conn: &Connection, id: Uuid) -> rusqlite::Result<Option<ProjectTag>> {
    let query = format!("SELECT {COLUMNS} FROM project_tags WHERE id = ?1");
    let mut stmt = conn.prepare(&query)?;
    let mut rows = stmt.query([id.to_string()])?;
    match rows.next()? {
        Some(row) => Ok(Some(tag_from_row(row)?)),
        None => Ok(None),
    }
}

pub(crate) fn insert(conn: &Connection, tag: &ProjectTag) -> rusqlite::Result<()> {
    let inserted = conn.execute(
        "INSERT INTO project_tags (id, name, color, notes, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            tag.id.to_string(),
            &tag.name,
            tag.color.as_deref(),
            tag.notes.as_deref(),
            format_ts(tag.created_at),
            format_ts(tag.updated_at),
        ],
    )?;
    debug_assert_eq!(inserted, 1, "INSERT into project_tags must affect one row");
    Ok(())
}

pub(crate) fn delete(conn: &Connection, id: Uuid) -> rusqlite::Result<usize> {
    conn.execute("DELETE FROM project_tags WHERE id = ?1", [id.to_string()])
}

/// Check whether a tag with the given name exists.
pub(crate) fn tag_exists_by_name(conn: &Connection, name: &str) -> rusqlite::Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM project_tags WHERE name = ?1",
        [name],
        |r| r.get(0),
    )?;
    Ok(count > 0)
}

/// Write `sessions.project_tag` for one session.
pub(crate) fn set_session_tag(
    conn: &Connection,
    session_id: Uuid,
    tag_name: Option<&str>,
    now: DateTime<Utc>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE sessions SET project_tag = ?1, updated_at = ?2 WHERE id = ?3",
        params![tag_name, format_ts(now), session_id.to_string()],
    )
}

/// Null out `sessions.project_tag` for every session currently tagged
/// `name`. Used during tag deletion so we never leave dangling labels.
pub(crate) fn clear_sessions_tagged(
    conn: &Connection,
    name: &str,
    now: DateTime<Utc>,
) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE sessions SET project_tag = NULL, updated_at = ?1 WHERE project_tag = ?2",
        params![format_ts(now), name],
    )
}

fn tag_from_row(row: &Row<'_>) -> rusqlite::Result<ProjectTag> {
    let id_str: String = row.get("id")?;
    let id = Uuid::parse_str(&id_str)
        .map_err(|err| SqlError::FromSqlConversionFailure(0, Type::Text, Box::new(err)))?;
    let created_at_str: String = row.get("created_at")?;
    let updated_at_str: String = row.get("updated_at")?;
    Ok(ProjectTag {
        id,
        name: row.get("name")?,
        color: row.get("color")?,
        notes: row.get("notes")?,
        created_at: parse_ts(&created_at_str)?,
        updated_at: parse_ts(&updated_at_str)?,
    })
}
