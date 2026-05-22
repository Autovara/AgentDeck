//! Integration tests for `ProcessScanner`.
//!
//! These verify the scanner behaves correctly against both the in-memory
//! mock (deterministic) and the real `SysinfoProcessSource` (smoke: this
//! process itself must appear).

use std::sync::Arc;

use agentdeck_core::{Clock, ProcessInfo, ProcessSource, SystemClock};
use agentdeck_process::{
    extract_candidates, ProcessScanner, SnapshotSource, SysinfoProcessSource, ALPHA_AGENT_PATTERNS,
};
use chrono::{TimeZone, Utc};

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

struct VecSource(std::sync::Mutex<Vec<ProcessInfo>>);
impl VecSource {
    fn new(p: Vec<ProcessInfo>) -> Self {
        Self(std::sync::Mutex::new(p))
    }
    fn set(&self, p: Vec<ProcessInfo>) {
        *self.0.lock().expect("mutex") = p;
    }
}
impl ProcessSource for VecSource {
    fn list(&self) -> Vec<ProcessInfo> {
        self.0.lock().expect("mutex").clone()
    }
}

#[test]
fn diff_after_pid_churn() {
    let source = Arc::new(VecSource::new(vec![
        sample(10, &["aider"]),
        sample(20, &["bash"]),
    ]));
    let scanner = ProcessScanner::new(
        source.clone() as Arc<dyn ProcessSource>,
        Arc::new(SystemClock) as Arc<dyn Clock>,
        SnapshotSource::Mock,
    );

    let first = scanner.scan();
    assert!(first.diff.is_none());
    assert_eq!(first.snapshot.total_count(), 2);

    source.set(vec![
        sample(20, &["bash", "-l"]), // cmdline changed
        sample(30, &["codex"]),      // started
    ]);

    let second = scanner.scan();
    let diff = second.diff.expect("second scan must yield a diff");
    assert_eq!(diff.started.len(), 1);
    assert_eq!(diff.started[0].pid, 30);
    assert_eq!(diff.exited.len(), 1);
    assert_eq!(diff.exited[0].pid, 10);
    assert_eq!(diff.cmdline_changed.len(), 1);
    assert_eq!(diff.cmdline_changed[0].after.pid, 20);
}

#[test]
fn candidates_extracted_from_snapshot() {
    let source = Arc::new(VecSource::new(vec![
        sample(10, &["aider", "/tmp/repo"]),
        sample(11, &["bash"]),
        sample(12, &["node", "/opt/claude-code/bin"]),
    ]));
    let scanner = ProcessScanner::new(
        source as Arc<dyn ProcessSource>,
        Arc::new(SystemClock) as Arc<dyn Clock>,
        SnapshotSource::Mock,
    );
    let snap = scanner.scan_once();
    let candidates = extract_candidates(&snap, ALPHA_AGENT_PATTERNS);
    let pids: Vec<u32> = candidates.iter().map(|c| c.process.pid).collect();
    assert_eq!(pids, vec![10, 12]);
}

#[test]
fn sysinfo_source_sees_current_process() {
    let source = Arc::new(SysinfoProcessSource::new());
    let scanner = ProcessScanner::new(
        source as Arc<dyn ProcessSource>,
        Arc::new(SystemClock) as Arc<dyn Clock>,
        SnapshotSource::Sysinfo,
    );
    let snap = scanner.scan_once();
    assert!(
        snap.total_count() > 0,
        "sysinfo must report at least one process",
    );

    let me = std::process::id();
    assert!(
        snap.processes.iter().any(|p| p.pid == me),
        "sysinfo must list this test process (pid={me})",
    );
    assert_eq!(snap.source.label(), "sysinfo");
    assert!(
        snap.scan_duration_ms < 5_000,
        "scan should not block for 5s"
    );
}
