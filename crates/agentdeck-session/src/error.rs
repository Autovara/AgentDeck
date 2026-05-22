use thiserror::Error;

/// Errors emitted by the session state machine.
///
/// All storage failures surface as [`SessionError::Storage`]; row-parse
/// failures inside the repository are wrapped as rusqlite conversion
/// errors before they reach this layer, so callers only need to
/// distinguish "the database was unhappy" from "the storage handle was
/// poisoned" (the latter is fatal).
#[derive(Debug, Error)]
pub enum SessionError {
    #[error("storage failure during session state machine apply: {0}")]
    Storage(#[from] agentdeck_storage::StorageError),
}
