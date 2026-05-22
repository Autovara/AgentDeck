//! Bridge between the Tauri shell and the `agentdeck-storage` crate.
//!
//! The shell owns one [`Storage`] instance for the lifetime of the process.
//! It is *not* the long-term home for the database handle — that will move
//! into the monitor core in a later step — but holding it here today gives
//! the dashboard a real diagnostics surface from day one, and proves the
//! schema and migration path on every cold start.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agentdeck_storage::{
    default_db_path, MigrationSummary, Storage, StorageDiagnostics, StorageError, TableStat,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Runtime};

/// Outcome of opening the database at startup. Mirrors the TypeScript
/// `StorageReport` interface in `src/lib/tauri.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageReport {
    /// `true` when the storage layer is ready for reads and writes.
    pub ready: bool,
    /// Absolute path of the database file. Reported even on failure so the
    /// user can investigate.
    pub path: PathBuf,
    /// `PRAGMA user_version` after migrations applied.
    pub schema_version: Option<u32>,
    /// On-disk size in bytes (`None` for in-memory or when the file is
    /// missing).
    pub size_bytes: Option<u64>,
    /// SQLite journal mode (expected to be `wal` after configure_pragmas).
    pub journal_mode: Option<String>,
    /// Migrations the running binary knows about.
    pub applied_migrations: Vec<MigrationSummary>,
    /// Row counts per known table.
    pub tables: Vec<TableSnapshot>,
    /// When the storage layer was last queried.
    pub captured_at: DateTime<Utc>,
    /// `Some(message)` when initialisation or refresh failed. The dashboard
    /// shows this verbatim.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableSnapshot {
    pub name: &'static str,
    pub row_count: i64,
}

/// Cached storage handle plus the most recent [`StorageReport`]. Held as
/// app-managed state so the `get_storage_report` Tauri command can return
/// fresh data on every call.
pub struct StorageReportState {
    inner: Mutex<StorageReportInner>,
}

struct StorageReportInner {
    storage: Option<Arc<Storage>>,
    last_report: StorageReport,
}

impl StorageReportState {
    pub fn new(report: StorageReportWithHandle) -> Self {
        Self {
            inner: Mutex::new(StorageReportInner {
                storage: report.storage,
                last_report: report.report,
            }),
        }
    }

    /// Clone the shared storage handle, if the database opened successfully.
    /// Other Tauri-managed state (e.g. the custom adapter state) hold their
    /// own clone of the same `Arc<Storage>`. Currently unused in the shell
    /// (the Arc is wired in at setup time), kept as a stable accessor for
    /// future Tauri commands and tests.
    #[allow(dead_code)]
    pub fn storage(&self) -> Option<Arc<Storage>> {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.storage.as_ref().cloned())
    }

    /// Return the most recent report. If the storage handle is alive we
    /// refresh it first; otherwise we return the cached (probably failed)
    /// startup snapshot.
    pub fn refresh(&self) -> StorageReport {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };

        if let Some(storage) = guard.storage.clone() {
            match storage.diagnostics() {
                Ok(diag) => {
                    guard.last_report = report_from_diagnostics(diag);
                }
                Err(err) => {
                    tracing::warn!(error = %err, "storage diagnostics refresh failed");
                    guard.last_report.error = Some(err.to_string());
                    guard.last_report.captured_at = Utc::now();
                }
            }
        }
        guard.last_report.clone()
    }
}

/// Returned by [`initialise`]; carries both the handle (kept alive for the
/// lifetime of the app) and the initial diagnostic snapshot.
pub struct StorageReportWithHandle {
    pub storage: Option<Arc<Storage>>,
    pub report: StorageReport,
}

/// Open the database, run migrations, and produce the initial diagnostic
/// snapshot. Failures degrade gracefully: the shell continues to run and the
/// dashboard surfaces the error.
pub fn initialise<R: Runtime>(_app: &AppHandle<R>) -> StorageReportWithHandle {
    let path = match default_db_path() {
        Ok(p) => p,
        Err(err) => {
            return StorageReportWithHandle {
                storage: None,
                report: failure_report(PathBuf::from("?"), err),
            };
        }
    };

    tracing::info!(path = %path.display(), "Opening AgentDeck SQLite database");

    match Storage::open(&path) {
        Ok(storage) => {
            let storage = Arc::new(storage);
            match storage.diagnostics() {
                Ok(diag) => {
                    tracing::info!(
                        schema = diag.schema_version,
                        journal_mode = diag.journal_mode.as_str(),
                        size_bytes = ?diag.size_bytes,
                        "Storage initialised successfully",
                    );
                    StorageReportWithHandle {
                        storage: Some(storage),
                        report: report_from_diagnostics(diag),
                    }
                }
                Err(err) => {
                    tracing::error!(error = %err, "Storage diagnostics failed after open");
                    StorageReportWithHandle {
                        storage: Some(storage),
                        report: failure_report(path, err),
                    }
                }
            }
        }
        Err(err) => {
            tracing::error!(error = %err, path = %path.display(), "Storage open failed");
            StorageReportWithHandle {
                storage: None,
                report: failure_report(path, err),
            }
        }
    }
}

fn report_from_diagnostics(diag: StorageDiagnostics) -> StorageReport {
    StorageReport {
        ready: true,
        path: diag.path,
        schema_version: Some(diag.schema_version),
        size_bytes: diag.size_bytes,
        journal_mode: Some(diag.journal_mode),
        applied_migrations: diag.applied_migrations,
        tables: diag.tables.into_iter().map(table_snapshot).collect(),
        captured_at: diag.captured_at,
        error: None,
    }
}

fn table_snapshot(stat: TableStat) -> TableSnapshot {
    TableSnapshot {
        name: stat.name,
        row_count: stat.row_count,
    }
}

fn failure_report(path: PathBuf, err: StorageError) -> StorageReport {
    StorageReport {
        ready: false,
        path,
        schema_version: None,
        size_bytes: None,
        journal_mode: None,
        applied_migrations: Vec::new(),
        tables: Vec::new(),
        captured_at: Utc::now(),
        error: Some(err.to_string()),
    }
}
