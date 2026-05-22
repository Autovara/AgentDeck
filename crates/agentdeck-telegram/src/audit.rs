//! Audit log helpers for Telegram-related actions.
//!
//! Every sensitive Telegram transition writes a row to `audit_log` per
//! build-plan §6.6. Pairing and revocation events identify the
//! Telegram user id as the target; token changes deliberately do **not**
//! record the token itself (only the fact that a change happened).
//!
//! These helpers live in this crate (rather than a hypothetical
//! `agentdeck-audit`) because Telegram is the first writer to
//! `audit_log` in the alpha. If a second writer arrives (e.g. /stop
//! dispatch in step 19) it will be straightforward to lift the helpers
//! into a shared crate.

use std::sync::Arc;

use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde_json::{json, Value};

use crate::error::TelegramError;

/// `audit_log.source` value used by every row this module writes.
const SOURCE: &str = "system";

/// `audit_log.actor` value used when AgentDeck itself drives the
/// change (vs. a remote Telegram message). The dashboard is the only
/// actor that calls these helpers today.
const ACTOR_DASHBOARD: &str = "dashboard";
const ACTOR_TELEGRAM_BOT: &str = "telegram-bot";

const RESULT_SUCCESS: &str = "success";

/// Telegram was enabled or disabled from the dashboard.
pub fn write_telegram_enabled(
    storage: &Arc<Storage>,
    enabled: bool,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    write(
        storage,
        ACTOR_DASHBOARD,
        if enabled { "telegram.enabled" } else { "telegram.disabled" },
        None,
        None,
        json!({ "enabled": enabled }),
        now,
    )
}

/// A new bot token was saved (or the existing one was cleared). The
/// token itself is **never** logged.
pub fn write_telegram_token_changed(
    storage: &Arc<Storage>,
    has_token: bool,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    write(
        storage,
        ACTOR_DASHBOARD,
        if has_token {
            "telegram.token_set"
        } else {
            "telegram.token_cleared"
        },
        None,
        None,
        json!({ "has_token": has_token }),
        now,
    )
}

/// A pairing code was generated. The code itself is not logged (it
/// would be useless after the fact; logging it would weaken the
/// single-use invariant if the log leaks).
pub fn write_pairing_code_generated(
    storage: &Arc<Storage>,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    write(
        storage,
        ACTOR_DASHBOARD,
        "telegram.pairing_code_generated",
        None,
        None,
        Value::Null,
        now,
    )
}

/// A Telegram user was successfully paired. Records the user id and
/// (where available) the @username at pairing time.
pub fn write_telegram_paired(
    storage: &Arc<Storage>,
    user_id: i64,
    username: Option<&str>,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    write(
        storage,
        ACTOR_TELEGRAM_BOT,
        "telegram.paired",
        Some("telegram_user"),
        Some(user_id.to_string()),
        json!({ "username": username }),
        now,
    )
}

/// A Telegram user was revoked from the dashboard.
pub fn write_telegram_revoked(
    storage: &Arc<Storage>,
    user_id: i64,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    write(
        storage,
        ACTOR_DASHBOARD,
        "telegram.revoked",
        Some("telegram_user"),
        Some(user_id.to_string()),
        Value::Null,
        now,
    )
}

/// An open attention item was muted via Telegram `/mute`.
pub fn write_telegram_attention_muted(
    storage: &Arc<Storage>,
    attention_id: uuid::Uuid,
    session_id: uuid::Uuid,
    hours: u32,
    until: DateTime<Utc>,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    write(
        storage,
        ACTOR_TELEGRAM_BOT,
        "attention.muted",
        Some("attention_item"),
        Some(attention_id.to_string()),
        json!({
            "session_id": session_id.to_string(),
            "hours": hours,
            "until": until.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            "via": "telegram",
        }),
        now,
    )
}

fn write(
    storage: &Arc<Storage>,
    actor: &str,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<String>,
    metadata: Value,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    let metadata_text = if metadata.is_null() {
        None
    } else {
        Some(metadata.to_string())
    };
    let ts = format_ts(now);
    storage.with_conn(|conn| {
        conn.execute(
            "INSERT INTO audit_log \
                (actor, source, action, target_type, target_id, result, metadata, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                actor,
                SOURCE,
                action,
                target_type,
                target_id,
                RESULT_SUCCESS,
                metadata_text,
                ts,
            ],
        )
    })?;
    Ok(())
}

fn format_ts(ts: DateTime<Utc>) -> String {
    ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}
