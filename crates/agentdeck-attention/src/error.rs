//! Error types for the attention engine.

use thiserror::Error;
use uuid::Uuid;

/// Errors raised by the attention engine and repository.
#[derive(Debug, Error)]
pub enum AttentionError {
    #[error("no attention item with id {0}")]
    NotFound(Uuid),

    #[error("attention item {0} is already resolved")]
    AlreadyResolved(Uuid),

    #[error("recommended_actions JSON could not be parsed: {0}")]
    InvalidRecommendedActions(#[from] serde_json::Error),

    #[error("attention row has unknown {field} {value:?}")]
    UnknownEnum { field: &'static str, value: String },

    #[error("storage error: {0}")]
    Query(#[from] rusqlite::Error),

    #[error("storage layer error: {0}")]
    Storage(#[from] agentdeck_storage::StorageError),
}
