//! Match custom adapters against [`ProcessSnapshot`]s.
//!
//! The matcher is intentionally a pure function: it takes a slice of
//! pre-compiled [`CompiledAdapter`]s and a `&ProcessSnapshot`, and returns
//! every (adapter, process) pair where the adapter's regex matched. Adapters
//! are compiled once and re-used across many snapshots; recompiling every
//! tick would be expensive at hundreds-of-processes scale.

use std::collections::BTreeMap;

use agentdeck_core::ProcessInfo;
use agentdeck_process::ProcessSnapshot;
use regex::Regex;
use serde::Serialize;
use uuid::Uuid;

use crate::definition::{CustomAdapter, CustomAdapterError, MatchKind};

/// A [`CustomAdapter`] plus its compiled regex.
///
/// Compilation can fail if the row in SQLite was somehow inserted with an
/// invalid regex (the application path validates beforehand, but the column
/// stores arbitrary text). [`CompiledAdapter::from_definition`] surfaces the
/// failure.
#[derive(Debug, Clone)]
pub struct CompiledAdapter {
    pub definition: CustomAdapter,
    pub(crate) regex: Regex,
}

impl CompiledAdapter {
    pub fn from_definition(def: CustomAdapter) -> Result<Self, CustomAdapterError> {
        let regex = Regex::new(&def.pattern)?;
        Ok(Self {
            definition: def,
            regex,
        })
    }

    pub fn matches(&self, proc: &ProcessInfo) -> bool {
        match self.definition.match_kind {
            MatchKind::Name => self.regex.is_match(&proc.name),
            MatchKind::Cmdline => {
                let joined = proc.cmdline.join(" ");
                self.regex.is_match(&joined)
            }
            MatchKind::Cwd => proc
                .cwd
                .as_deref()
                .map(|c| self.regex.is_match(c))
                .unwrap_or(false),
        }
    }
}

/// One matched (adapter, process) pair plus the field that matched.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAdapterMatch {
    pub adapter_id: Uuid,
    pub adapter_label: String,
    pub agent_name: String,
    pub match_kind: MatchKind,
    pub process: ProcessInfo,
    /// User-configured rate from the `custom_adapters.cost_per_hour_cents`
    /// column. Threaded through to [`agentdeck_adapter::AdapterMatch`]
    /// so the session state machine can populate
    /// `sessions.estimated_cost`.
    pub cost_per_hour_cents: Option<i64>,
}

/// Run every enabled adapter against the snapshot and return all matches.
///
/// Adapter order is preserved (most-recently-created adapters last); within
/// each adapter, matches are ordered by PID.
pub fn match_snapshot(
    adapters: &[CompiledAdapter],
    snapshot: &ProcessSnapshot,
) -> Vec<CustomAdapterMatch> {
    let mut out = Vec::new();
    for adapter in adapters {
        for proc in &snapshot.processes {
            if adapter.matches(proc) {
                out.push(CustomAdapterMatch {
                    adapter_id: adapter.definition.id,
                    adapter_label: adapter.definition.label.clone(),
                    agent_name: adapter.definition.agent_name.clone(),
                    match_kind: adapter.definition.match_kind,
                    process: proc.clone(),
                    cost_per_hour_cents: adapter.definition.cost_per_hour_cents,
                });
            }
        }
    }
    out
}

