use std::path::PathBuf;

use thiserror::Error;

/// Errors emitted by the storage layer.
///
/// The split between `Open`, `Migrate`, and `Query` lets the Tauri shell map
/// startup failures (database creation / migration) to actionable diagnostics
/// without leaking SQLite implementation details.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("could not resolve the default AgentDeck data directory; set AGENTDECK_DATA_DIR to override")]
    NoDataDir,

    #[error("failed to create data directory {path:?}: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to open database at {path:?}: {source}")]
    OpenDatabase {
        path: PathBuf,
        #[source]
        source: rusqlite::Error,
    },

    #[error("failed to apply storage migrations: {0}")]
    Migrate(#[from] rusqlite_migration::Error),

    #[error("query failed: {0}")]
    Query(#[from] rusqlite::Error),

    #[error("storage mutex was poisoned; the previous holder panicked")]
    Poisoned,
}
