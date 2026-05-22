//! Integration tests for the storage schema and migrations.
//!
//! These run against either an in-memory database or a fresh on-disk file
//! under a `tempfile`-managed directory. They exist to catch:
//!
//! - migrations failing to apply,
//! - non-idempotent re-runs (re-opening a migrated DB should be a no-op),
//! - foreign-key or check constraints accidentally being dropped,
//! - the diagnostics surface forgetting about a table.

use std::path::PathBuf;

use agentdeck_storage::Storage;
use chrono::Utc;
use rusqlite::params;
use tempfile::TempDir;

fn rfc3339_now() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

#[test]
fn fresh_in_memory_db_has_latest_schema() {
    let storage = Storage::open_in_memory().expect("in-memory open succeeds");
    let diag = storage.diagnostics().expect("diagnostics succeed");
    assert_eq!(diag.schema_version, 2);
    assert_eq!(diag.applied_migrations.len(), 2);
    assert_eq!(diag.applied_migrations[0].version, 1);
    assert_eq!(diag.applied_migrations[1].version, 2);
    assert_eq!(diag.tables.len(), 10, "all known tables exist");
}

#[test]
fn settings_bootstrap_row_is_seeded() {
    let storage = Storage::open_in_memory().expect("in-memory open succeeds");
    let count: i64 = storage
        .with_conn(|c| {
            c.query_row(
                "SELECT count(*) FROM settings WHERE key = 'schema.bootstrap'",
                [],
                |row| row.get(0),
            )
        })
        .expect("query succeeds");
    assert_eq!(count, 1);
}

#[test]
fn opening_existing_db_is_idempotent() {
    let dir: TempDir = tempfile::tempdir().expect("tempdir");
    let path: PathBuf = dir.path().join("agentdeck.db");

    {
        let storage = Storage::open(&path).expect("first open succeeds");
        assert_eq!(
            storage.diagnostics().expect("diagnostics").schema_version,
            2
        );
    }

    // Reopen; should not error or change schema version.
    let storage = Storage::open(&path).expect("second open succeeds");
    let diag = storage.diagnostics().expect("diagnostics on reopen");
    assert_eq!(diag.schema_version, 2);
    assert!(
        diag.size_bytes.unwrap_or(0) > 0,
        "on-disk DB reports a non-zero file size"
    );
    assert_eq!(diag.journal_mode.to_lowercase(), "wal");
}

#[test]
fn foreign_keys_are_enforced() {
    let storage = Storage::open_in_memory().expect("open");
    let now = rfc3339_now();

    // Cannot insert a session_event for a session that does not exist.
    let err = storage
        .with_conn(|c| {
            c.execute(
                "INSERT INTO session_events (session_id, event_kind, created_at) \
                 VALUES (?1, ?2, ?3)",
                params!["00000000-0000-0000-0000-000000000000", "detected", &now],
            )
        })
        .expect_err("insert with bogus FK must fail");

    let msg = err.to_string().to_lowercase();
    assert!(
        msg.contains("foreign key") || msg.contains("constraint"),
        "error mentions the FK violation: {msg}",
    );
}

#[test]
fn check_constraints_reject_unknown_status() {
    let storage = Storage::open_in_memory().expect("open");
    let now = rfc3339_now();

    let err = storage
        .with_conn(|c| {
            c.execute(
                "INSERT INTO sessions (id, agent_name, adapter_name, adapter_level, \
                 command, status, start_time, last_seen_time, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    "11111111-1111-1111-1111-111111111111",
                    "aider",
                    "aider",
                    1,
                    "aider",
                    "definitely-not-valid",
                    &now,
                    &now,
                    &now,
                    &now,
                ],
            )
        })
        .expect_err("invalid status must fail CHECK constraint");

    let msg = err.to_string().to_lowercase();
    assert!(msg.contains("check"), "error mentions CHECK: {msg}");
}

