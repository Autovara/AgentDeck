//! The [`Adapter`] trait and per-scan result types.

use agentdeck_process::ProcessSnapshot;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::diagnostics::AdapterDiagnostic;
use crate::types::{CapabilityLevel, Confidence, SessionStatus};

/// One detected agent process, produced by an [`Adapter`] for a single
/// process in a [`ProcessSnapshot`].
///
/// `AdapterMatch` is the bridge between the per-tick adapter scan and the
/// session state machine (build plan step 10). The state machine decides
/// whether each match creates a new session or updates an existing one;
/// the adapter does not maintain any session identity of its own.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterMatch {
    /// Adapter that emitted this match (e.g. `"custom"`, `"aider"`).
    /// Reported into `sessions.adapter_name`.
    pub adapter_name: String,
    /// Agent name written into `sessions.agent_name`.
    pub agent_name: String,
    /// Capability level the adapter is operating at for this match.
    pub capability_level: CapabilityLevel,
    /// PID of the matched process at the time of the scan.
    pub pid: u32,
    /// Best-effort full command line, joined with spaces.
    pub command: String,
    /// Process working directory if the OS disclosed it.
    pub cwd: Option<String>,
    /// Repo path inferred from `cwd` (alpha: identical to `cwd` for Level 1;
    /// higher-tier adapters may walk upward looking for a `.git` directory).
    pub repo_path: Option<String>,
    /// Status classification at this scan. Level 1 adapters emit
    /// [`SessionStatus::Running`]; the session state machine maps to
    /// `Completed` when the PID later disappears.
    pub status: SessionStatus,
    /// Confidence in the `status` value.
    pub status_confidence: Confidence,
    /// Short label for *how* the status was decided. The diagnostics card
    /// surfaces this verbatim (e.g. `"process-list"`, `"history-mtime"`).
    pub status_source: String,
    /// Snapshot timestamp this match was derived from.
    pub observed_at: DateTime<Utc>,
}

/// Per-scan output of a single adapter.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterScanResult {
    /// Every match this adapter produced during the scan, in adapter-defined
    /// order.
    pub matches: Vec<AdapterMatch>,
    /// Fresh diagnostic for this adapter. Always returned so the
    /// `adapter_diagnostics` table can be `INSERT OR REPLACE`-d after every
    /// tick.
    pub diagnostic: AdapterDiagnostic,
}

/// The contract every concrete adapter implements.
///
/// Adapters are **synchronous and side-effect-free over their inputs**: a
/// `scan` call takes a borrowed snapshot, must not touch the database, and
/// must not block on the network. Cheap filesystem reads required for
/// status classification (e.g. tailing Aider's history file) are allowed.
/// The monitor core wraps every scan tick in `tokio::task::spawn_blocking`.
///
/// Adapters must be `Send + Sync` so the registry can be shared across
/// tokio tasks and per-adapter background workers can be added later.
pub trait Adapter: Send + Sync {
    /// Stable, lower-case identifier (e.g. `"custom"`, `"aider"`,
    /// `"codex"`, `"claude-code"`). Reported as `sessions.adapter_name`
    /// and `adapter_diagnostics.adapter_name`.
    fn name(&self) -> &str;

    /// Capability level the adapter is operating at *right now*. May fall
    /// below the maximum the adapter is built for (e.g. when an external
    /// log file is unreadable).
    fn capability_level(&self) -> CapabilityLevel;

    /// Whether this adapter participates in the current scan tick. The
    /// registry skips disabled adapters' `scan` calls but still emits a
    /// "disabled" diagnostic so the dashboard can render their state.
    fn enabled(&self) -> bool {
        true
    }

    /// Scan the snapshot and return any matches plus a fresh diagnostic.
    /// Adapters surface transient failures via
    /// [`AdapterDiagnostic::failure_reasons`] rather than `Result`, so the
    /// monitor can still publish partial results on every tick.
    fn scan(&self, snapshot: &ProcessSnapshot) -> AdapterScanResult;
}
