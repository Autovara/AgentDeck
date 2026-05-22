use std::path::PathBuf;

use crate::StorageError;

/// Filename used for the live database. Bumping this name is a *major*
/// migration event and should be coordinated through `planning/`.
pub const DEFAULT_DB_FILENAME: &str = "agentdeck.db";

/// Resolve the platform-standard AgentDeck data directory.
///
/// Precedence:
///
/// 1. `AGENTDECK_DATA_DIR` (overrides everything; used for dev and tests).
/// 2. `dirs::data_dir()`, joined with a per-OS sub-directory:
///    - Linux: `agentdeck`     (XDG convention is lowercase)
///    - macOS / Windows: `AgentDeck` (the user-visible product name)
///
/// The directory is *not* created here; that is [`Storage::open`]'s job.
pub fn default_data_dir() -> Result<PathBuf, StorageError> {
    if let Some(custom) = std::env::var_os("AGENTDECK_DATA_DIR") {
        return Ok(PathBuf::from(custom));
    }

    let base = dirs::data_dir().ok_or(StorageError::NoDataDir)?;

    let subdir = if cfg!(target_os = "linux") {
        "agentdeck"
    } else {
        "AgentDeck"
    };

    Ok(base.join(subdir))
}

/// Resolve the default path of the live database, including filename.
pub fn default_db_path() -> Result<PathBuf, StorageError> {
    Ok(default_data_dir()?.join(DEFAULT_DB_FILENAME))
}