#[test]
fn round_trips_session_and_event() {
    let storage = Storage::open_in_memory().expect("open");
    let now = rfc3339_now();
    let session_id = uuid::Uuid::new_v4().to_string();

    storage
        .with_conn_mut(|c| {
            let tx = c.transaction()?;
            tx.execute(
                "INSERT INTO sessions (id, agent_name, adapter_name, adapter_level, \
                 command, status, status_confidence, start_time, last_seen_time, \
                 created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    &session_id,
                    "aider",
                    "aider",
                    1,
                    "aider /tmp/foo",
                    "running",
                    "medium",
                    &now,
                    &now,
                    &now,
                    &now,
                ],
            )?;
            tx.execute(
                "INSERT INTO session_events (session_id, event_kind, payload, created_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                params![&session_id, "detected", r#"{"reason":"new pid"}"#, &now],
            )?;
            tx.commit()
        })
        .expect("write succeeds");

    let diag = storage.diagnostics().expect("diagnostics");
    let by_name = |n: &str| {
        diag.tables
            .iter()
            .find(|t| t.name == n)
            .unwrap_or_else(|| panic!("table {n} missing"))
            .row_count
    };
    assert_eq!(by_name("sessions"), 1);
    assert_eq!(by_name("session_events"), 1);
    assert_eq!(by_name("attention_items"), 0);
}

#[test]
fn custom_adapters_table_enforces_match_kind_check() {
    let storage = Storage::open_in_memory().expect("open");
    let now = rfc3339_now();

    let err = storage
        .with_conn(|c| {
            c.execute(
                "INSERT INTO custom_adapters \
                 (id, label, agent_name, match_kind, pattern, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "33333333-3333-3333-3333-333333333333",
                    "my-bot",
                    "my-bot",
                    "not-a-valid-kind",
                    "foo",
                    &now,
                    &now,
                ],
            )
        })
        .expect_err("invalid match_kind must fail CHECK constraint");

    let msg = err.to_string().to_lowercase();
    assert!(msg.contains("check"), "error mentions CHECK: {msg}");
}

#[test]
fn custom_adapters_label_is_unique() {
    let storage = Storage::open_in_memory().expect("open");
    let now = rfc3339_now();

    storage
        .with_conn(|c| {
            c.execute(
                "INSERT INTO custom_adapters \
                 (id, label, agent_name, match_kind, pattern, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "44444444-4444-4444-4444-444444444444",
                    "duplicate",
                    "agent",
                    "name",
                    "agent",
                    &now,
                    &now,
                ],
            )
        })
        .expect("first insert succeeds");

    let err = storage
        .with_conn(|c| {
            c.execute(
                "INSERT INTO custom_adapters \
                 (id, label, agent_name, match_kind, pattern, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    "55555555-5555-5555-5555-555555555555",
                    "duplicate",
                    "agent",
                    "name",
                    "agent",
                    &now,
                    &now,
                ],
            )
        })
        .expect_err("duplicate label must fail UNIQUE");
    let msg = err.to_string().to_lowercase();
    assert!(msg.contains("unique"), "error mentions UNIQUE: {msg}");
}

#[test]
fn settings_kv_overwrites_on_conflict() {
    let storage = Storage::open_in_memory().expect("open");
    let now = rfc3339_now();

    storage
        .with_conn_mut(|c| {
            let tx = c.transaction()?;
            tx.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)",
                params!["telemetry.opt_in", r#"{"enabled":false}"#, &now],
            )?;
            tx.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, \
                                                 updated_at = excluded.updated_at",
                params!["telemetry.opt_in", r#"{"enabled":true}"#, &now],
            )?;
            tx.commit()
        })
        .expect("write");

    let value: String = storage
        .with_conn(|c| {
            c.query_row(
                "SELECT value FROM settings WHERE key = 'telemetry.opt_in'",
                [],
                |row| row.get(0),
            )
        })
        .expect("read");
    assert_eq!(value, r#"{"enabled":true}"#);
}