/// Group matches by adapter id. Useful for the dashboard card which renders
/// one row per adapter with the matched PIDs underneath.
pub fn group_by_adapter(
    matches: &[CustomAdapterMatch],
) -> BTreeMap<Uuid, Vec<&CustomAdapterMatch>> {
    let mut out: BTreeMap<Uuid, Vec<&CustomAdapterMatch>> = BTreeMap::new();
    for m in matches {
        out.entry(m.adapter_id).or_default().push(m);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::CustomAdapter;
    use agentdeck_process::SnapshotSource;
    use chrono::{TimeZone, Utc};

    fn proc(pid: u32, name: &str, cmd: &[&str], cwd: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid: None,
            name: name.into(),
            cmdline: cmd.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.map(|s| s.to_string()),
            started_at: Utc.with_ymd_and_hms(2026, 5, 22, 12, 0, 0).unwrap(),
        }
    }

    fn snapshot(processes: Vec<ProcessInfo>) -> ProcessSnapshot {
        ProcessSnapshot {
            captured_at: Utc.with_ymd_and_hms(2026, 5, 22, 12, 0, 0).unwrap(),
            source: SnapshotSource::Mock,
            scan_duration_ms: 0,
            processes,
        }
    }

    fn adapter(label: &str, kind: MatchKind, pattern: &str) -> CompiledAdapter {
        CompiledAdapter::from_definition(CustomAdapter {
            id: Uuid::new_v4(),
            label: label.into(),
            enabled: true,
            agent_name: label.into(),
            color: None,
            match_kind: kind,
            pattern: pattern.into(),
            cost_per_hour_cents: None,
            notes: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
        .expect("compiles")
    }

    #[test]
    fn name_matcher_matches_substring_by_default() {
        let a = adapter("my-bot", MatchKind::Name, "bot");
        let snap = snapshot(vec![
            proc(10, "my-bot", &["my-bot"], None),
            proc(11, "bash", &["bash"], None),
        ]);
        let matches = match_snapshot(&[a], &snap);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].process.pid, 10);
    }

    #[test]
    fn cmdline_matcher_inspects_joined_args() {
        let a = adapter("py-bot", MatchKind::Cmdline, r"py-bot\.py\b");
        let snap = snapshot(vec![
            proc(10, "python", &["python", "/opt/py-bot.py", "--watch"], None),
            proc(11, "python", &["python", "/opt/other.py"], None),
        ]);
        let matches = match_snapshot(&[a], &snap);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].process.pid, 10);
    }

    #[test]
    fn cwd_matcher_skips_processes_without_cwd() {
        let a = adapter("repo-bot", MatchKind::Cwd, "^/tmp/repo$");
        let snap = snapshot(vec![
            proc(10, "bash", &["bash"], Some("/tmp/repo")),
            proc(11, "bash", &["bash"], None),
            proc(12, "bash", &["bash"], Some("/elsewhere")),
        ]);
        let matches = match_snapshot(&[a], &snap);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].process.pid, 10);
    }

    #[test]
    fn multiple_adapters_produce_separate_matches() {
        let a = adapter("a", MatchKind::Name, "agent-a");
        let b = adapter("b", MatchKind::Name, "agent-b");
        let snap = snapshot(vec![
            proc(10, "agent-a", &["agent-a"], None),
            proc(20, "agent-b", &["agent-b"], None),
            proc(30, "agent-c", &["agent-c"], None),
        ]);
        let matches = match_snapshot(&[a, b], &snap);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].process.pid, 10);
        assert_eq!(matches[1].process.pid, 20);
    }

    #[test]
    fn one_adapter_can_match_many_processes() {
        let a = adapter("agent", MatchKind::Name, "agent");
        let snap = snapshot(vec![
            proc(10, "agent-a", &["agent-a"], None),
            proc(11, "agent-b", &["agent-b"], None),
            proc(12, "unrelated", &["unrelated"], None),
        ]);
        let matches = match_snapshot(&[a], &snap);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn empty_snapshot_produces_no_matches() {
        let a = adapter("agent", MatchKind::Name, "agent");
        let matches = match_snapshot(&[a], &snapshot(vec![]));
        assert!(matches.is_empty());
    }

    #[test]
    fn empty_adapter_set_produces_no_matches() {
        let snap = snapshot(vec![proc(10, "agent-a", &["agent-a"], None)]);
        let matches = match_snapshot(&[], &snap);
        assert!(matches.is_empty());
    }

    #[test]
    fn group_by_adapter_buckets_correctly() {
        let a = adapter("a", MatchKind::Name, "x");
        let id = a.definition.id;
        let snap = snapshot(vec![
            proc(10, "x1", &["x"], None),
            proc(11, "x2", &["x"], None),
        ]);
        let matches = match_snapshot(&[a], &snap);
        let grouped = group_by_adapter(&matches);
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped.get(&id).unwrap().len(), 2);
    }
}
