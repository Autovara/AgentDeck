use thiserror::Error;
use uuid::Uuid;

/// Errors raised by the tag CRUD helpers.
#[derive(Debug, Error)]
pub enum TagError {
    #[error("storage error: {0}")]
    Storage(#[from] agentdeck_storage::StorageError),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("a project tag named {0:?} already exists")]
    DuplicateName(String),

    #[error("no project tag found with id {0}")]
    NotFound(Uuid),

    #[error("no project tag named {0:?}")]
    UnknownTagName(String),

    #[error("no session with id {0}")]
    UnknownSession(Uuid),

    #[error("project tag name must be 1..=64 characters; got {0} characters")]
    NameLength(usize),

    #[error("project tag name must not contain control characters")]
    NameControlChar,

    #[error("color must be a #RGB or #RRGGBB hex string; got {0:?}")]
    InvalidColor(String),
}
