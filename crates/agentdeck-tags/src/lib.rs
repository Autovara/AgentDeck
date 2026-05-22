//! Project / client tags for AgentDeck sessions.
//!
//! Two pieces of state live here:
//!
//! 1. CRUD over the `project_tags` table (name + optional color + notes).
//! 2. Per-session tag assignment — writes `sessions.project_tag` to the
//!    tag's `name`. The session column intentionally references the
//!    name rather than the tag's UUID so the dashboard can render a tag
//!    label without a join.
//!
//! The alpha keeps tagging manual: an auto-rule engine (regex over
//! `cwd` / `repo_path`) is a later build step.

mod error;
mod model;
mod repository;
mod timestamp;

use std::sync::Arc;

use agentdeck_storage::Storage;
use chrono::Utc;
use uuid::Uuid;

pub use error::TagError;
pub use model::{NewProjectTag, ProjectTag};

/// List every project tag, oldest first.
pub fn list(storage: &Arc<Storage>) -> Result<Vec<ProjectTag>, TagError> {
    storage.with_conn(repository::list).map_err(TagError::from)
}

/// Look up one tag by id. `Ok(None)` when the row does not exist.
pub fn get(storage: &Arc<Storage>, id: Uuid) -> Result<Option<ProjectTag>, TagError> {
    storage
        .with_conn(|conn| repository::get(conn, id))
        .map_err(TagError::from)
}

/// Create a fresh tag. Validates the input and returns the persisted
/// row (including the generated id and timestamps).
pub fn create(storage: &Arc<Storage>, input: NewProjectTag) -> Result<ProjectTag, TagError> {
    let validated = input.validate()?;
    let now = Utc::now();
    let id = Uuid::new_v4();
    let tag = ProjectTag {
        id,
        name: validated.name,
        color: validated.color,
        notes: validated.notes,
        created_at: now,
        updated_at: now,
    };
    storage
        .with_conn(|conn| repository::insert(conn, &tag))
        .map_err(|err| match err {
            agentdeck_storage::StorageError::Query(e) if is_unique_violation(&e) => {
                TagError::DuplicateName(tag.name.clone())
            }
            other => TagError::Storage(other),
        })?;
    Ok(tag)
}

/// Delete a tag by id. Also clears the tag name from any session row
/// that referenced it, so dangling `sessions.project_tag` values do
/// not survive.
pub fn delete(storage: &Arc<Storage>, id: Uuid) -> Result<(), TagError> {
    let now = Utc::now();
    let tag = get(storage, id)?.ok_or(TagError::NotFound(id))?;
    storage.with_conn_mut(|conn| {
        let tx = conn.transaction()?;
        repository::clear_sessions_tagged(&tx, &tag.name, now)?;
        repository::delete(&tx, id)?;
        tx.commit()
    })?;
    Ok(())
}

/// Set `sessions.project_tag` for one session.
pub fn assign_session_tag(
    storage: &Arc<Storage>,
    session_id: Uuid,
    tag_name: &str,
) -> Result<(), TagError> {
    let now = Utc::now();
    // Validate the tag exists; we keep the soft FK only at the
    // dashboard layer (the SQL column is plain text) so this check is
    // best-effort but worth doing.
    let exists = storage
        .with_conn(|conn| repository::tag_exists_by_name(conn, tag_name))?;
    if !exists {
        return Err(TagError::UnknownTagName(tag_name.to_string()));
    }
    let affected = storage.with_conn(|conn| {
        repository::set_session_tag(conn, session_id, Some(tag_name), now)
    })?;
    if affected == 0 {
        return Err(TagError::UnknownSession(session_id));
    }
    Ok(())
}

/// Clear `sessions.project_tag` for one session.
pub fn clear_session_tag(storage: &Arc<Storage>, session_id: Uuid) -> Result<(), TagError> {
    let now = Utc::now();
    let affected =
        storage.with_conn(|conn| repository::set_session_tag(conn, session_id, None, now))?;
    if affected == 0 {
        return Err(TagError::UnknownSession(session_id));
    }
    Ok(())
}

fn is_unique_violation(err: &rusqlite::Error) -> bool {
    matches!(
        err,
        rusqlite::Error::SqliteFailure(ffi, _)
            if ffi.code == rusqlite::ErrorCode::ConstraintViolation
                && ffi.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}
