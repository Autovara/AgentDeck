//! Tray surface detection.
//!
//! AgentDeck's tray/menu-bar icon is the primary surface on macOS and Windows,
//! and on Linux desktop environments that ship a StatusNotifierItem host.
//! When that surface is unavailable — most importantly on stock GNOME without
//! the AppIndicator extension, and on headless or minimal Linux sessions —
//! AgentDeck must fall straight back to the dashboard window plus desktop
//! notifications, never to a silently invisible tray.
//!
//! This module performs three jobs:
//!
//! 1. Classify the host environment ([`probe`]). On Linux this includes a DBus
//!    check for `org.kde.StatusNotifierWatcher`, which is the canonical signal
//!    that a tray host is running on the session bus.
//! 2. Optionally attempt to construct a real `TrayIcon` ([`try_build_tray`]),
//!    which is the only fully reliable test of tray support and which yields
//!    the icon we then keep alive for the lifetime of the app.
//! 3. Expose the result to the dashboard via a serialisable
//!    [`TraySurfaceReport`].
//!
//! The result is cached in [`TraySurfaceState`] so the dashboard can render it
//! immediately without re-probing every render.

use std::sync::Mutex;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{
    menu::MenuEvent,
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};

use crate::alerts::AlertsPausedState;
use crate::tray_menu::{
    build_menu, parse_attention_id, TrayMenuSnapshot, MENU_ID_OPEN_DASHBOARD, MENU_ID_PAUSE_ALERTS,
    MENU_ID_QUIT, MENU_ID_REFRESH,
};

/// Underlying mechanism the tray icon would use on the current host.
///
/// Variants are reachable from different `cfg(target_os = ...)` branches and
/// from external serialised consumers, so we explicitly allow the dead-code
/// lint that fires for variants not constructible on the build host.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrayMechanism {
    MacosStatusItem,
    WindowsShellNotifyIcon,
    LinuxStatusNotifier,
    LinuxXembed,
    LinuxAppindicator,
    Unsupported,
    Unknown,
}

/// Operating system family used to classify a [`TraySurfaceReport`].
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Macos,
    Windows,
    Linux,
    Other,
}

/// Outcome of the startup tray probe. Mirrors the TypeScript
/// `TraySurfaceReport` interface in `src/lib/tauri.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraySurfaceReport {
    /// `true` when the dashboard should treat the tray icon as the primary
    /// surface.
    pub available: bool,
    /// Underlying tray mechanism for this platform/desktop.
    pub mechanism: TrayMechanism,
    /// Free-form environment label (e.g. `"linux/gnome/wayland"`).
    pub environment: String,
    /// Coarse-grained operating system family.
    pub platform: Platform,
    /// Optional desktop environment (Linux only).
    pub desktop_environment: Option<String>,
    /// Optional XDG session type (Linux only): `x11`, `wayland`, `tty`, ...
    pub session_type: Option<String>,
    /// Human-readable explanation; surfaced verbatim in the dashboard.
    pub reason: String,
    /// `true` when the app should open the dashboard window directly instead
    /// of hiding behind a tray that the user cannot see.
    pub fallback_required: bool,
    /// When the probe was performed (UTC).
    pub probed_at: DateTime<Utc>,
    /// Optional link to documentation that explains how to enable a tray on
    /// this environment.
    pub help_url: Option<String>,
}

impl TraySurfaceReport {
    /// Convert a previously-positive report into a fallback report after a
    /// late failure (e.g. `TrayIconBuilder::build` returned `Err` even though
    /// the environment probe said the host should support a tray).
    pub fn into_fallback(mut self, reason: String) -> Self {
        self.available = false;
        self.fallback_required = true;
        self.mechanism = TrayMechanism::Unsupported;
        self.reason = reason;
        self
    }
}

/// Holds the cached [`TraySurfaceReport`] for the lifetime of the Tauri app.
pub struct TraySurfaceState(Mutex<TraySurfaceReport>);

impl TraySurfaceState {
    pub fn new(report: TraySurfaceReport) -> Self {
        Self(Mutex::new(report))
    }

    pub fn snapshot(&self) -> TraySurfaceReport {
        self.0
            .lock()
            .expect("TraySurfaceState mutex poisoned")
            .clone()
    }
}

/// Wrapper that keeps the [`TrayIcon`] alive for the duration of the app.
/// Dropping the icon would remove it from the host tray on most platforms.
pub struct TrayKeeper<R: Runtime>(TrayIcon<R>);

