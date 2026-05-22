import { invoke } from "@tauri-apps/api/core";

/**
 * Result of the startup tray availability probe.
 *
 * The Rust side classifies the host environment, optionally probes DBus on
 * Linux for a StatusNotifierWatcher, and may attempt a real
 * `TrayIconBuilder::new(...).build(...)` call. The structure mirrors the
 * `TraySurfaceReport` defined in `src-tauri/src/tray.rs`.
 */
export interface TraySurfaceReport {
  /** Whether the tray icon was successfully created or, if not probed, whether
   * the environment is likely to support a tray icon. */
  available: boolean;
  /** Underlying mechanism the tray would use on this OS / desktop. */
  mechanism: TrayMechanism;
  /** Free-form classification of the host environment, e.g.
   * "linux/gnome/wayland" or "linux/kde/x11". */
  environment: string;
  /** Operating system family the report was generated on. */
  platform: "macos" | "windows" | "linux" | "other";
  /** Optional desktop environment name (Linux only). */
  desktopEnvironment: string | null;
  /** Optional session type (Linux only): "x11" | "wayland" | "tty" | ... */
  sessionType: string | null;
  /** Human-readable reason explaining why the tray is or is not available. */
  reason: string;
  /** Whether AgentDeck must fall back to the dashboard window instead of
   * relying on the tray as its primary surface. */
  fallbackRequired: boolean;
  /** ISO-8601 UTC timestamp at which the probe was performed. */
  probedAt: string;
  /** Optional documentation link to help the user enable a tray on this
   * environment (e.g. GNOME AppIndicator extension). */
  helpUrl: string | null;
}

export type TrayMechanism =
  | "macos_status_item"
  | "windows_shell_notify_icon"
  | "linux_status_notifier"
  | "linux_xembed"
  | "linux_appindicator"
  | "unsupported"
  | "unknown";

/**
 * Ask the Rust side for the cached tray availability report. The first call
 * after launch may trigger the actual probe; subsequent calls return the
 * cached value until the session ends.
 */
export async function getTraySurface(): Promise<TraySurfaceReport> {
  return invoke<TraySurfaceReport>("get_tray_surface");
}
