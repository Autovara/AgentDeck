//! Per-scan adapter diagnostic mirroring `AdapterDiagnostic` from
//! `planning/alpha-build-plan.md` §6.4 and the `adapter_diagnostics` SQL
//! table.

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::types::{CapabilityLevel, Confidence};

/// Diagnostic snapshot of one adapter at the end of one scan.
///
/// The monitor core upserts this into the `adapter_diagnostics` table on
/// every tick (one row per `adapter_name`, per the
/// `INSERT OR REPLACE` discipline noted in
/// `0001_initial.sql`). The same structure is serialised straight to the
/// diagnostics dashboard card.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDiagnostic {
    /// Matches [`crate::Adapter::name`].
    pub adapter_name: String,
    /// Whether the adapter is currently participating in scans.
    pub enabled: bool,
    /// Capability level the adapter is operating at for *this* scan.
    pub capability_level: CapabilityLevel,
    /// Snapshot time used to derive matches; the monitor copies this into
    /// `adapter_diagnostics.last_scan_time`.
    pub last_scan_time: DateTime<Utc>,
    /// Number of matches the adapter emitted from this snapshot.
    pub detected_count: u32,
    /// Free-form labels for the data sources the adapter consulted
    /// (e.g. `"process-list"`, `"aider-history-file"`). Surfaced verbatim
    /// in the diagnostics card.
    pub data_sources_used: Vec<String>,
    /// Permissions the adapter wanted but could not get (e.g.
    /// `"read /proc/<pid>/cwd"`). Empty when nothing was denied.
    pub missing_permissions: Vec<String>,
    /// Transient or persistent failures the adapter wants to surface
    /// without aborting the scan.
    pub failure_reasons: Vec<String>,
    /// Confidence in this scan's classification.
    pub confidence: Confidence,
    /// Free-form caveats the user should see on the diagnostics card.
    pub known_limitations: Vec<String>,
}