impl<R: Runtime> TrayKeeper<R> {
    pub fn new(icon: TrayIcon<R>) -> Self {
        Self(icon)
    }

    /// Borrow the underlying [`TrayIcon`]. Used by the tray scheduler
    /// to swap the menu on every refresh.
    pub fn icon(&self) -> &TrayIcon<R> {
        &self.0
    }
}

/// Classify the host environment. Does not actually create a tray icon; see
/// [`try_build_tray`] for that.
pub fn probe<R: Runtime>(_app: &AppHandle<R>) -> TraySurfaceReport {
    let probed_at = Utc::now();

    #[cfg(target_os = "macos")]
    {
        TraySurfaceReport {
            available: true,
            mechanism: TrayMechanism::MacosStatusItem,
            environment: "macos".to_string(),
            platform: Platform::Macos,
            desktop_environment: None,
            session_type: None,
            reason: "macOS status bar is always available.".to_string(),
            fallback_required: false,
            probed_at,
            help_url: None,
        }
    }

    #[cfg(target_os = "windows")]
    {
        TraySurfaceReport {
            available: true,
            mechanism: TrayMechanism::WindowsShellNotifyIcon,
            environment: "windows".to_string(),
            platform: Platform::Windows,
            desktop_environment: None,
            session_type: None,
            reason: "Windows system tray is always available.".to_string(),
            fallback_required: false,
            probed_at,
            help_url: None,
        }
    }

    #[cfg(target_os = "linux")]
    {
        probe_linux(probed_at)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        TraySurfaceReport {
            available: false,
            mechanism: TrayMechanism::Unsupported,
            environment: std::env::consts::OS.to_string(),
            platform: Platform::Other,
            desktop_environment: None,
            session_type: None,
            reason: format!(
                "Unrecognised operating system ({}); assuming no tray surface.",
                std::env::consts::OS,
            ),
            fallback_required: true,
            probed_at,
            help_url: None,
        }
    }
}

#[cfg(target_os = "linux")]
fn probe_linux(probed_at: DateTime<Utc>) -> TraySurfaceReport {
    let desktop = std::env::var("XDG_CURRENT_DESKTOP")
        .ok()
        .filter(|s| !s.is_empty());
    let session_type = std::env::var("XDG_SESSION_TYPE")
        .ok()
        .filter(|s| !s.is_empty());
    let has_sni = tauri::async_runtime::block_on(probe_sni_watcher());

    classify_linux(desktop, session_type, has_sni, probed_at)
}

/// Pure function form of the Linux classification logic. Split out so the
/// branch table can be unit-tested without a real DBus session or live
/// environment variables. The exhaustively-tested branches feed back into
/// [`probe_linux`] above.
#[cfg(target_os = "linux")]
fn classify_linux(
    desktop: Option<String>,
    session_type: Option<String>,
    has_sni: bool,
    probed_at: DateTime<Utc>,
) -> TraySurfaceReport {
    let desktop_label = desktop
        .as_deref()
        .unwrap_or("unknown")
        .to_lowercase()
        .replace(':', "-");
    let session_label = session_type.as_deref().unwrap_or("unknown").to_lowercase();
    let environment = format!("linux/{desktop_label}/{session_label}");

    let desktop_lower = desktop.as_deref().unwrap_or("").to_lowercase();
    let is_gnome = desktop_lower.contains("gnome") || desktop_lower.contains("unity");
    let is_kde = desktop_lower.contains("kde");

    if has_sni {
        return TraySurfaceReport {
            available: true,
            mechanism: TrayMechanism::LinuxStatusNotifier,
            environment,
            platform: Platform::Linux,
            desktop_environment: desktop,
            session_type,
            reason: "Detected an active StatusNotifierWatcher on the session bus.".to_string(),
            fallback_required: false,
            probed_at,
            help_url: None,
        };
    }

    if is_gnome {
        return TraySurfaceReport {
            available: false,
            mechanism: TrayMechanism::Unsupported,
            environment,
            platform: Platform::Linux,
            desktop_environment: desktop,
            session_type,
            reason: "No StatusNotifierWatcher is registered on the session bus. \
                     Stock GNOME removed the legacy system tray in version 3; \
                     install the AppIndicator extension to expose one."
                .to_string(),
            fallback_required: true,
            probed_at,
            help_url: Some(
                "https://extensions.gnome.org/extension/615/appindicator-support/".to_string(),
            ),
        };
    }

    // For KDE we expect SNI to be present; if it is not, treat that as a
    // genuine error rather than the expected "stock GNOME" case.
    let reason = if is_kde {
        "No StatusNotifierWatcher detected on the session bus, even though \
         KDE Plasma usually provides one. The tray host may have crashed; \
         AgentDeck will run as a dashboard."
            .to_string()
    } else {
        "No StatusNotifierWatcher detected on the session bus. AgentDeck will \
         run as a dashboard window and rely on desktop notifications."
            .to_string()
    };

    TraySurfaceReport {
        available: false,
        mechanism: TrayMechanism::Unknown,
        environment,
        platform: Platform::Linux,
        desktop_environment: desktop,
        session_type,
        reason,
        fallback_required: true,
        probed_at,
        help_url: None,
    }
}

