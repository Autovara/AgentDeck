//! Tauri-facing bridge for session export.
//!
//! The dashboard pairs each export call with `tauri-plugin-dialog`'s
//! `save()` to pick a destination path, then invokes the relevant
//! command here with that path. We write the file directly with
//! `std::fs::write`; no extra Tauri filesystem permission is needed
//! because the path was just picked by the user.

use std::path::PathBuf;
use std::sync::Arc;

use agentdeck_export::{export_sessions_csv, export_sessions_json};
use agentdeck_storage::Storage;
use serde::Serialize;

/// Result of one export command.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportResult {
    /// Absolute path the export was written to.
    pub path: PathBuf,
    /// Bytes written to disk.
    pub bytes_written: u64,
    /// Format that was emitted; useful for the UI confirmation toast.
    pub format: &'static str,
}

/// Shared state. Stores the `Arc<Storage>` so we can re-export at any
/// time without going through `MonitorTickState`.
pub struct ExportState {
    storage: Option<Arc<Storage>>,
}

impl ExportState {
    pub fn new(storage: Option<Arc<Storage>>) -> Self {
        Self { storage }
    }
}

pub fn write_sessions_json(state: &ExportState, path: PathBuf) -> Result<ExportResult, String> {
    let storage = state.storage.as_ref().ok_or("storage unavailable")?;
    let body = export_sessions_json(storage).map_err(|e| format!("export failed: {e}"))?;
    write_body(&path, &body, "json")
}

pub fn write_sessions_csv(state: &ExportState, path: PathBuf) -> Result<ExportResult, String> {
    let storage = state.storage.as_ref().ok_or("storage unavailable")?;
    let body = export_sessions_csv(storage).map_err(|e| format!("export failed: {e}"))?;
    write_body(&path, &body, "csv")
}

fn write_body(path: &std::path::Path, body: &str, format: &'static str) -> Result<ExportResult, String> {
    std::fs::write(path, body).map_err(|err| format!("write to {path:?} failed: {err}"))?;
    Ok(ExportResult {
        path: path.to_path_buf(),
        bytes_written: body.len() as u64,
        format,
    })
}
