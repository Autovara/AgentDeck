//! Tauri-facing wrapper for the Sessions dashboard page.
//!
//! Returns every session in the DB, oldest first, so the page can
//! surface cost / tag history alongside the live set.

use std::sync::Arc;

use agentdeck_session::{list_all, Session};
use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionsReport {
    pub ready: bool,
    pub sessions: Vec<Session>,
    pub captured_at: DateTime<Utc>,
    pub error: Option<String>,
}

impl SessionsReport {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            ready: false,
            sessions: Vec::new(),
            captured_at: Utc::now(),
            error: Some(message.into()),
        }
    }
}

pub struct SessionsState {
    storage: Option<Arc<Storage>>,
}

impl SessionsState {
    pub fn new(storage: Option<Arc<Storage>>) -> Self {
        Self { storage }
    }
}

pub fn snapshot(state: &SessionsState) -> SessionsReport {
    let Some(storage) = state.storage.as_ref() else {
        return SessionsReport::unavailable("storage unavailable");
    };
    match list_all(storage) {
        Ok(sessions) => SessionsReport {
            ready: true,
            sessions,
            captured_at: Utc::now(),
            error: None,
        },
        Err(err) => {
            tracing::warn!(error = %err, "sessions: list_all failed");
            SessionsReport::unavailable(format!("read failed: {err}"))
        }
    }
}
