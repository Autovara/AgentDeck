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

/**
 * Storage layer diagnostic snapshot. Mirrors the `StorageReport` struct in
 * `src-tauri/src/storage.rs`.
 */
export interface StorageReport {
  /** `true` when the database opened and migrations applied cleanly. */
  ready: boolean;
  /** Absolute path to the database file. */
  path: string;
  /** `PRAGMA user_version` after migrations; null on failure. */
  schemaVersion: number | null;
  /** File size in bytes (null for in-memory or before first write). */
  sizeBytes: number | null;
  /** SQLite journal mode; the storage layer aims for `wal`. */
  journalMode: string | null;
  /** Migrations the running binary knows about. */
  appliedMigrations: MigrationSummary[];
  /** Row counts per known table. */
  tables: TableSnapshot[];
  /** ISO-8601 UTC timestamp of this snapshot. */
  capturedAt: string;
  /** Verbatim error message when initialisation or refresh failed. */
  error: string | null;
}

export interface MigrationSummary {
  version: number;
  label: string;
}

export interface TableSnapshot {
  name: string;
  rowCount: number;
}

export async function getStorageReport(): Promise<StorageReport> {
  return invoke<StorageReport>("get_storage_report");
}

/**
 * Process scanner diagnostic snapshot. Mirrors the `ProcessScannerReport`
 * struct in `src-tauri/src/process_scanner.rs`.
 */
export interface ProcessScannerReport {
  /** `true` once at least one scan has completed without error. */
  ready: boolean;
  /** Source identifier for the underlying `ProcessSource`: "sysinfo",
   * "mock", or any future label. */
  source: string;
  /** Summary of the most recent scan; null until the first scan completes. */
  lastScan: ScanSummary | null;
  /** Candidate processes that matched at least one alpha agent pattern. */
  candidates: AgentCandidate[];
  /** Patterns the candidate extractor was run with. */
  patterns: string[];
  /** Verbatim error message when the most recent scan failed. */
  error: string | null;
  /** ISO-8601 UTC timestamp at which this report was assembled. */
  capturedAt: string;
}

export interface ScanSummary {
  /** UTC timestamp when the scan completed. */
  startedAt: string;
  /** Time spent enumerating processes, measured by the Rust side. */
  scanDurationMs: number;
  /** Number of processes in the snapshot. */
  totalProcesses: number;
}

export interface AgentCandidate {
  process: ProcessInfo;
  matchedPatterns: string[];
}

export interface ProcessInfo {
  pid: number;
  parent_pid: number | null;
  name: string;
  cmdline: string[];
  cwd: string | null;
  /** ISO-8601 UTC. */
  started_at: string;
}

/**
 * Trigger a fresh process scan and return the resulting report. The Rust
 * side runs sysinfo synchronously on this call, so callers should debounce.
 */
export async function getProcessScannerReport(): Promise<ProcessScannerReport> {
  return invoke<ProcessScannerReport>("get_process_scanner_report");
}

/**
 * One user-defined custom adapter plus the PIDs that matched in the latest
 * snapshot. Mirrors the `CustomAdapterSummary` struct in
 * `src-tauri/src/custom_adapter.rs`.
 */
export interface CustomAdapterSummary {
  id: string;
  label: string;
  agentName: string;
  enabled: boolean;
  color: string | null;
  matchKind: CustomAdapterMatchKind;
  pattern: string;
  costPerHourCents: number | null;
  notes: string | null;
  createdAt: string;
  updatedAt: string;
  matchedPids: number[];
  regexCompileError: string | null;
}

export type CustomAdapterMatchKind = "name" | "cmdline" | "cwd";

/**
 * Diagnostic snapshot of the custom-adapter registry. Mirrors
 * `CustomAdapterReport`.
 */
export interface CustomAdapterReport {
  ready: boolean;
  adapters: CustomAdapterSummary[];
  lastMatchRan: boolean;
  capturedAt: string;
  error: string | null;
}

/** Input shape for `add_custom_adapter`, matched by Rust serde. */
export interface NewCustomAdapter {
  label: string;
  agentName: string;
  matchKind: CustomAdapterMatchKind;
  pattern: string;
  enabled?: boolean;
  color?: string | null;
  costPerHourCents?: number | null;
  notes?: string | null;
}

export async function getCustomAdapterReport(): Promise<CustomAdapterReport> {
  return invoke<CustomAdapterReport>("get_custom_adapter_report");
}

export async function addCustomAdapter(
  input: NewCustomAdapter,
): Promise<CustomAdapterReport> {
  return invoke<CustomAdapterReport>("add_custom_adapter", { input });
}

export async function deleteCustomAdapter(
  id: string,
): Promise<CustomAdapterReport> {
  return invoke<CustomAdapterReport>("delete_custom_adapter", { id });
}

export async function setCustomAdapterEnabled(
  id: string,
  enabled: boolean,
): Promise<CustomAdapterReport> {
  return invoke<CustomAdapterReport>("set_custom_adapter_enabled", {
    id,
    enabled,
  });
}
