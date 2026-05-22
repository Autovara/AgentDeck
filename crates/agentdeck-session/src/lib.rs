//! Session state machine.
//!
//! This crate is the first layer that *writes* to the SQLite database. It
//! consumes [`agentdeck_adapter::AdapterMatch`]es produced by the adapter
//! registry on every monitor tick and decides, for each match, whether it
//! corresponds to a brand-new session or updates one that's already in the
//! `sessions` table.
//!
//! Identity rule for the alpha:
//!
//! - A session is uniquely identified at runtime by `(adapter_name, pid)`
//!   among non-completed rows.
//! - When a previously-active `(adapter_name, pid)` pair stops appearing
//!   in the match stream it is marked `completed` *immediately*. There is
//!   no grace tick in the alpha; if transient sysinfo blips become a
//!   problem we add one later.
//! - If a new match arrives with the same `(adapter_name, pid)` as a
//!   previously-completed session, it is treated as a brand new session
//!   (OS PID reuse always means a different process).
//! - When several matches in one tick share the same `(adapter_name, pid)`
//!   — possible if two custom-adapter regex rows hit the same process —
//!   the state machine collapses them: one session row per running
//!   process, last writer wins on field conflicts.
//!
//! The whole tick runs inside a single SQLite transaction so an
//! interrupted apply never leaves the database half-written.
//!
//! What this crate **does not** do (those land in later build-plan
//! steps):
//!
//! - Generate or persist attention items.
//! - Compute or attribute cost.
//! - Assign project tags.
//! - Drive the scan schedule. The future monitor core ticks the registry
//!   and hands the results to [`SessionStateMachine::apply`].

mod error;
mod model;
mod repository;
mod state_machine;
mod timestamp;

use std::sync::Arc;

use agentdeck_storage::Storage;

pub use error::SessionError;
pub use model::{
    detected_event_payload, status_changed_event_payload, Session, SessionEvent,
    EVENT_KIND_DETECTED, EVENT_KIND_PROCESS_EXITED, EVENT_KIND_STATUS_CHANGED,
};
pub use state_machine::{SessionStateMachine, StatusChange, TickReport};

/// Read every non-completed session from the database, oldest first.
///
/// Equivalent to the lookup the state machine performs at the start of
/// each tick, exposed so the monitor core can feed the session list to
/// the attention engine without duplicating SQL.
pub fn list_active(storage: &Arc<Storage>) -> Result<Vec<Session>, SessionError> {
    storage
        .with_conn(repository::list_active)
        .map_err(SessionError::from)
}

/// Look up a single session by id. Returns `Ok(None)` when the row does
/// not exist; the row is returned regardless of its status, so a caller
/// (e.g. `/stop`) can distinguish "no such session" from "already
/// completed".
pub fn get_session(
    storage: &Arc<Storage>,
    id: uuid::Uuid,
) -> Result<Option<Session>, SessionError> {
    storage
        .with_conn(|conn| repository::get(conn, id))
        .map_err(SessionError::from)
}
