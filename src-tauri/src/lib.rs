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

mod tray;

use tauri::Manager;
use tray::{TraySurfaceReport, TraySurfaceState};

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
        .invoke_handler(tauri::generate_handler![get_tray_surface])
        .setup(|app| {
            let handle = app.handle().clone();
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
