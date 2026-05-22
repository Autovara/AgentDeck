//! Background tick scheduler driving the tray menu + notifications.
//!
//! Without a scheduler the tray surface is stale whenever the dashboard
//! window is closed — defeating the point of having a tray at all. The
//! scheduler is intentionally minimal in the alpha:
//!
//! - One tokio interval task spawned at startup.
//! - 15-second cadence, the same trade-off between freshness and
//!   battery cost the rest of the alpha targets.
//! - Each tick:
//!   1. Drives [`crate::monitor_tick::MonitorTickState::run_tick`]
//!      (snapshot → adapters → sessions → attention → diagnostics).
//!   2. Reads the resulting open-attention list once, reuses it for
//!      both the tray menu rebuild and the urgent-notification scan.
//!   3. Updates the tray menu if a tray exists.
//!   4. Sends desktop notifications for newly-created urgent items
//!      unless [`crate::alerts::AlertsPausedState`] is on.
//!
//! The scheduler runs regardless of tray availability — desktop
//! notifications are still useful when the tray is hidden behind a
//! fallback. Likewise, [`run_once`] is exposed so the tray's "Refresh"
//! menu item and (later) other surfaces can trigger the same pipeline
//! on demand.

use std::sync::Arc;
use std::time::Duration;

use agentdeck_attention::OpenAttentionEntry;
use agentdeck_session::{list_active, Session};
use agentdeck_storage::Storage;
use tauri::{AppHandle, Manager, Runtime};
use tokio::time::{interval_at, Instant, MissedTickBehavior};

use crate::alerts::AlertsPausedState;
use crate::monitor_tick::{MonitorTickReport, MonitorTickState};
use crate::notifications::notify_for_new_urgent;
use crate::process_scanner::ProcessScannerState;
use crate::tray::{set_menu_from_snapshot, TrayKeeper};
use crate::tray_menu::TrayMenuSnapshot;

/// Interval between scheduled ticks. 15 seconds is a balance between
/// dashboard freshness and battery cost; the user can also click the
/// "Refresh" tray item to force a tick.
const TICK_INTERVAL: Duration = Duration::from_secs(15);

/// Spawn the background scheduler. Returns immediately; the loop runs
/// for the lifetime of the app.
pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        // Run an immediate tick at startup so the tray is populated by
        // the time the user sees it.
        run_once(&app).await;

        let mut ticker = interval_at(Instant::now() + TICK_INTERVAL, TICK_INTERVAL);
        // If the system is asleep / suspended, don't try to "catch up"
        // by firing back-to-back ticks when it wakes — Skip drops missed
        // ticks and resumes at the regular cadence.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;
            run_once(&app).await;
        }
    });
}

/// Run one tick + tray refresh + notification pass. Public so the
/// "Refresh" menu item can drive the same pipeline.
pub async fn run_once<R: Runtime>(app: &AppHandle<R>) {
    // Bail out early if state isn't managed yet (e.g. during teardown).
    if app.try_state::<MonitorTickState>().is_none()
        || app.try_state::<ProcessScannerState>().is_none()
    {
        tracing::debug!("tray scheduler: monitor state not managed; skipping tick");
        return;
    }

    // The tick is synchronous (sysinfo + several SQL statements);
    // off-thread it via spawn_blocking. Re-fetch state inside the
    // closure because `tauri::State<'_, T>` is not `Send`.
    let app_for_tick = app.clone();
    let report: MonitorTickReport = match tauri::async_runtime::spawn_blocking(move || {
        let tick_state = app_for_tick.state::<MonitorTickState>();
        let scanner = app_for_tick.state::<ProcessScannerState>();
        tick_state.run_tick(&scanner)
    })
    .await
    {
        Ok(r) => r,
        Err(err) => {
            tracing::warn!(error = %err, "tray scheduler: tick task panicked");
            return;
        }
    };

    if !report.ready {
        tracing::debug!(
            error = report.error.as_deref().unwrap_or("(none)"),
            "tray scheduler: tick reported not ready"
        );
        update_tray_menu(app, TrayMenuSnapshot::unavailable(alerts_paused(app))).await;
        return;
    }

    let snapshot = build_snapshot(app, &report).await;
    let open = snapshot.attention.clone();
    update_tray_menu(app, snapshot).await;

    if !alerts_paused(app) {
        notify_for_new_urgent(app, &report.attention_created, &open);
    }
}

/// Read the current state and replace the tray menu — without driving a
/// fresh tick. Used by the "Pause Alerts" toggle so the label flips
/// without paying for a full scan.
pub async fn rebuild_menu_only<R: Runtime>(app: &AppHandle<R>) {
    let snapshot = match read_current_snapshot(app).await {
        Some(s) => s,
        None => TrayMenuSnapshot::unavailable(alerts_paused(app)),
    };
    update_tray_menu(app, snapshot).await;
}

async fn build_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    report: &MonitorTickReport,
) -> TrayMenuSnapshot {
    let _ = report; // Reserved for future use (e.g. surface tick errors in tray).
    read_current_snapshot(app)
        .await
        .unwrap_or_else(|| TrayMenuSnapshot::unavailable(alerts_paused(app)))
}

async fn read_current_snapshot<R: Runtime>(app: &AppHandle<R>) -> Option<TrayMenuSnapshot> {
    let tick_state = app.try_state::<MonitorTickState>()?;
    let storage = tick_state.storage()?;
    let engine = tick_state.attention()?.clone();
    let paused = alerts_paused(app);

    // Run the SQL reads off-thread so the async loop stays responsive
    // even when the storage mutex is hot.
    let snapshot = tauri::async_runtime::spawn_blocking(move || {
        compose_snapshot(&storage, engine.list_open().unwrap_or_default(), paused)
    })
    .await
    .ok()?;
    Some(snapshot)
}

fn compose_snapshot(
    storage: &Arc<Storage>,
    attention: Vec<OpenAttentionEntry>,
    alerts_paused: bool,
) -> TrayMenuSnapshot {
    let sessions = list_active(storage).unwrap_or_default();
    let active_sessions = sessions.len();
    let waiting_sessions = count_waiting(&sessions);
    TrayMenuSnapshot {
        ready: true,
        active_sessions,
        waiting_sessions,
        attention,
        alerts_paused,
    }
}

fn count_waiting(sessions: &[Session]) -> usize {
    use agentdeck_adapter::SessionStatus;
    sessions
        .iter()
        .filter(|s| s.status == SessionStatus::WaitingForInput)
        .count()
}

async fn update_tray_menu<R: Runtime>(app: &AppHandle<R>, snapshot: TrayMenuSnapshot) {
    let Some(keeper) = app.try_state::<TrayKeeper<R>>() else {
        // No tray on this host (fallback path). Nothing to update;
        // notifications are still sent by run_once.
        return;
    };
    if let Err(err) = set_menu_from_snapshot(app, keeper.icon(), &snapshot) {
        tracing::warn!(error = %err, "tray scheduler: failed to update tray menu");
    }
}

fn alerts_paused<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<AlertsPausedState>()
        .map(|s| s.is_paused())
        .unwrap_or(false)
}
