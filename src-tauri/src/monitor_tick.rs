//! Single-tick orchestrator: snapshot → adapters → sessions → attention.
//!
//! This is the first concrete monitor-core pipeline in the alpha. The
//! Tauri shell does not yet run it on a schedule; instead the dashboard
//! invokes [`MonitorTickState::run_tick`] via the `run_monitor_tick`
//! command. A future build step will move the orchestrator into a
//! dedicated background task and trigger it on a steady cadence.
//!
//! The orchestrator owns:
//!
//! - an [`AdapterRegistry`] containing every adapter we know how to
//!   build today. Built-in adapters (currently [`AiderAdapter`]) are
//!   registered as stateless values. The custom adapter is rebuilt
//!   from the current `custom_adapters` table contents on every tick
//!   because its definitions can change between ticks (add / remove /
//!   toggle).
//! - the [`SessionStateMachine`] that writes `sessions` / `session_events`.
//! - the [`AttentionEngine`] that writes `attention_items`.
//!
//! Each tick:
//! 1. Takes a fresh process snapshot.
//! 2. Builds the custom adapter from the persisted definitions and
//!    registers it (we keep the registry per-tick rather than caching it
//!    so toggling adapters on the dashboard takes effect immediately).
//! 3. Runs every enabled adapter against the snapshot.
//! 4. Persists the per-adapter `AdapterDiagnostic` rows via
//!    [`agentdeck_diagnostics::record`]. Persistence is best-effort: a
//!    failure is logged but does not abort the tick, so the dashboard's
//!    session and attention surfaces keep updating even if the
//!    diagnostics table is wedged.
//! 5. Calls [`SessionStateMachine::apply`] with the merged match set.
//! 6. Reads the active session list back from storage and feeds the
//!    [`TickReport`] + sessions into [`AttentionEngine::apply`].
//! 7. Bundles the reports into a [`MonitorTickReport`] for the
//!    dashboard.

use std::sync::Arc;

use agentdeck_adapter::AdapterRegistry;
use agentdeck_adapter_aider::AiderAdapter;
use agentdeck_adapter_claude_code::ClaudeCodeAdapter;
use agentdeck_adapter_codex::CodexAdapter;
use agentdeck_adapter_custom::{CustomAdapterRepository, CustomProcessAdapter};
use agentdeck_attention::{AttentionEngine, AttentionTickReport};
use agentdeck_core::{Clock, SystemClock};
use agentdeck_session::{list_active, SessionStateMachine, TickReport};
use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::process_scanner::ProcessScannerState;

/// Payload returned by `run_monitor_tick`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorTickReport {
    /// `true` when the database opened at startup; `false` otherwise.
    pub ready: bool,
    /// UTC timestamp the tick was processed.
    pub tick_at: DateTime<Utc>,
    /// Matches the snapshot timestamp from the process scanner.
    pub snapshot_time: DateTime<Utc>,
    /// Number of adapter matches emitted by the registry.
    pub adapter_matches: usize,
    /// Sessions inserted on this tick.
    pub sessions_created: Vec<Uuid>,
    /// Sessions whose `last_seen_time` was refreshed on this tick.
    pub sessions_updated: Vec<Uuid>,
    /// Sessions marked completed on this tick.
    pub sessions_completed: Vec<Uuid>,
    /// Attention items inserted.
    pub attention_created: Vec<Uuid>,
    /// Attention items updated in-place.
    pub attention_updated: Vec<Uuid>,
    /// Attention items resolved on this tick.
    pub attention_resolved: Vec<Uuid>,
    /// Attention items whose update was skipped because the user has
    /// the item muted.
    pub attention_skipped_muted: Vec<Uuid>,
    /// Fatal error, if any. When set, none of the writes above
    /// happened.
    pub error: Option<String>,
}

/// Tauri-managed state. Holds the storage handle plus the session
/// state machine and the attention engine.
///
/// When `storage` is `None` (DB failed to open) every command returns a
/// degraded report with `ready = false` and `error = Some(...)`.
pub struct MonitorTickState {
    inner: Option<Inner>,
}

struct Inner {
    storage: Arc<Storage>,
    state_machine: Arc<SessionStateMachine>,
    attention: Arc<AttentionEngine>,
}

impl MonitorTickState {
    pub fn new(storage: Option<Arc<Storage>>) -> Self {
        let inner = storage.map(|storage| {
            let clock: Arc<dyn Clock> = Arc::new(SystemClock);
            Inner {
                state_machine: Arc::new(SessionStateMachine::new(storage.clone(), clock.clone())),
                attention: Arc::new(AttentionEngine::new(storage.clone(), clock.clone())),
                storage,
            }
        });
        Self { inner }
    }

