use thiserror::Error;

/// Errors raised by [`crate::settings`], [`crate::allowlist`],
/// [`crate::pairing`], and [`crate::audit`].
///
/// The bot module returns its own teloxide-rooted errors via
/// [`crate::bot`] callbacks; bot-runtime failures are logged via
/// `tracing` and do not surface here.
#[derive(Debug, Error)]
pub enum TelegramError {
    #[error("storage error: {0}")]
    Storage(#[from] agentdeck_storage::StorageError),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("settings JSON could not be parsed: {0}")]
    InvalidJson(#[from] serde_json::Error),

    #[error("Telegram bot token must not be empty")]
    EmptyToken,

    #[error("no Telegram user is paired with id {0}")]
    UnknownUser(i64),
}
