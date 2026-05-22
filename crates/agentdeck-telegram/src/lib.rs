//! Telegram pairing for AgentDeck.
//!
//! This crate is the user-facing interface to the Telegram bot. The
//! alpha scope is **pairing only**: enabling Telegram, storing a bot
//! token, generating a one-time pairing code, listening for it on the
//! bot, and recording the sender's Telegram user id in an allowlist.
//! Read-only commands land in step 18 (build-plan §15), `/mute` in step
//! 19, `/stop` with confirmation in step 20.
//!
//! Storage:
//!
//! - The bot token and allowlist live in the `settings` table. The
//!   token is plaintext at rest in the alpha. The docs in
//!   `docs/security.md` call this out and beta replaces it with an OS
//!   keychain (Keychain / Credential Manager / libsecret).
//! - Every sensitive transition (enable, disable, token set, paired,
//!   revoked) writes a row to `audit_log` via [`audit`].
//!
//! Bot lifecycle:
//!
//! - When `(enabled && token set)` the Tauri shell starts a teloxide
//!   long-polling task via [`bot::start_bot`]. Disabling, clearing the
//!   token, or replacing the token cleanly aborts the existing task
//!   and spawns a fresh one.
//! - The bot only services pairing in this step; once paired, the
//!   sender is added to the allowlist and a stub reply tells them
//!   commands are coming. Future steps wire the command dispatcher
//!   onto the same handler.

pub mod allowlist;
pub mod audit;
pub mod bot;
pub mod error;
pub mod pairing;
pub mod settings;

pub use allowlist::AllowlistEntry;
pub use bot::{start_bot, BotContext, BotHandle};
pub use error::TelegramError;
pub use pairing::{generate_pairing_code, PairingCode, PairingState, TryConsume};