    /// Drive one full tick. Always returns; the report carries the
    /// failure mode in `error` if the pipeline could not run.
    pub fn run_tick(&self, scanner: &ProcessScannerState) -> MonitorTickReport {
        let tick_at = Utc::now();
        let Some(inner) = self.inner.as_ref() else {
            return MonitorTickReport::error(tick_at, "storage unavailable");
        };

        let snapshot = scanner.scan_once_for_matching();
        let snapshot_time = snapshot.captured_at;

        // Rebuild the registry from persisted state. Definitions and
        // their enabled flag can change between ticks; rebuilding keeps
        // the shell honest and avoids stale state.
        let custom_defs = match CustomAdapterRepository::new(&inner.storage).list() {
            Ok(defs) => defs,
            Err(err) => {
                return MonitorTickReport::error(
                    tick_at,
                    format!("failed to load custom adapters: {err}"),
                );
            }
        };

        let mut registry = AdapterRegistry::new();
        // Built-in adapters run before the custom adapter so their
        // diagnostics rows appear first in the dashboard table. Order
        // does not affect the session state machine (each match is
        // keyed by `(adapter_name, pid)`).
        registry.register(Box::new(AiderAdapter::new()));
        registry.register(Box::new(CodexAdapter::new()));
        registry.register(Box::new(ClaudeCodeAdapter::new()));
        registry.register(Box::new(CustomProcessAdapter::from_definitions(
            custom_defs,
        )));

        let scan_result = registry.scan_all(&snapshot);
        let adapter_matches = scan_result.matches.len();

        // Best-effort persistence of diagnostics. A failure here must
        // not block the rest of the tick: an empty diagnostics card is
        // recoverable, but losing a session/attention update is not.
        if let Err(err) = agentdeck_diagnostics::record(&inner.storage, &scan_result.diagnostics) {
            tracing::warn!(error = %err, "failed to persist adapter diagnostics");
        }

        let tick_report = match inner
            .state_machine
            .apply(snapshot_time, &scan_result.matches)
        {
            Ok(r) => r,
            Err(err) => {
                return MonitorTickReport::error(tick_at, format!("session apply failed: {err}"));
            }
        };

        let sessions = match list_active(&inner.storage) {
            Ok(s) => s,
            Err(err) => {
                return MonitorTickReport::error(
                    tick_at,
                    format!("could not read sessions back: {err}"),
                );
            }
        };

        let attention_report = match inner.attention.apply(&tick_report, &sessions) {
            Ok(r) => r,
            Err(err) => {
                return MonitorTickReport::error(tick_at, format!("attention apply failed: {err}"));
            }
        };

        MonitorTickReport::from_pipeline(
            tick_at,
            snapshot_time,
            adapter_matches,
            tick_report,
            attention_report,
        )
    }

    /// Borrow the attention engine, e.g. for `mute`/`resolve` commands.
    pub fn attention(&self) -> Option<&Arc<AttentionEngine>> {
        self.inner.as_ref().map(|i| &i.attention)
    }

    /// Borrow the storage handle, for diagnostics that need to read
    /// sessions directly. Currently unused by the shell; reserved for
    /// future command handlers.
    #[allow(dead_code)]
    pub fn storage(&self) -> Option<Arc<Storage>> {
        self.inner.as_ref().map(|i| i.storage.clone())
    }
}

impl MonitorTickReport {
    fn from_pipeline(
        tick_at: DateTime<Utc>,
        snapshot_time: DateTime<Utc>,
        adapter_matches: usize,
        tick: TickReport,
        attention: AttentionTickReport,
    ) -> Self {
        Self {
            ready: true,
            tick_at,
            snapshot_time,
            adapter_matches,
            sessions_created: tick.created,
            sessions_updated: tick.updated,
            sessions_completed: tick.completed,
            attention_created: attention.created,
            attention_updated: attention.updated,
            attention_resolved: attention.resolved,
            attention_skipped_muted: attention.skipped_muted,
            error: None,
        }
    }

    fn error(tick_at: DateTime<Utc>, message: impl Into<String>) -> Self {
        Self {
            ready: false,
            tick_at,
            snapshot_time: tick_at,
            adapter_matches: 0,
            sessions_created: Vec::new(),
            sessions_updated: Vec::new(),
            sessions_completed: Vec::new(),
            attention_created: Vec::new(),
            attention_updated: Vec::new(),
            attention_resolved: Vec::new(),
            attention_skipped_muted: Vec::new(),
            error: Some(message.into()),
        }
    }
}
