use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::schema::MigrationSummary;

/// Diagnostic snapshot of the storage layer at the moment [`Storage::diagnostics`]
/// is called.
///
/// The shape is stable and is serialised directly to the dashboard. Add new
/// fields as `Option<T>` so older builds of the dashboard keep rendering.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageDiagnostics {
    /// Absolute path to the live database file.
    pub path: PathBuf,
    /// File size in bytes; `None` while the file does not yet exist (only
    /// observable from an in-memory database).
    pub size_bytes: Option<u64>,
    /// `PRAGMA user_version` — the highest migration applied to this DB.
    pub schema_version: u32,
    /// `PRAGMA journal_mode`; the storage layer aims for `wal`.
    pub journal_mode: String,
    /// Migrations that the running binary knows about.
    pub applied_migrations: Vec<MigrationSummary>,
    /// Row counts per known table, useful as a quick sanity check after a
    /// fresh install.
    pub tables: Vec<TableStat>,
    /// UTC instant the diagnostic was computed.
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStat {
    pub name: &'static str,
    pub row_count: i64,
}

/// The tables the diagnostics card knows how to count. New tables added via
/// a migration must be appended here.
pub(crate) const KNOWN_TABLES: &[&str] = &[
    "sessions",
    "session_events",
    "attention_items",
    "adapter_diagnostics",
    "usage_records",
    "project_tags",
    "remote_commands",
    "audit_log",
    "settings",
];
