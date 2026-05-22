//! Typed accessors over the `settings` K/V table for Telegram keys.
//!
//! Keys:
//!
//! - `telegram.enabled` — JSON boolean (`true` / `false`)
//! - `telegram.bot_token` — JSON string (the raw bot token)
//! - `telegram.allowlist` — JSON array (see [`crate::allowlist`])
//!
//! Storing values as JSON (even for primitives) keeps the column
//! shape consistent with the rest of the `settings` table — the
//! bootstrap row at install also uses `json_object(...)`.

use std::sync::Arc;

use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use rusqlite::params;
use serde_json::{json, Value};

use crate::error::TelegramError;

pub const KEY_ENABLED: &str = "telegram.enabled";
pub const KEY_BOT_TOKEN: &str = "telegram.bot_token";
pub const KEY_ALLOWLIST: &str = "telegram.allowlist";

/// Whether the user has opted Telegram on. Defaults to `false` when
/// the key is absent (fresh install).
pub fn get_enabled(storage: &Arc<Storage>) -> Result<bool, TelegramError> {
    match read_value(storage, KEY_ENABLED)? {
        Some(v) => Ok(v.as_bool().unwrap_or(false)),
        None => Ok(false),
    }
}

pub fn set_enabled(
    storage: &Arc<Storage>,
    enabled: bool,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    write_value(storage, KEY_ENABLED, json!(enabled), now)
}

/// The current bot token, if any. Empty strings are treated as
/// "no token" (defensive — the setter rejects empty tokens, but a
/// corrupt row should not surface as a usable token).
pub fn get_token(storage: &Arc<Storage>) -> Result<Option<String>, TelegramError> {
    match read_value(storage, KEY_BOT_TOKEN)? {
        Some(v) => Ok(v
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)),
        None => Ok(None),
    }
}

pub fn set_token(
    storage: &Arc<Storage>,
    token: &str,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return Err(TelegramError::EmptyToken);
    }
    write_value(storage, KEY_BOT_TOKEN, json!(trimmed), now)
}

pub fn clear_token(storage: &Arc<Storage>, now: DateTime<Utc>) -> Result<(), TelegramError> {
    write_value(storage, KEY_BOT_TOKEN, Value::Null, now)
}

/// Read the raw JSON value for `key`, if the row exists.
pub(crate) fn read_value(
    storage: &Arc<Storage>,
    key: &str,
) -> Result<Option<Value>, TelegramError> {
    let raw: Option<String> = storage.with_conn(|conn| {
        conn.query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [key],
            |row| row.get::<_, String>(0),
        )
        .map(Some)
        .or_else(|err| match err {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })
    })?;
    match raw {
        Some(text) => Ok(Some(serde_json::from_str(&text)?)),
        None => Ok(None),
    }
}

/// Upsert `value` for `key`, writing `updated_at` to `now`.
pub(crate) fn write_value(
    storage: &Arc<Storage>,
    key: &str,
    value: Value,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    let text = serde_json::to_string(&value)?;
    let ts = format_ts(now);
    storage.with_conn(|conn| {
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, text, ts],
        )
    })?;
    Ok(())
}

fn format_ts(ts: DateTime<Utc>) -> String {
    ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 15, 0, 0).unwrap()
    }

    fn open() -> Arc<Storage> {
        Arc::new(Storage::open_in_memory().expect("in-memory db"))
    }

    #[test]
    fn enabled_defaults_to_false() {
        let s = open();
        assert!(!get_enabled(&s).unwrap());
    }

    #[test]
    fn enabled_round_trip() {
        let s = open();
        set_enabled(&s, true, now()).unwrap();
        assert!(get_enabled(&s).unwrap());
        set_enabled(&s, false, now()).unwrap();
        assert!(!get_enabled(&s).unwrap());
    }

    #[test]
    fn token_round_trip_trims_whitespace() {
        let s = open();
        assert!(get_token(&s).unwrap().is_none());
        set_token(&s, "  123:ABC  ", now()).unwrap();
        assert_eq!(get_token(&s).unwrap().as_deref(), Some("123:ABC"));
    }

    #[test]
    fn set_empty_token_errors() {
        let s = open();
        assert!(matches!(
            set_token(&s, "   ", now()).unwrap_err(),
            TelegramError::EmptyToken
        ));
    }

    #[test]
    fn clear_token_removes_value() {
        let s = open();
        set_token(&s, "tok", now()).unwrap();
        clear_token(&s, now()).unwrap();
        assert!(get_token(&s).unwrap().is_none());
    }
}
