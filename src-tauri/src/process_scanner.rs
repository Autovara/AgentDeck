//! Bridge between the Tauri shell and `agentdeck-process`.
//!
//! The shell owns one [`ProcessScanner`] for the lifetime of the process. It
//! is currently invoked on demand via the `get_process_scanner_report`
//! command; the monitor core will later own this and tick it on a schedule.

use std::sync::Arc;
use std::sync::Mutex;

use agentdeck_core::{Clock, ProcessSource, SystemClock};
use agentdeck_process::{
    extract_candidates, AgentCandidate, ProcessScanner, ProcessSnapshot, SnapshotSource,
    SysinfoProcessSource, ALPHA_AGENT_PATTERNS,
};
use chrono::{DateTime, Utc};
use serde::Serialize;

/// Outcome of a single scanner invocation. Mirrors the TypeScript
/// `ProcessScannerReport` in `src/lib/tauri.ts`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessScannerReport {
    /// `true` once at least one scan has completed without error.
    pub ready: bool,
    /// Underlying process source identifier ("sysinfo", "mock", etc.).
    pub source: String,
    /// Last scan summary; `None` until the first scan completes.
    pub last_scan: Option<ScanSummary>,
    /// Agent candidates from the most recent snapshot, if any.
    pub candidates: Vec<AgentCandidate>,
    /// Patterns the candidate extractor was run with.
    pub patterns: Vec<&'static str>,
    /// Verbatim error from the most recent scan attempt, if any.
    pub error: Option<String>,
    /// UTC instant this report was assembled.
    pub captured_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    pub started_at: DateTime<Utc>,
    pub scan_duration_ms: u64,
    pub total_processes: usize,
}

/// Owns the [`ProcessScanner`] and its most recent report.
pub struct ProcessScannerState {
    scanner: Arc<ProcessScanner>,
    last_report: Mutex<ProcessScannerReport>,
}

impl ProcessScannerState {
    pub fn new() -> Self {
        let source: Arc<dyn ProcessSource> = Arc::new(SysinfoProcessSource::new());
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let scanner = Arc::new(ProcessScanner::new(source, clock, SnapshotSource::Sysinfo));
        Self {
            last_report: Mutex::new(initial_report(scanner.source_kind())),
            scanner,
        }
    }

    /// Run a scan, refresh the cached report, and return it.
    pub fn refresh(&self) -> ProcessScannerReport {
        let scanner = self.scanner.clone();
        // `scan` may be slow on Windows; this is invoked from the Tauri IPC
        // thread but only when the dashboard asks. The monitor core will
        // later move this onto its dedicated tick loop with `spawn_blocking`.
        let report = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scanner.scan()))
        {
            Ok(result) => build_report(scanner.source_kind(), &result.snapshot, None),
            Err(panic) => {
                let message = panic_message(panic);
                tracing::error!(error = %message, "process scanner panicked");
                error_report(scanner.source_kind(), &message)
            }
        };
        let mut guard = match self.last_report.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = report.clone();
        report
    }
}

impl Default for ProcessScannerState {
    fn default() -> Self {
        Self::new()
    }
}

fn initial_report(kind: &SnapshotSource) -> ProcessScannerReport {
    ProcessScannerReport {
        ready: false,
        source: kind.label().to_string(),
        last_scan: None,
        candidates: Vec::new(),
        patterns: ALPHA_AGENT_PATTERNS.to_vec(),
        error: None,
        captured_at: Utc::now(),
    }
}

fn build_report(
    kind: &SnapshotSource,
    snapshot: &ProcessSnapshot,
    error: Option<String>,
) -> ProcessScannerReport {
    let candidates = extract_candidates(snapshot, ALPHA_AGENT_PATTERNS);
    ProcessScannerReport {
        ready: error.is_none(),
        source: kind.label().to_string(),
        last_scan: Some(ScanSummary {
            started_at: snapshot.captured_at,
            scan_duration_ms: snapshot.scan_duration_ms,
            total_processes: snapshot.total_count(),
        }),
        candidates,
        patterns: ALPHA_AGENT_PATTERNS.to_vec(),
        error,
        captured_at: Utc::now(),
    }
}

fn error_report(kind: &SnapshotSource, message: &str) -> ProcessScannerReport {
    ProcessScannerReport {
        ready: false,
        source: kind.label().to_string(),
        last_scan: None,
        candidates: Vec::new(),
        patterns: ALPHA_AGENT_PATTERNS.to_vec(),
        error: Some(message.to_string()),
        captured_at: Utc::now(),
    }
}

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&'static str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "process scanner panicked".to_string()
    }
}