#[cfg(target_os = "linux")]
async fn probe_sni_watcher() -> bool {
    use std::convert::TryFrom;

    use zbus::{names::BusName, Connection};

    let conn = match Connection::session().await {
        Ok(conn) => conn,
        Err(err) => {
            tracing::debug!(
                error = %err,
                "Failed to connect to session DBus during tray probe",
            );
            return false;
        }
    };

    let proxy = match zbus::fdo::DBusProxy::new(&conn).await {
        Ok(proxy) => proxy,
        Err(err) => {
            tracing::debug!(
                error = %err,
                "Failed to open org.freedesktop.DBus proxy during tray probe",
            );
            return false;
        }
    };

    let name = match BusName::try_from("org.kde.StatusNotifierWatcher") {
        Ok(name) => name,
        Err(err) => {
            tracing::debug!(
                error = %err,
                "Failed to parse StatusNotifierWatcher bus name (programmer error)",
            );
            return false;
        }
    };

    match proxy.name_has_owner(name).await {
        Ok(owned) => owned,
        Err(err) => {
            tracing::debug!(
                error = %err,
                "DBus name_has_owner check failed during tray probe",
            );
            false
        }
    }
}

/// Build the tray icon for the current host. Caller is responsible for keeping
/// the returned [`TrayIcon`] alive (see [`TrayKeeper`]).
///
/// `initial_snapshot` populates the menu on first paint. The tray scheduler
/// rebuilds the menu from a fresh snapshot on every tick (see
/// [`set_menu_from_snapshot`]).
pub fn try_build_tray<R: Runtime>(
    app: &AppHandle<R>,
    initial_snapshot: &TrayMenuSnapshot,
) -> tauri::Result<TrayIcon<R>> {
    let menu = build_menu(app, initial_snapshot)?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default window icon".into()))?;

    TrayIconBuilder::with_id("agentdeck-main")
        .icon(icon)
        .tooltip("AgentDeck (alpha)")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                focus_dashboard(tray.app_handle());
            }
        })
        .build(app)
}

/// Replace the tray icon's menu with one built from `snapshot`.
pub fn set_menu_from_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    icon: &TrayIcon<R>,
    snapshot: &TrayMenuSnapshot,
) -> tauri::Result<()> {
    let menu = build_menu(app, snapshot)?;
    icon.set_menu(Some(menu))?;
    Ok(())
}

fn handle_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    let id = event.id.as_ref();
    match id {
        MENU_ID_OPEN_DASHBOARD => focus_dashboard(app),
        MENU_ID_REFRESH => spawn_refresh(app),
        MENU_ID_PAUSE_ALERTS => spawn_toggle_alerts(app),
        MENU_ID_QUIT => app.exit(0),
        other if other.starts_with(crate::tray_menu::ATTENTION_PREFIX) => {
            if parse_attention_id(other).is_some() {
                // Per build-plan §13 the alpha tray hands the user to
                // the dashboard for any per-item action. The dashboard
                // already opens on the Overview page, which surfaces
                // the same attention list.
                focus_dashboard(app);
            } else {
                tracing::debug!(menu_id = other, "Malformed attention menu id");
            }
        }
        other => {
            tracing::debug!(menu_id = other, "Unhandled tray menu event");
        }
    }
}

/// Spawn a one-shot tick + tray refresh in response to the "Refresh"
/// menu item. Same code path the scheduler uses, so concurrent ticks
/// serialise safely on the storage mutex.
fn spawn_refresh<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        crate::tray_scheduler::run_once(&handle).await;
    });
}

