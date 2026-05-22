use std::collections::HashMap;

use agentdeck_core::ProcessInfo;
use chrono::{DateTime, Utc};
use serde::Serialize;

/// One point-in-time view of running processes.
///
/// Snapshots are produced by [`crate::ProcessScanner::scan_once`] and stored
/// briefly so that the next scan can produce a [`ProcessSnapshotDiff`].
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshot {
    /// UTC wall-clock time the snapshot finished. Sourced from the
    /// [`agentdeck_core::Clock`] passed to the scanner so tests can pin it.
    pub captured_at: DateTime<Utc>,
    /// Which backing [`agentdeck_core::ProcessSource`] produced this snapshot.
    pub source: SnapshotSource,
    /// Time taken to enumerate processes, measured with monotonic
    /// [`std::time::Instant`]s. Useful for spotting slow probes on Windows.
    pub scan_duration_ms: u64,
    /// All processes returned by the source, ordered by PID for determinism.
    pub processes: Vec<ProcessInfo>,
}

impl ProcessSnapshot {
    /// Number of processes in this snapshot.
    pub fn total_count(&self) -> usize {
        self.processes.len()
    }
}

/// Identifies the backing [`agentdeck_core::ProcessSource`] for a snapshot so
/// the UI can label which data path the user is looking at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotSource {
    /// Real OS data via the `sysinfo` crate.
    Sysinfo,
    /// In-memory mock used by the harness and tests.
    Mock,
    /// Future or test sources; carries an arbitrary label.
    Other(String),
}

impl SnapshotSource {
    pub fn label(&self) -> &str {
        match self {
            SnapshotSource::Sysinfo => "sysinfo",
            SnapshotSource::Mock => "mock",
            SnapshotSource::Other(s) => s.as_str(),
        }
    }
}

/// Difference between two consecutive [`ProcessSnapshot`]s.
///
/// Diffs are derived from the snapshots themselves (by PID), not provided by
/// the underlying source. Adapters and the attention engine consume these
/// events.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshotDiff {
    /// PIDs present in the new snapshot but absent from the previous one.
    pub started: Vec<ProcessInfo>,
    /// PIDs present in the previous snapshot but absent from the new one.
    pub exited: Vec<ProcessInfo>,
    /// PIDs in both snapshots whose `cmdline` changed (e.g. argv mutation
    /// via `prctl(PR_SET_NAME)` on Linux). Rare but worth surfacing.
    pub cmdline_changed: Vec<ProcessChange>,
    /// PIDs in both snapshots whose `cwd` changed (e.g. interactive shells).
    pub cwd_changed: Vec<ProcessChange>,
}

impl ProcessSnapshotDiff {
    pub fn is_empty(&self) -> bool {
        self.started.is_empty()
            && self.exited.is_empty()
            && self.cmdline_changed.is_empty()
            && self.cwd_changed.is_empty()
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessChange {
    pub before: ProcessInfo,
    pub after: ProcessInfo,
}

/// Pure-function diff over two snapshots. Both inputs are read-only; we do
/// not assume their `processes` lists are sorted (but the scanner does sort
/// them, which makes this cheap in practice).
pub fn diff(prev: &ProcessSnapshot, curr: &ProcessSnapshot) -> ProcessSnapshotDiff {
    let prev_map: HashMap<u32, &ProcessInfo> = prev.processes.iter().map(|p| (p.pid, p)).collect();
    let curr_map: HashMap<u32, &ProcessInfo> = curr.processes.iter().map(|p| (p.pid, p)).collect();

    let mut diff = ProcessSnapshotDiff::default();

    for (pid, info) in &curr_map {
        match prev_map.get(pid) {
            None => diff.started.push((*info).clone()),
            Some(before) => {
                if before.cmdline != info.cmdline {
                    diff.cmdline_changed.push(ProcessChange {
                        before: (*before).clone(),
                        after: (*info).clone(),
                    });
                }
                if before.cwd != info.cwd {
                    diff.cwd_changed.push(ProcessChange {
                        before: (*before).clone(),
                        after: (*info).clone(),
                    });
                }
            }
        }
    }
    for (pid, info) in &prev_map {
        if !curr_map.contains_key(pid) {
            diff.exited.push((*info).clone());
        }
    }

    diff.started.sort_by_key(|p| p.pid);
    diff.exited.sort_by_key(|p| p.pid);
    diff.cmdline_changed.sort_by_key(|c| c.after.pid);
    diff.cwd_changed.sort_by_key(|c| c.after.pid);

    diff
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample(pid: u32, cmd: &[&str], cwd: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid: None,
            name: cmd[0].to_string(),
            cmdline: cmd.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.map(|s| s.to_string()),
            started_at: Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
        }
    }

    fn snapshot(processes: Vec<ProcessInfo>) -> ProcessSnapshot {
        ProcessSnapshot {
            captured_at: Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
            source: SnapshotSource::Mock,
            scan_duration_ms: 0,
            processes,
        }
    }

    #[test]
    fn started_and_exited_are_detected() {
        let prev = snapshot(vec![sample(10, &["a"], None), sample(20, &["b"], None)]);
        let curr = snapshot(vec![sample(20, &["b"], None), sample(30, &["c"], None)]);
        let d = diff(&prev, &curr);
        assert_eq!(
            d.started.iter().map(|p| p.pid).collect::<Vec<_>>(),
            vec![30]
        );
        assert_eq!(d.exited.iter().map(|p| p.pid).collect::<Vec<_>>(), vec![10]);
        assert!(d.cmdline_changed.is_empty());
        assert!(d.cwd_changed.is_empty());
    }

    #[test]
    fn cmdline_change_is_detected() {
        let prev = snapshot(vec![sample(10, &["a", "foo"], None)]);
        let curr = snapshot(vec![sample(10, &["a", "bar"], None)]);
        let d = diff(&prev, &curr);
        assert!(d.started.is_empty());
        assert!(d.exited.is_empty());
        assert_eq!(d.cmdline_changed.len(), 1);
        assert_eq!(d.cmdline_changed[0].after.cmdline, vec!["a", "bar"]);
    }

    #[test]
    fn cwd_change_is_detected() {
        let prev = snapshot(vec![sample(10, &["bash"], Some("/home"))]);
        let curr = snapshot(vec![sample(10, &["bash"], Some("/tmp"))]);
        let d = diff(&prev, &curr);
        assert!(d.cmdline_changed.is_empty());
        assert_eq!(d.cwd_changed.len(), 1);
        assert_eq!(d.cwd_changed[0].before.cwd.as_deref(), Some("/home"));
        assert_eq!(d.cwd_changed[0].after.cwd.as_deref(), Some("/tmp"));
    }

    #[test]
    fn identical_snapshots_produce_empty_diff() {
        let s = snapshot(vec![sample(10, &["a"], None)]);
        assert!(diff(&s, &s).is_empty());
    }

    #[test]
    fn diff_lists_are_pid_sorted() {
        let prev = snapshot(vec![]);
        let curr = snapshot(vec![
            sample(30, &["c"], None),
            sample(10, &["a"], None),
            sample(20, &["b"], None),
        ]);
        let d = diff(&prev, &curr);
        let pids: Vec<u32> = d.started.iter().map(|p| p.pid).collect();
        assert_eq!(pids, vec![10, 20, 30]);
    }
}
