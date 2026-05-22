//! Tauri-facing aggregate for the dashboard's "Overview" page.
//!
//! The Overview page is the alpha's first cross-domain surface: it
//! mixes the session list (from `agentdeck-session`) with the open
//! attention items (from `agentdeck-attention`). Rather than fan that
//! out across several Tauri calls, the page reads one consolidated
//! [`OverviewReport`] per refresh.
//!
//! The numbers here are intentionally simple — the page is a status
//! dashboard, not an analytics surface. Cost tracking lands later
//! (build-plan §15 step 18); until then `estimatedCostToday` is always
//! `None` and the dashboard renders `—`.

use agentdeck_adapter::SessionStatus;
use agentdeck_attention::{AttentionSeverity, OpenAttentionEntry};
use agentdeck_session::{list_active, Session};
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::monitor_tick::MonitorTickState;

const RECENT_ATTENTION_LIMIT: usize = 5;

/// Dashboard payload for the Overview page.
///
/// Field order follows the visual layout of the Overview grid:
/// `activeSessions` is the headline, then attention counts (with a
/// severity breakdown), then the in-flight subsets the user might want
/// to act on, then cost. `recentAttention` is the inline "what changed
/// most recently" list at the bottom of the page.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewReport {
    /// `true` when the database opened at startup and we can compose a
    /// real snapshot. `false` triggers the dashboard's error state.
    pub ready: bool,

    /// Count of non-completed `sessions` rows.
    pub active_sessions: usize,
    /// Count of open attention items (resolved_at IS NULL).
    pub attention_total: usize,
    /// `attention_total` split by severity.
    pub attention_urgent: usize,
    pub attention_warn: usize,
    pub attention_info: usize,

    /// Sessions whose status is currently `stalled`. Subset of
    /// `active_sessions`.
    pub stalled_sessions: usize,
    /// Sessions whose status is currently `waiting_for_input`. Always
    /// `0` in the alpha because no Level 2+ adapter ships yet; we keep
    /// the field shape stable so the UI does not need to change.
    pub waiting_sessions: usize,

    /// Estimated cost since local midnight, in the user's pricing
    /// currency. Always `None` in the alpha; cost tracking is build-plan
    /// §15 step 18.
    pub estimated_cost_today: Option<f64>,

    /// Up to [`RECENT_ATTENTION_LIMIT`] open attention items, urgent
    /// first then oldest first. The same `OpenAttentionEntry` shape the
    /// Attention page uses, so the dashboard can render either with one
    /// component.
    pub recent_attention: Vec<OpenAttentionEntry>,

    /// UTC timestamp the report was assembled.
    pub captured_at: DateTime<Utc>,
    /// Engine / storage error, if any.
    pub error: Option<String>,
}

impl OverviewReport {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            ready: false,
            active_sessions: 0,
            attention_total: 0,
            attention_urgent: 0,
            attention_warn: 0,
            attention_info: 0,
            stalled_sessions: 0,
            waiting_sessions: 0,
            estimated_cost_today: None,
            recent_attention: Vec::new(),
            captured_at: Utc::now(),
            error: Some(message.into()),
        }
    }
}

/// Read the current overview snapshot without driving a tick.
///
/// The dashboard pairs this with `run_monitor_tick` to refresh the
/// underlying data when the user clicks "Refresh now".
pub fn snapshot(state: &MonitorTickState) -> OverviewReport {
    let Some(storage) = state.storage() else {
        return OverviewReport::unavailable("storage unavailable");
    };
    let Some(engine) = state.attention() else {
        // Storage exists but attention engine does not — defensive,
        // should not happen because both are wired together.
        return OverviewReport::unavailable("attention engine unavailable");
    };

    let sessions = match list_active(&storage) {
        Ok(s) => s,
        Err(err) => {
            tracing::warn!(error = %err, "overview: list_active failed");
            return OverviewReport::unavailable(format!("session read failed: {err}"));
        }
    };
    let open_attention = match engine.list_open() {
        Ok(items) => items,
        Err(err) => {
            tracing::warn!(error = %err, "overview: attention list_open failed");
            return OverviewReport::unavailable(format!("attention read failed: {err}"));
        }
    };

    let (urgent, warn, info) = severity_breakdown(&open_attention);
    let stalled = count_by_status(&sessions, SessionStatus::Stalled);
    let waiting = count_by_status(&sessions, SessionStatus::WaitingForInput);

    let recent_attention = open_attention
        .iter()
        .take(RECENT_ATTENTION_LIMIT)
        .cloned()
        .collect();

    OverviewReport {
        ready: true,
        active_sessions: sessions.len(),
        attention_total: open_attention.len(),
        attention_urgent: urgent,
        attention_warn: warn,
        attention_info: info,
        stalled_sessions: stalled,
        waiting_sessions: waiting,
        estimated_cost_today: None,
        recent_attention,
        captured_at: Utc::now(),
        error: None,
    }
}

fn severity_breakdown(items: &[OpenAttentionEntry]) -> (usize, usize, usize) {
    let mut urgent = 0;
    let mut warn = 0;
    let mut info = 0;
    for e in items {
        match e.item.severity {
            AttentionSeverity::Urgent => urgent += 1,
            AttentionSeverity::Warn => warn += 1,
            AttentionSeverity::Info => info += 1,
        }
    }
    (urgent, warn, info)
}

fn count_by_status(sessions: &[Session], status: SessionStatus) -> usize {
    sessions.iter().filter(|s| s.status == status).count()
}