/// Flip [`AlertsPausedState`] and rebuild the tray menu so the label
/// updates immediately. Does *not* run a monitor tick — pausing alerts
/// is independent of the data refresh.
fn spawn_toggle_alerts<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let paused = match handle.try_state::<AlertsPausedState>() {
            Some(s) => s.toggle(),
            None => return,
        };
        tracing::info!(paused, "tray: alerts toggled");
        crate::tray_scheduler::rebuild_menu_only(&handle).await;
    });
}

fn focus_dashboard<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        if let Err(err) = window.show() {
            tracing::warn!(error = %err, "Failed to show main window from tray");
        }
        if let Err(err) = window.set_focus() {
            tracing::debug!(error = %err, "Failed to focus main window from tray");
        }
    } else {
        tracing::warn!("No main webview window present when tray click fired");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts() -> DateTime<Utc> {
        "2026-05-22T00:00:00Z"
            .parse::<DateTime<Utc>>()
            .expect("static timestamp parses")
    }

    #[test]
    fn into_fallback_marks_report_unavailable() {
        let report = TraySurfaceReport {
            available: true,
            mechanism: TrayMechanism::LinuxStatusNotifier,
            environment: "linux/kde/wayland".into(),
            platform: Platform::Linux,
            desktop_environment: Some("KDE".into()),
            session_type: Some("wayland".into()),
            reason: "ok".into(),
            fallback_required: false,
            probed_at: ts(),
            help_url: None,
        };
        let fallback = report.into_fallback("forced".into());
        assert!(!fallback.available);
        assert!(fallback.fallback_required);
        assert!(matches!(fallback.mechanism, TrayMechanism::Unsupported));
        assert_eq!(fallback.reason, "forced");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn kde_with_sni_classifies_as_available() {
        let r = classify_linux(Some("KDE".into()), Some("wayland".into()), true, ts());
        assert!(r.available);
        assert!(!r.fallback_required);
        assert!(matches!(r.mechanism, TrayMechanism::LinuxStatusNotifier));
        assert_eq!(r.environment, "linux/kde/wayland");
        assert!(r.help_url.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn gnome_without_sni_classifies_as_fallback_with_help_link() {
        let r = classify_linux(Some("GNOME".into()), Some("wayland".into()), false, ts());
        assert!(!r.available);
        assert!(r.fallback_required);
        assert!(matches!(r.mechanism, TrayMechanism::Unsupported));
        let help = r.help_url.expect("GNOME fallback advertises a help link");
        assert!(help.contains("appindicator-support"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn gnome_with_sni_classifies_as_available() {
        let r = classify_linux(Some("ubuntu:GNOME".into()), Some("x11".into()), true, ts());
        assert!(r.available);
        assert!(matches!(r.mechanism, TrayMechanism::LinuxStatusNotifier));
        assert_eq!(r.environment, "linux/ubuntu-gnome/x11");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn kde_without_sni_calls_out_unexpected_state() {
        let r = classify_linux(Some("KDE".into()), Some("x11".into()), false, ts());
        assert!(!r.available);
        assert!(matches!(r.mechanism, TrayMechanism::Unknown));
        assert!(r.reason.contains("KDE"));
        assert!(r.help_url.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn unknown_desktop_without_sni_falls_back_quietly() {
        let r = classify_linux(None, None, false, ts());
        assert!(!r.available);
        assert!(r.fallback_required);
        assert!(matches!(r.mechanism, TrayMechanism::Unknown));
        assert_eq!(r.environment, "linux/unknown/unknown");
        assert!(r.help_url.is_none());
        assert!(r.desktop_environment.is_none());
        assert!(r.session_type.is_none());
    }

    #[test]
    fn report_serialises_to_camel_case_keys() {
        let r = TraySurfaceReport {
            available: false,
            mechanism: TrayMechanism::Unknown,
            environment: "test".into(),
            platform: Platform::Linux,
            desktop_environment: Some("X".into()),
            session_type: Some("wayland".into()),
            reason: "r".into(),
            fallback_required: true,
            probed_at: ts(),
            help_url: None,
        };
        let json = serde_json::to_value(&r).expect("report must serialise");
        let obj = json.as_object().expect("report serialises as object");
        assert!(obj.contains_key("desktopEnvironment"));
        assert!(obj.contains_key("sessionType"));
        assert!(obj.contains_key("fallbackRequired"));
        assert!(obj.contains_key("probedAt"));
        assert!(obj.contains_key("helpUrl"));
        assert_eq!(obj["mechanism"], "unknown");
        assert_eq!(obj["platform"], "linux");
    }
}
