//! End-to-end test: persist custom adapter definitions in SQLite, load
//! them back, build a [`CustomProcessAdapter`], and scan a snapshot
//! through the [`agentdeck_adapter::Adapter`] trait.
//!
//! This exists so we never regress the full storage → matcher → adapter
//! path: each individual layer has its own unit tests; this test proves
//! they compose.

use agentdeck_adapter::{Adapter, AdapterRegistry, CapabilityLevel};
use agentdeck_adapter_custom::{
    CustomAdapterRepository, CustomProcessAdapter, MatchKind, NewCustomAdapter, ADAPTER_NAME,
};
use agentdeck_core::{Clock, ProcessInfo};
use agentdeck_process::{ProcessSnapshot, SnapshotSource};
use agentdeck_storage::Storage;
use chrono::{DateTime, TimeZone, Utc};

struct FixedClock(DateTime<Utc>);
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

fn make_input(label: &str, pattern: &str, enabled: bool) -> NewCustomAdapter {
    NewCustomAdapter {
        label: label.into(),
        agent_name: label.into(),
        match_kind: MatchKind::Name,
        pattern: pattern.into(),
        enabled,
        color: None,
        cost_per_hour_cents: None,
        notes: None,
    }
}

fn proc(pid: u32, name: &str, cmd: &[&str], cwd: Option<&str>) -> ProcessInfo {
    ProcessInfo {
        pid,
        parent_pid: None,
        name: name.into(),
        cmdline: cmd.iter().map(|s| s.to_string()).collect(),
        cwd: cwd.map(|s| s.to_string()),
        started_at: Utc.with_ymd_and_hms(2026, 5, 22, 13, 0, 0).unwrap(),
    }
}

fn snapshot(processes: Vec<ProcessInfo>) -> ProcessSnapshot {
    ProcessSnapshot {
        captured_at: Utc.with_ymd_and_hms(2026, 5, 22, 14, 0, 0).unwrap(),
        source: SnapshotSource::Mock,
        scan_duration_ms: 0,
        processes,
    }
}

#[test]
fn end_to_end_storage_to_adapter_scan() {
    let storage = Storage::open_in_memory().expect("in-memory db");
    let repo = CustomAdapterRepository::new(&storage);
    let clock = FixedClock(Utc.with_ymd_and_hms(2026, 5, 22, 13, 0, 0).unwrap());

    let on = make_input("aider-dev", "aider", true)
        .validate()
        .expect("validate on");
    repo.add(on, &clock).expect("add on");

    let off = make_input("codex-off", "codex", false)
        .validate()
        .expect("validate off");
    repo.add(off, &clock).expect("add off");

    let definitions = repo.list().expect("list");
    assert_eq!(definitions.len(), 2);

    let adapter = CustomProcessAdapter::from_definitions(definitions);
    assert_eq!(
        adapter.compiled_count(),
        1,
        "only the enabled row should compile"
    );
    assert_eq!(adapter.name(), ADAPTER_NAME);
    assert_eq!(adapter.capability_level(), CapabilityLevel::Presence);

    let snap = snapshot(vec![
        proc(100, "aider", &["aider", "/repo"], Some("/repo")),
        proc(101, "codex", &["codex"], None),
        proc(102, "bash", &["bash"], None),
    ]);
    let result = adapter.scan(&snap);

    assert_eq!(result.matches.len(), 1, "disabled row must not match");
    let m = &result.matches[0];
    assert_eq!(m.adapter_name, ADAPTER_NAME);
    assert_eq!(m.agent_name, "aider-dev");
    assert_eq!(m.pid, 100);
    assert_eq!(m.command, "aider /repo");
    assert_eq!(m.cwd.as_deref(), Some("/repo"));
    assert_eq!(m.repo_path.as_deref(), Some("/repo"));
    assert_eq!(m.observed_at, snap.captured_at);

    assert!(result.diagnostic.enabled);
    assert_eq!(result.diagnostic.adapter_name, ADAPTER_NAME);
    assert_eq!(result.diagnostic.detected_count, 1);
    assert!(result.diagnostic.failure_reasons.is_empty());
    assert_eq!(result.diagnostic.last_scan_time, snap.captured_at);
}

#[test]
fn registry_runs_custom_adapter_among_others() {
    let storage = Storage::open_in_memory().expect("in-memory db");
    let repo = CustomAdapterRepository::new(&storage);
    let clock = FixedClock(Utc.with_ymd_and_hms(2026, 5, 22, 13, 0, 0).unwrap());

    repo.add(
        make_input("aider-dev", "aider", true)
            .validate()
            .expect("validate"),
        &clock,
    )
    .expect("add");

    let custom = CustomProcessAdapter::from_definitions(repo.list().expect("list"));

    let mut registry = AdapterRegistry::new();
    registry.register(Box::new(custom));
    assert_eq!(registry.len(), 1);

    let snap = snapshot(vec![proc(100, "aider", &["aider"], Some("/repo"))]);
    let result = registry.scan_all(&snap);

    assert_eq!(result.matches.len(), 1);
    assert_eq!(result.matches[0].adapter_name, ADAPTER_NAME);
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].adapter_name, ADAPTER_NAME);
    assert_eq!(result.diagnostics[0].detected_count, 1);
}
