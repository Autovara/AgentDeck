use thiserror::Error;

/// Errors raised by core abstractions.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("file watcher backend error: {0}")]
    FileWatcher(String),

    #[error("process source backend error: {0}")]
    ProcessSource(String),
}
