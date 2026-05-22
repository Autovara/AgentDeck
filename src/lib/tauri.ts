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

/**
 * Severity tier on an attention item. Mirrors `AttentionSeverity` in
 * `crates/agentdeck-attention/src/severity.rs` and the `attention_items.severity`
 * SQL CHECK constraint.
 */
export type AttentionSeverity = "info" | "warn" | "urgent";

/**
 * Reason an attention item exists. Mirrors `AttentionReason` in
 * `crates/agentdeck-attention/src/reason.rs`.
 */
export type AttentionReason =
  | "waiting_for_input"
  | "approval_required"
  | "rate_limit"
  | "authentication_required"
  | "context_limit"
  | "stalled_session"
  | "command_failed"
  | "process_crashed"
  | "budget_threshold";

/**
 * Action AgentDeck suggests for an attention item. Mirrors
 * `RecommendedAction` in `crates/agentdeck-attention/src/action.rs`.
 */
export type RecommendedAction =
  | "open_dashboard"
  | "mute"
  | "stop"
  | "notify_later";

/** Confidence label shared by sessions and attention items. */
export type AttentionConfidence = "high" | "medium" | "low" | "unknown";

/** Session status the engine joins with each attention item. */
export type SessionStatus =
  | "running"
  | "idle"
  | "waiting_for_input"
  | "rate_limited"
  | "stalled"
  | "errored"
  | "completed"
  | "unknown";

/**
 * One row from `attention_items`. Mirrors
 * `agentdeck_attention::AttentionItem`.
 */
export interface AttentionItem {
  id: string;
  sessionId: string;
  reason: AttentionReason;
  severity: AttentionSeverity;
  message: string;
  source: string;
  confidence: AttentionConfidence;
  recommendedActions: RecommendedAction[];
  createdAt: string;
  resolvedAt: string | null;
  mutedUntil: string | null;
}

/** Session label joined to an open attention item. */
export interface AttentionSessionRef {
  id: string;
  agentName: string;
  adapterName: string;
  status: SessionStatus;
  pid: number | null;
  /** Repo path inferred from `sessions.repo_path`; nullable. */
  repoPath: string | null;
  /** User-assigned project tag; always null in the alpha. */
  projectTag: string | null;
}

/** Open attention item + parent session, returned by `get_attention_report`. */
export interface OpenAttentionEntry {
  item: AttentionItem;
  session: AttentionSessionRef;
}

/** Dashboard payload for the "Attention" card. */
export interface AttentionReport {
  ready: boolean;
  items: OpenAttentionEntry[];
  capturedAt: string;
  error: string | null;
}

/**
 * Payload returned by `run_monitor_tick`. The shell drives this on demand
 * today; a future build step turns it into a background loop.
 */
export interface MonitorTickReport {
  ready: boolean;
  tickAt: string;
  snapshotTime: string;
  adapterMatches: number;
  sessionsCreated: string[];
  sessionsUpdated: string[];
  sessionsCompleted: string[];
  attentionCreated: string[];
  attentionUpdated: string[];
  attentionResolved: string[];
  attentionSkippedMuted: string[];
  error: string | null;
}

/** Drive one full monitor tick. */
export async function runMonitorTick(): Promise<MonitorTickReport> {
  return invoke<MonitorTickReport>("run_monitor_tick");
}

/** Read the current open attention items without driving a tick. */
export async function getAttentionReport(): Promise<AttentionReport> {
  return invoke<AttentionReport>("get_attention_report");
}

/**
 * Mute an open attention item for `hours` (rounded to integer hours).
 * Pass `0` to clear an existing mute.
 */
export async function muteAttentionItem(
  id: string,
  hours: number,
): Promise<AttentionItem> {
  return invoke<AttentionItem>("mute_attention_item", { id, hours });
}

/** Resolve an open attention item manually. */
export async function resolveAttentionItem(id: string): Promise<AttentionItem> {
  return invoke<AttentionItem>("resolve_attention_item", { id });
}

/**
 * Dashboard payload for the Overview page. Mirrors the `OverviewReport`
 * struct in `src-tauri/src/overview.rs`.
 */
export interface OverviewReport {
  ready: boolean;
  activeSessions: number;
  attentionTotal: number;
  attentionUrgent: number;
  attentionWarn: number;
  attentionInfo: number;
  stalledSessions: number;
  waitingSessions: number;
  /** `null` until cost tracking lands in §15 step 18. */
  estimatedCostToday: number | null;
  /** Up to 5 open attention items, urgent first. */
  recentAttention: OpenAttentionEntry[];
  capturedAt: string;
  error: string | null;
}

export async function getOverviewReport(): Promise<OverviewReport> {
  return invoke<OverviewReport>("get_overview_report");
}

/**
 * Capability level enum from `agentdeck_adapter::CapabilityLevel`,
 * serialised as snake_case strings.
 */
export type CapabilityLevel = "presence" | "status" | "usage" | "control";

/** Confidence label from `agentdeck_adapter::Confidence`. */
export type Confidence = "high" | "medium" | "low" | "unknown";

/**
 * Mirrors `agentdeck_adapter::AdapterDiagnostic`. One row per
 * `adapter_name` in the `adapter_diagnostics` table.
 */
export interface AdapterDiagnostic {
  adapterName: string;
  enabled: boolean;
  capabilityLevel: CapabilityLevel;
  lastScanTime: string;
  detectedCount: number;
  dataSourcesUsed: string[];
  missingPermissions: string[];
  failureReasons: string[];
  confidence: Confidence;
  knownLimitations: string[];
}

