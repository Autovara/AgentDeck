//! Allowlist of paired Telegram users.
//!
//! Stored as a JSON array in the `settings` table under
//! [`crate::settings::KEY_ALLOWLIST`]. A flat JSON column is fine for
//! the alpha (≤10 users in practice); a dedicated `telegram_users`
//! table is only worth it once we want per-row audit columns or
//! atomic constraint-driven uniqueness.

use std::sync::Arc;

use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::TelegramError;
use crate::settings::{read_value, write_value, KEY_ALLOWLIST};

/// One paired Telegram user. Field names use camelCase so the JSON
/// matches what the dashboard already expects from other AgentDeck
/// payloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowlistEntry {
    /// Telegram user id (the numeric one, not a username). Telegram
    /// user ids fit in i64.
    pub user_id: i64,
    /// `@username` at pairing time, if Telegram exposed it. May be
    /// `None` for users who have no @-handle. Never used for
    /// authorisation; the id is the source of truth.
    pub username: Option<String>,
    /// When the user was paired.
    pub paired_at: DateTime<Utc>,
}

/// Load every paired entry. Returns an empty vector when no allowlist
/// row exists yet (fresh install).
pub fn load(storage: &Arc<Storage>) -> Result<Vec<AllowlistEntry>, TelegramError> {
    match read_value(storage, KEY_ALLOWLIST)? {
        Some(Value::Array(_)) | Some(Value::Object(_)) => {
            let entries: Vec<AllowlistEntry> =
                serde_json::from_value(read_value(storage, KEY_ALLOWLIST)?.unwrap())?;
            Ok(entries)
        }
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(other) => {
            tracing::warn!(?other, "telegram.allowlist has unexpected shape; treating as empty");
            Ok(Vec::new())
        }
    }
}

/// Add `entry` to the allowlist. If the user is already paired the
/// existing row is replaced (idempotent re-pairing). Returns the
/// allowlist after the change.
pub fn add(
    storage: &Arc<Storage>,
    entry: AllowlistEntry,
    now: DateTime<Utc>,
) -> Result<Vec<AllowlistEntry>, TelegramError> {
    let mut entries = load(storage)?;
    entries.retain(|e| e.user_id != entry.user_id);
    entries.push(entry);
    persist(storage, &entries, now)?;
    Ok(entries)
}

/// Remove the user by id. Returns the new allowlist.
pub fn remove(
    storage: &Arc<Storage>,
    user_id: i64,
    now: DateTime<Utc>,
) -> Result<Vec<AllowlistEntry>, TelegramError> {
    let mut entries = load(storage)?;
    let before = entries.len();
    entries.retain(|e| e.user_id != user_id);
    if entries.len() == before {
        return Err(TelegramError::UnknownUser(user_id));
    }
    persist(storage, &entries, now)?;
    Ok(entries)
}

/// Replace the entire allowlist (used by [`clear`]).
pub fn clear(storage: &Arc<Storage>, now: DateTime<Utc>) -> Result<(), TelegramError> {
    persist(storage, &Vec::new(), now)
}

/// True when the given user id is currently allowed to interact with
/// the bot. Uses [`load`] under the hood; for the alpha's low call
/// rate that is plenty fast.
pub fn contains(storage: &Arc<Storage>, user_id: i64) -> Result<bool, TelegramError> {
    let entries = load(storage)?;
    Ok(entries.iter().any(|e| e.user_id == user_id))
}

fn persist(
    storage: &Arc<Storage>,
    entries: &[AllowlistEntry],
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    let value = serde_json::to_value(entries)?;
    write_value(storage, KEY_ALLOWLIST, value, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 15, 0, 0).unwrap() + chrono::Duration::seconds(secs)
    }

    fn open() -> Arc<Storage> {
        Arc::new(Storage::open_in_memory().expect("in-memory db"))
    }

    fn entry(id: i64, username: Option<&str>) -> AllowlistEntry {
        AllowlistEntry {
            user_id: id,
            username: username.map(str::to_string),
            paired_at: ts(0),
        }
    }

    #[test]
    fn empty_by_default() {
        let s = open();
        assert!(load(&s).unwrap().is_empty());
        assert!(!contains(&s, 1).unwrap());
    }

    #[test]
    fn add_round_trips_through_storage() {
        let s = open();
        let entries = add(&s, entry(123, Some("alice")), ts(0)).unwrap();
        assert_eq!(entries.len(), 1);
        let reload = load(&s).unwrap();
        assert_eq!(reload, entries);
        assert!(contains(&s, 123).unwrap());
    }

    #[test]
    fn add_is_idempotent_per_user() {
        let s = open();
        add(&s, entry(1, Some("a")), ts(0)).unwrap();
        add(&s, entry(1, Some("a-renamed")), ts(60)).unwrap();
        let entries = load(&s).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].username.as_deref(), Some("a-renamed"));
    }

    #[test]
    fn remove_existing_user() {
        let s = open();
        add(&s, entry(1, None), ts(0)).unwrap();
        add(&s, entry(2, None), ts(0)).unwrap();
        let entries = remove(&s, 1, ts(0)).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].user_id, 2);
        assert!(!contains(&s, 1).unwrap());
    }

    #[test]
    fn remove_unknown_user_errors() {
        let s = open();
        assert!(matches!(
            remove(&s, 99, ts(0)).unwrap_err(),
            TelegramError::UnknownUser(99)
        ));
    }

    #[test]
    fn clear_empties_the_allowlist() {
        let s = open();
        add(&s, entry(1, None), ts(0)).unwrap();
        clear(&s, ts(0)).unwrap();
        assert!(load(&s).unwrap().is_empty());
    }
}
