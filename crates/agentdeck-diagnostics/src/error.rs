//! Error types for the adapter-diagnostics repository.

use thiserror::Error;

/// Errors raised by [`record`](crate::record) and [`list`](crate::list).
#[derive(Debug, Error)]
pub enum DiagnosticsError {
    /// A JSON column (`data_sources_used`, `missing_permissions`,
    /// `failure_reasons`, `known_limitations`) failed to encode or
    /// decode.
    #[error("adapter diagnostics JSON could not be parsed: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// A `capability_level` or `confidence` value persisted in the
    /// database does not map to any known enum variant. Indicates a
    /// schema/version mismatch and is treated as fatal for the
    /// affected row.
    #[error("adapter diagnostics row has unknown {field} {value:?}")]
    UnknownEnum { field: &'static str, value: String },

    /// Wraps a low-level `rusqlite::Error`.
    #[error("storage error: {0}")]
    Query(#[from] rusqlite::Error),

    /// Wraps a high-level `agentdeck-storage` error (lock poisoning,
    /// connection-pool failure, etc.).
    #[error("storage layer error: {0}")]
    Storage(#[from] agentdeck_storage::StorageError),
}
