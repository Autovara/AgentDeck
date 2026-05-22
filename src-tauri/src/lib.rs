//! AgentDeck Tauri application shell.
//!
//! This crate is intentionally thin in the alpha. Its responsibilities are:
//!
//! - Initialise structured logging.
//! - Probe the host environment for a tray surface (see [`tray`]).
//! - Build the main webview window and, when supported, a tray icon.
//! - Expose a small set of Tauri commands that the React dashboard consumes.
//!
//! All product logic (process scanning, adapters, attention engine, Telegram,
//! storage) will live in dedicated crates under `crates/` and the monitor core
//! will own all state. This shell simply wires the UI surface to the core
//! through Tauri commands and events.

mod attention;
mod custom_adapter;
mod monitor_tick;
mod overview;
mod process_scanner;
mod storage;
mod tray;

use agentdeck_adapter_custom::NewCustomAdapter;
use agentdeck_attention::AttentionItem;
use tauri::Manager;
use tray::{TraySurfaceReport, TraySurfaceState};
use uuid::Uuid;

use crate::attention::AttentionReport;
use crate::custom_adapter::{CustomAdapterReport, CustomAdapterState};
use crate::monitor_tick::{MonitorTickReport, MonitorTickState};
use crate::overview::OverviewReport;
use crate::process_scanner::{ProcessScannerReport, ProcessScannerState};
use crate::storage::{StorageReport, StorageReportState};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    if let Err(err) = build_app().run(tauri::generate_context!()) {
        tracing::error!(error = %err, "AgentDeck Tauri runtime exited with an error");
        std::process::exit(1);
    }
}

fn build_app() -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .invoke_handler(tauri::generate_handler![
            get_tray_surface,
            get_storage_report,
            get_process_scanner_report,
            get_custom_adapter_report,
            add_custom_adapter,
            delete_custom_adapter,
            set_custom_adapter_enabled,
            run_monitor_tick,
            get_attention_report,
            mute_attention_item,
            resolve_attention_item,
            get_overview_report,
        ])
        .setup(|app| {
            let handle = app.handle().clone();

            let storage_report = storage::initialise(&handle);
            let storage_handle = storage_report.storage.clone();
            app.manage(StorageReportState::new(storage_report));

            app.manage(ProcessScannerState::new());
            app.manage(CustomAdapterState::new(storage_handle.clone()));
            app.manage(MonitorTickState::new(storage_handle));

            let report = tray::probe(&handle);

            tracing::info!(
                available = report.available,
                mechanism = ?report.mechanism,
                env = report.environment.as_str(),
                "Tray surface probe complete",
            );

            let final_report = match (report.available, tray::try_build_tray(&handle)) {
                (true, Ok(icon)) => {
                    app.manage(tray::TrayKeeper::new(icon));
                    report
                }
                (true, Err(err)) => {
                    tracing::warn!(
                        error = %err,
                        "Tray probe reported support but TrayIconBuilder::build failed; falling back to dashboard window",
                    );
                    report.into_fallback(format!(
                        "Tray icon construction failed: {err}. Running as a \
                         dashboard-only surface."
                    ))
                }
                (false, _) => report,
            };

            if final_report.fallback_required {
                ensure_dashboard_visible(&handle);
            }

            app.manage(TraySurfaceState::new(final_report));
            Ok(())
        })
}

fn ensure_dashboard_visible<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(err) = window.show() {
            tracing::warn!(error = %err, "Failed to show main window during fallback path");
        }
        if let Err(err) = window.set_focus() {
            tracing::debug!(error = %err, "Failed to focus main window during fallback path");
        }
    } else {
        tracing::warn!("No 'main' webview window found while preparing dashboard fallback");
    }
}

#[tauri::command]
fn get_tray_surface(state: tauri::State<'_, TraySurfaceState>) -> TraySurfaceReport {
    state.snapshot()
}

#[tauri::command]
fn get_storage_report(state: tauri::State<'_, StorageReportState>) -> StorageReport {
    state.refresh()
}

#[tauri::command]
fn get_process_scanner_report(
    state: tauri::State<'_, ProcessScannerState>,
) -> ProcessScannerReport {
    state.refresh()
}

#[tauri::command]
fn get_custom_adapter_report(
    state: tauri::State<'_, CustomAdapterState>,
    scanner: tauri::State<'_, ProcessScannerState>,
) -> CustomAdapterReport {
    state.refresh(&scanner)
}

#[tauri::command]
fn add_custom_adapter(
    input: NewCustomAdapter,
    state: tauri::State<'_, CustomAdapterState>,
    scanner: tauri::State<'_, ProcessScannerState>,
) -> Result<CustomAdapterReport, String> {
    state.add(input, &scanner)
}

#[tauri::command]
fn delete_custom_adapter(
    id: String,
    state: tauri::State<'_, CustomAdapterState>,
    scanner: tauri::State<'_, ProcessScannerState>,
) -> Result<CustomAdapterReport, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid uuid {id:?}: {e}"))?;
    state.delete(uuid, &scanner)
}

#[tauri::command]
fn set_custom_adapter_enabled(
    id: String,
    enabled: bool,
    state: tauri::State<'_, CustomAdapterState>,
    scanner: tauri::State<'_, ProcessScannerState>,
) -> Result<CustomAdapterReport, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid uuid {id:?}: {e}"))?;
    state.set_enabled(uuid, enabled, &scanner)
}

#[tauri::command]
fn run_monitor_tick(
    state: tauri::State<'_, MonitorTickState>,
    scanner: tauri::State<'_, ProcessScannerState>,
) -> MonitorTickReport {
    state.run_tick(&scanner)
}

#[tauri::command]
fn get_attention_report(state: tauri::State<'_, MonitorTickState>) -> AttentionReport {
    attention::snapshot(&state)
}

#[tauri::command]
fn mute_attention_item(
    id: String,
    hours: i64,
    state: tauri::State<'_, MonitorTickState>,
) -> Result<AttentionItem, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid uuid {id:?}: {e}"))?;
    attention::mute(&state, uuid, hours)
}

#[tauri::command]
fn resolve_attention_item(
    id: String,
    state: tauri::State<'_, MonitorTickState>,
) -> Result<AttentionItem, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("invalid uuid {id:?}: {e}"))?;
    attention::resolve(&state, uuid)
}

#[tauri::command]
fn get_overview_report(state: tauri::State<'_, MonitorTickState>) -> OverviewReport {
    overview::snapshot(&state)
}

fn init_tracing() {
    // The dashboard and tests both rely on tracing. We default to `info` and
    // honour `AGENTDECK_LOG` for finer control. We deliberately do not
    // surface logs to the user through the UI yet; the alpha plan dedicates a
    // later step to a diagnostics panel.
    use tracing_subscriber::{fmt, EnvFilter};

    let filter = EnvFilter::try_from_env("AGENTDECK_LOG")
        .or_else(|_| EnvFilter::try_new("info"))
        .expect("default log filter must parse");

    let _ = fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_ansi(true)
        .try_init();
}
