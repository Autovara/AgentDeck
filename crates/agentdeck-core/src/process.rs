use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A snapshot of one process as seen by the monitor.
///
/// Field meanings are uniform across [`ProcessSource`] implementations:
/// `sysinfo` on a real machine, the mock source in the harness, or any
/// future backend must populate these fields with the same semantics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessInfo {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    /// Full command-line as parsed by the backend. May be empty when the
    /// OS does not expose it.
    pub cmdline: Vec<String>,
    /// Current working directory of the process. May be `None` when the
    /// OS refuses to disclose it (common on Windows for sandboxed
    /// processes) or when the process exited between listing and detail
    /// read.
    pub cwd: Option<String>,
    pub started_at: DateTime<Utc>,
}

/// A source of process snapshots.
///
/// Implementations must be cheap to call: the monitor invokes
/// [`ProcessSource::list`] on every scan tick (default once per second).
/// Implementations use interior mutability where needed; the trait takes
/// `&self` so multiple callers can share a single source through
/// `Arc<dyn ProcessSource>` without forcing exclusive locks.
pub trait ProcessSource: Send + Sync {
    /// Returns a snapshot of currently visible processes, ordered by PID
    /// for determinism in tests.
    fn list(&self) -> Vec<ProcessInfo>;
}
