//! Tauri-facing bridge for the Diagnostics page's "Adapter diagnostics"
//! card.
//!
//! The orchestrator persists adapter diagnostics inside
//! [`crate::monitor_tick::MonitorTickState::run_tick`]. This module is
//! the read-only side: a single `get_adapter_diagnostics` command that
//! the dashboard pairs with `run_monitor_tick`.

use agentdeck_adapter::AdapterDiagnostic;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::monitor_tick::MonitorTickState;

/// Payload for the Adapter diagnostics card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDiagnosticsReport {
    /// `true` when storage opened at startup and the read succeeded.
    pub ready: bool,
    /// One row per `adapter_name` (the SQL PK), oldest scan first.
    pub items: Vec<AdapterDiagnostic>,
    /// UTC instant the report was assembled.
    pub captured_at: DateTime<Utc>,
    /// Storage / read error, if any. When set, `items` is empty.
    pub error: Option<String>,
}

impl AdapterDiagnosticsReport {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            ready: false,
            items: Vec::new(),
            captured_at: Utc::now(),
            error: Some(message.into()),
        }
    }
}

/// Read every persisted adapter diagnostic.
pub fn snapshot(state: &MonitorTickState) -> AdapterDiagnosticsReport {
    let Some(storage) = state.storage() else {
        return AdapterDiagnosticsReport::unavailable("storage unavailable");
    };
    match agentdeck_diagnostics::list(&storage) {
        Ok(items) => AdapterDiagnosticsReport {
            ready: true,
            items,
            captured_at: Utc::now(),
            error: None,
        },
        Err(err) => {
            tracing::warn!(error = %err, "adapter_diagnostics: list failed");
            AdapterDiagnosticsReport::unavailable(format!("read failed: {err}"))
        }
    }
}