/** Payload for the Diagnostics page's "Adapter diagnostics" card. */
export interface AdapterDiagnosticsReport {
  ready: boolean;
  items: AdapterDiagnostic[];
  capturedAt: string;
  error: string | null;
}

export async function getAdapterDiagnostics(): Promise<AdapterDiagnosticsReport> {
  return invoke<AdapterDiagnosticsReport>("get_adapter_diagnostics");
}

// --- Telegram (build-plan §15 step 17) -------------------------------------

/** One paired Telegram user. */
export interface AllowlistEntry {
  userId: number;
  username: string | null;
  pairedAt: string;
}

/** Pending pairing code surfaced through `get_telegram_status`. */
export interface PendingPairingPayload {
  code: string;
  expiresAt: string;
}

/** Dashboard payload for the Telegram pairing card. */
export interface TelegramStatusReport {
  /** `true` when storage opened at startup. */
  ready: boolean;
  /** Whether the user has opted Telegram on. */
  enabled: boolean;
  /** Whether a bot token is saved (never returns the token itself). */
  hasToken: boolean;
  /** Whether the long-polling bot task is currently running. */
  running: boolean;
  /** Paired Telegram users, sorted by `pairedAt` ascending. */
  allowlist: AllowlistEntry[];
  /** Active pairing code, if any. Null after a code is consumed or
   * expired. */
  pendingPairing: PendingPairingPayload | null;
  capturedAt: string;
  error: string | null;
}

/** Result of `generate_telegram_pairing_code`. */
export interface PairingCodeResult {
  code: string;
  expiresAt: string;
  status: TelegramStatusReport;
}

export async function getTelegramStatus(): Promise<TelegramStatusReport> {
  return invoke<TelegramStatusReport>("get_telegram_status");
}

export async function setTelegramToken(
  token: string,
): Promise<TelegramStatusReport> {
  return invoke<TelegramStatusReport>("set_telegram_token", { token });
}

export async function clearTelegramToken(): Promise<TelegramStatusReport> {
  return invoke<TelegramStatusReport>("clear_telegram_token");
}

export async function enableTelegram(): Promise<TelegramStatusReport> {
  return invoke<TelegramStatusReport>("enable_telegram");
}

export async function disableTelegram(): Promise<TelegramStatusReport> {
  return invoke<TelegramStatusReport>("disable_telegram");
}

export async function generateTelegramPairingCode(): Promise<PairingCodeResult> {
  return invoke<PairingCodeResult>("generate_telegram_pairing_code");
}

export async function cancelTelegramPairing(): Promise<TelegramStatusReport> {
  return invoke<TelegramStatusReport>("cancel_telegram_pairing");
}

export async function revokeTelegramUser(
  userId: number,
): Promise<TelegramStatusReport> {
  return invoke<TelegramStatusReport>("revoke_telegram_user", { userId });
}

// --- Sessions / tags / export (build-plan §15 step 18) ---------------------

/**
 * Mirrors `agentdeck_session::Session`. Includes everything the
 * Sessions page needs to render the table.
 */
export interface SessionRow {
  id: string;
  agentName: string;
  adapterName: string;
  adapterLevel: CapabilityLevel;
  pid: number | null;
  command: string;
  cwd: string | null;
  repoPath: string | null;
  projectTag: string | null;
  status: SessionStatus;
  statusConfidence: AttentionConfidence;
  attentionReason: string | null;
  startTime: string;
  lastSeenTime: string;
  lastActivityTime: string | null;
  estimatedCost: number | null;
  costKind: CostKind;
  createdAt: string;
  updatedAt: string;
}

/** Mirrors `sessions.cost_kind` enum. */
export type CostKind = "exact" | "estimated" | "unknown";

/** Dashboard payload for the Sessions page. */
export interface SessionsReport {
  ready: boolean;
  sessions: SessionRow[];
  capturedAt: string;
  error: string | null;
}

export async function listSessions(): Promise<SessionsReport> {
  return invoke<SessionsReport>("list_sessions");
}

/** One row in the `project_tags` table. */
export interface ProjectTag {
  id: string;
  name: string;
  color: string | null;
  notes: string | null;
  createdAt: string;
  updatedAt: string;
}

/** Input for `create_project_tag`. */
export interface NewProjectTag {
  name: string;
  color?: string | null;
  notes?: string | null;
}

/** Dashboard payload for the Project Tags card. */
export interface ProjectTagsReport {
  ready: boolean;
  tags: ProjectTag[];
  capturedAt: string;
  error: string | null;
}

export async function listProjectTags(): Promise<ProjectTagsReport> {
  return invoke<ProjectTagsReport>("list_project_tags");
}

export async function createProjectTag(
  input: NewProjectTag,
): Promise<ProjectTagsReport> {
  return invoke<ProjectTagsReport>("create_project_tag", { input });
}

export async function deleteProjectTag(id: string): Promise<ProjectTagsReport> {
  return invoke<ProjectTagsReport>("delete_project_tag", { id });
}

export async function assignSessionTag(
  sessionId: string,
  tagName: string,
): Promise<void> {
  return invoke<void>("assign_session_tag", { sessionId, tagName });
}

export async function clearSessionTagAssignment(
  sessionId: string,
): Promise<void> {
  return invoke<void>("clear_session_tag", { sessionId });
}

/** Result of an export command. */
export interface ExportResult {
  path: string;
  bytesWritten: number;
  format: "csv" | "json";
}

export async function exportSessionsCsv(path: string): Promise<ExportResult> {
  return invoke<ExportResult>("export_sessions_csv", { path });
}

export async function exportSessionsJson(path: string): Promise<ExportResult> {
  return invoke<ExportResult>("export_sessions_json", { path });
}
