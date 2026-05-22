use thiserror::Error;

/// Errors raised by [`crate::export_sessions_json`] /
/// [`crate::export_sessions_csv`].
#[derive(Debug, Error)]
pub enum ExportError {
    #[error("storage error: {0}")]
    Storage(#[from] agentdeck_storage::StorageError),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("session row could not be parsed: {0}")]
    Session(#[from] agentdeck_session::SessionError),

    #[error("JSON serialisation failed: {0}")]
    Json(#[from] serde_json::Error),
}
