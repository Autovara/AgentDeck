//! Tauri-facing wrappers around [`agentdeck_attention::AttentionEngine`].
//!
//! The orchestrator in [`crate::monitor_tick`] owns the engine; this
//! module exposes the three user-visible commands (`get_attention_report`,
//! `mute_attention_item`, `resolve_attention_item`) as plain JSON
//! payloads the dashboard can consume.

use agentdeck_attention::{AttentionEngine, AttentionError, AttentionItem, OpenAttentionEntry};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

use crate::monitor_tick::MonitorTickState;

/// Dashboard payload for the "Attention" card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionReport {
    /// `true` when the storage backend is available; `false` when the
    /// DB failed to open at startup. Mirrors `StorageReport.ready`.
    pub ready: bool,
    /// Open attention items, urgent → warn → info, then oldest first.
    pub items: Vec<OpenAttentionEntry>,
    /// UTC timestamp the report was assembled.
    pub captured_at: DateTime<Utc>,
    /// Engine/storage error, if any.
    pub error: Option<String>,
}

impl AttentionReport {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            ready: false,
            items: Vec::new(),
            captured_at: Utc::now(),
            error: Some(message.into()),
        }
    }
}

/// Read the current open attention items without driving a tick.
pub fn snapshot(state: &MonitorTickState) -> AttentionReport {
    let Some(engine) = state.attention() else {
        return AttentionReport::unavailable("storage unavailable");
    };
    match engine.list_open() {
        Ok(items) => AttentionReport {
            ready: true,
            items,
            captured_at: Utc::now(),
            error: None,
        },
        Err(err) => {
            tracing::warn!(error = %err, "attention list_open failed");
            AttentionReport {
                ready: true,
                items: Vec::new(),
                captured_at: Utc::now(),
                error: Some(stringify_err(err)),
            }
        }
    }
}

/// Set `muted_until` on an open item. `hours` is rounded to integer
/// hours; passing `0` clears the mute.
pub fn mute(state: &MonitorTickState, id: uuid::Uuid, hours: i64) -> Result<AttentionItem, String> {
    let engine = engine_or_err(state)?;
    let until = if hours <= 0 {
        None
    } else {
        Some(Utc::now() + Duration::hours(hours))
    };
    engine.set_mute(id, until).map_err(stringify_err)
}

pub fn resolve(state: &MonitorTickState, id: uuid::Uuid) -> Result<AttentionItem, String> {
    let engine = engine_or_err(state)?;
    engine.resolve(id).map_err(stringify_err)
}

fn engine_or_err(state: &MonitorTickState) -> Result<&AttentionEngine, String> {
    state
        .attention()
        .map(|arc| arc.as_ref())
        .ok_or_else(|| "storage unavailable".to_string())
}

fn stringify_err(err: AttentionError) -> String {
    err.to_string()
}
