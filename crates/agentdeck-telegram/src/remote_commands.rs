//! Write helpers for the `remote_commands` table.
//!
//! Build-plan §6.5 reserves this table for the full Telegram command
//! lifecycle. The alpha only populates it for `/stop`, which is the one
//! command that has a non-trivial pending→executed flow worth tracking
//! at the row level. Read-only commands (`/status`, `/agents`, etc.) are
//! audited in `audit_log` but do not insert here — they are stateless
//! and a row per call would just be noise.

use std::sync::Arc;

use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use rusqlite::params;
use uuid::Uuid;

use crate::error::TelegramError;

const PROVIDER: &str = "telegram";
const ACTION_STOP: &str = "stop";
const REQUESTED_ACTION_STOP: &str = "stop";

/// Insert a fresh `pending` row for `/stop <session_id>`.
pub fn insert_pending_stop(
    storage: &Arc<Storage>,
    command_id: Uuid,
    telegram_user_id: i64,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    let ts = format_ts(now);
    storage.with_conn(|conn| {
        conn.execute(
            "INSERT INTO remote_commands \
                (id, provider, external_user_id, command, session_id, requested_action, \
                 status, requires_confirmation, created_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', 1, ?7)",
            params![
                command_id.to_string(),
                PROVIDER,
                telegram_user_id.to_string(),
                ACTION_STOP,
                session_id.to_string(),
                REQUESTED_ACTION_STOP,
                ts,
            ],
        )
    })?;
    Ok(())
}

/// Mark a previously-pending command as executed. `success` controls
/// whether the row becomes `executed` (signal was delivered) or
/// `failed` (the dispatcher rejected it). `error` is stored verbatim
/// for the `failed` path.
pub fn mark_executed(
    storage: &Arc<Storage>,
    command_id: Uuid,
    success: bool,
    error: Option<&str>,
    now: DateTime<Utc>,
) -> Result<(), TelegramError> {
    let ts = format_ts(now);
    let status = if success { "executed" } else { "failed" };
    storage.with_conn(|conn| {
        conn.execute(
            "UPDATE remote_commands \
             SET status = ?1, confirmed_at = ?2, executed_at = ?2, error = ?3 \
             WHERE id = ?4",
            params![status, ts, error, command_id.to_string()],
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
        Utc.with_ymd_and_hms(2026, 5, 22, 17, 0, 0).unwrap()
    }

    fn open() -> Arc<Storage> {
        Arc::new(Storage::open_in_memory().expect("in-memory db"))
    }

    fn insert_session(storage: &Storage, id: Uuid, status: &str) {
        let ts = now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
        storage
            .with_conn(|conn| {
                conn.execute(
                    "INSERT INTO sessions (id, agent_name, adapter_name, adapter_level, command, \
                        status, status_confidence, start_time, last_seen_time, created_at, updated_at) \
                     VALUES (?1, 'aider', 'aider', 1, 'aider', ?2, 'medium', ?3, ?3, ?3, ?3)",
                    params![id.to_string(), status, ts],
                )
            })
            .unwrap();
    }

    #[test]
    fn insert_pending_round_trip() {
        let storage = open();
        let session_id = Uuid::new_v4();
        insert_session(&storage, session_id, "running");
        let command_id = Uuid::new_v4();
        insert_pending_stop(&storage, command_id, 42, session_id, now()).unwrap();

        let (provider, status, requires, error): (String, String, i64, Option<String>) = storage
            .with_conn(|c| {
                c.query_row(
                    "SELECT provider, status, requires_confirmation, error \
                     FROM remote_commands WHERE id = ?1",
                    [command_id.to_string()],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
            })
            .unwrap();
        assert_eq!(provider, "telegram");
        assert_eq!(status, "pending");
        assert_eq!(requires, 1);
        assert!(error.is_none());
    }

    #[test]
    fn mark_executed_success_updates_status_and_timestamps() {
        let storage = open();
        let session_id = Uuid::new_v4();
        insert_session(&storage, session_id, "running");
        let command_id = Uuid::new_v4();
        insert_pending_stop(&storage, command_id, 42, session_id, now()).unwrap();
        mark_executed(&storage, command_id, true, None, now()).unwrap();

        let (status, confirmed, executed, error): (String, Option<String>, Option<String>, Option<String>) =
            storage
                .with_conn(|c| {
                    c.query_row(
                        "SELECT status, confirmed_at, executed_at, error \
                         FROM remote_commands WHERE id = ?1",
                        [command_id.to_string()],
                        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                    )
                })
                .unwrap();
        assert_eq!(status, "executed");
        assert!(confirmed.is_some());
        assert!(executed.is_some());
        assert!(error.is_none());
    }

    #[test]
    fn mark_executed_failed_records_error() {
        let storage = open();
        let session_id = Uuid::new_v4();
        insert_session(&storage, session_id, "running");
        let command_id = Uuid::new_v4();
        insert_pending_stop(&storage, command_id, 42, session_id, now()).unwrap();
        mark_executed(&storage, command_id, false, Some("kill exited 1"), now()).unwrap();

        let (status, error): (String, Option<String>) = storage
            .with_conn(|c| {
                c.query_row(
                    "SELECT status, error FROM remote_commands WHERE id = ?1",
                    [command_id.to_string()],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
            })
            .unwrap();
        assert_eq!(status, "failed");
        assert_eq!(error.as_deref(), Some("kill exited 1"));
    }
}
