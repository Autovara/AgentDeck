use std::sync::{Arc, Mutex};
use std::time::Instant;

use agentdeck_core::{Clock, ProcessSource};

use crate::snapshot::{diff, ProcessSnapshot, ProcessSnapshotDiff, SnapshotSource};

/// Result of a [`ProcessScanner::scan`] call: the new snapshot, and the diff
/// against the previous one (if any).
#[derive(Debug, Clone)]
pub struct ProcessScanResult {
    pub snapshot: ProcessSnapshot,
    /// `None` on the first scan of a scanner's lifetime. After that, every
    /// scan produces a diff (possibly empty).
    pub diff: Option<ProcessSnapshotDiff>,
}

/// Wraps a [`ProcessSource`] with timestamping, monotonic duration
/// measurement, and a one-snapshot cache so [`Self::scan`] can return a diff.
///
/// `ProcessScanner` is intentionally synchronous; async callers should wrap
/// individual scans in `tokio::task::spawn_blocking`. The scanner has no
/// internal scheduling loop — that lives in the future monitor core.
pub struct ProcessScanner {
    source: Arc<dyn ProcessSource>,
    clock: Arc<dyn Clock>,
    source_kind: SnapshotSource,
    last: Mutex<Option<ProcessSnapshot>>,
}

impl ProcessScanner {
    pub fn new(
        source: Arc<dyn ProcessSource>,
        clock: Arc<dyn Clock>,
        source_kind: SnapshotSource,
    ) -> Self {
        Self {
            source,
            clock,
            source_kind,
            last: Mutex::new(None),
        }
    }

    /// Identifier of the underlying source (sysinfo / mock / other).
    pub fn source_kind(&self) -> &SnapshotSource {
        &self.source_kind
    }

    /// Most recent successful snapshot, if any.
    pub fn last_snapshot(&self) -> Option<ProcessSnapshot> {
        self.last.lock().ok().and_then(|guard| guard.clone())
    }

    /// Take a fresh snapshot without computing a diff. Useful when the caller
    /// only needs the current process list.
    pub fn scan_once(&self) -> ProcessSnapshot {
        let started = Instant::now();
        let mut processes = self.source.list();
        processes.sort_by_key(|p| p.pid);
        let captured_at = self.clock.now();
        let scan_duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;

        let snapshot = ProcessSnapshot {
            captured_at,
            source: self.source_kind.clone(),
            scan_duration_ms,
            processes,
        };

        if let Ok(mut guard) = self.last.lock() {
            *guard = Some(snapshot.clone());
        }
        snapshot
    }

    /// Take a fresh snapshot and return the diff against the previous one.
    /// The first call returns `diff = None`.
    pub fn scan(&self) -> ProcessScanResult {
        let previous = self.last_snapshot();
        let snapshot = self.scan_once();
        let diff = previous.as_ref().map(|prev| diff(prev, &snapshot));
        ProcessScanResult { snapshot, diff }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentdeck_core::{ProcessInfo, ProcessSource};
    use chrono::{DateTime, TimeZone, Utc};
    use std::sync::Mutex as StdMutex;

    /// Mock clock and source kept here to keep this crate independent of
    /// `agentdeck-harness`. Tests that need richer mocks pull in the harness.
    struct StaticClock(DateTime<Utc>);
    impl Clock for StaticClock {
        fn now(&self) -> DateTime<Utc> {
            self.0
        }
    }

    #[derive(Default)]
    struct StaticSource(StdMutex<Vec<ProcessInfo>>);
    impl ProcessSource for StaticSource {
        fn list(&self) -> Vec<ProcessInfo> {
            self.0.lock().expect("mutex").clone()
        }
    }
    impl StaticSource {
        fn set(&self, p: Vec<ProcessInfo>) {
            *self.0.lock().expect("mutex") = p;
        }
    }

    fn sample(pid: u32, cmd: &[&str]) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid: None,
            name: cmd[0].to_string(),
            cmdline: cmd.iter().map(|s| s.to_string()).collect(),
            cwd: None,
            started_at: Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
        }
    }

    #[test]
    fn first_scan_has_no_diff() {
        let source = Arc::new(StaticSource::default());
        source.set(vec![sample(10, &["aider"])]);
        let scanner = ProcessScanner::new(
            source,
            Arc::new(StaticClock(
                Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
            )),
            SnapshotSource::Mock,
        );
        let result = scanner.scan();
        assert!(result.diff.is_none(), "first scan must not yield a diff");
        assert_eq!(result.snapshot.processes.len(), 1);
        assert_eq!(result.snapshot.source.label(), "mock");
    }

    #[test]
    fn second_scan_detects_change() {
        let source = Arc::new(StaticSource::default());
        source.set(vec![sample(10, &["aider"])]);
        let scanner = ProcessScanner::new(
            source.clone(),
            Arc::new(StaticClock(
                Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
            )),
            SnapshotSource::Mock,
        );
        let _ = scanner.scan();
        source.set(vec![sample(10, &["aider"]), sample(20, &["codex"])]);
        let result = scanner.scan();
        let diff = result.diff.expect("second scan has a diff");
        assert_eq!(
            diff.started.iter().map(|p| p.pid).collect::<Vec<_>>(),
            vec![20]
        );
        assert!(diff.exited.is_empty());
    }

    #[test]
    fn snapshot_is_sorted_by_pid() {
        let source = Arc::new(StaticSource::default());
        source.set(vec![
            sample(30, &["c"]),
            sample(10, &["a"]),
            sample(20, &["b"]),
        ]);
        let scanner = ProcessScanner::new(
            source,
            Arc::new(StaticClock(
                Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
            )),
            SnapshotSource::Mock,
        );
        let snap = scanner.scan_once();
        let pids: Vec<u32> = snap.processes.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![10, 20, 30]);
    }

    #[test]
    fn last_snapshot_returns_after_scan() {
        let source = Arc::new(StaticSource::default());
        source.set(vec![sample(10, &["aider"])]);
        let scanner = ProcessScanner::new(
            source,
            Arc::new(StaticClock(
                Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
            )),
            SnapshotSource::Mock,
        );
        assert!(scanner.last_snapshot().is_none());
        let _ = scanner.scan_once();
        assert!(scanner.last_snapshot().is_some());
    }
}
