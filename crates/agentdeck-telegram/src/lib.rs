//! Telegram bot for AgentDeck.
//!
//! Build-plan §15 step 17 added pairing; step 18 added the read-only
//! commands (`/help`, `/status`, `/agents`, `/attention`, `/session`)
//! and a per-user rate limiter; `/mute` and `/stop` land in 19 and 20.
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
//! - The bot routes pairing first (no rate limit), then allowlist-
//!   gates everything else, then rate-limits, then parses + dispatches
//!   slash commands via the pure [`commands`] module.

pub mod allowlist;
pub mod audit;
pub mod bot;
pub mod commands;
pub mod error;
pub mod pairing;
pub mod rate_limit;
pub mod settings;

pub use allowlist::AllowlistEntry;
pub use bot::{start_bot, BotContext, BotHandle};
pub use commands::{
    format_agents, format_attention, format_help, format_mute_all_failed,
    format_mute_no_attention, format_mute_out_of_range, format_mute_success, format_mute_usage,
    format_rate_limited, format_session_detail, format_session_not_found, format_session_usage,
    format_status, format_unknown_command, lookup_session, parse_command, short_session_id,
    BotCommand, SessionLookup, DEFAULT_MUTE_HOURS, MAX_MUTE_HOURS,
};
pub use error::TelegramError;
pub use pairing::{generate_pairing_code, PairingCode, PairingState, TryConsume};
pub use rate_limit::{RateLimitOutcome, RateLimiter};
