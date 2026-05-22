//! `agentdeck-adapter` runtime impl for user-defined custom adapters.
//!
//! The dashboard treats each `custom_adapters` row as a separate
//! user-visible entry, but the monitor core sees the whole set as one
//! class-level adapter named `"custom"`. This module wraps a snapshot of
//! the current row set in a single [`Adapter`] implementation:
//!
//! - [`CustomProcessAdapter::from_definitions`] compiles every *enabled*
//!   row's regex, stashes any compile failures so they can surface as
//!   diagnostic failure reasons, and silently skips disabled rows.
//! - [`Adapter::scan`] runs every compiled matcher against the snapshot
//!   and emits one [`AdapterMatch`] per (compiled, process) hit.
//!
//! The adapter is a *value*: the monitor core rebuilds it whenever the
//! underlying definitions change (add / delete / toggle). The trait does
//! not require interior mutability and we deliberately keep it that way.

use agentdeck_adapter::{
    Adapter, AdapterDiagnostic, AdapterMatch, AdapterScanResult, CapabilityLevel, Confidence,
    SessionStatus,
};
use agentdeck_process::ProcessSnapshot;
use uuid::Uuid;

use crate::definition::CustomAdapter;
use crate::matcher::{match_snapshot, CompiledAdapter};

/// Stable adapter name reported into `sessions.adapter_name` and
/// `adapter_diagnostics.adapter_name` for every custom-adapter match.
pub const ADAPTER_NAME: &str = "custom";

const STATUS_SOURCE: &str = "process-list";

/// Snapshot of the user's custom adapter definitions, ready to scan.
pub struct CustomProcessAdapter {
    compiled: Vec<CompiledAdapter>,
    /// Definitions whose regex failed to compile. Held as
    /// `(id, label, error_message)` so the diagnostic can surface a useful
    /// message without re-reading the row.
    compile_errors: Vec<(Uuid, String, String)>,
}

impl CustomProcessAdapter {
    /// Build a runtime adapter from the user's persisted definitions.
    ///
    /// Disabled definitions are skipped silently; enabled definitions
    /// whose regex fails to compile are recorded as failure reasons so
    /// the diagnostics card can show them.
    pub fn from_definitions(definitions: Vec<CustomAdapter>) -> Self {
        let mut compiled = Vec::new();
        let mut compile_errors = Vec::new();
        for def in definitions {
            if !def.enabled {
                continue;
            }
            let id = def.id;
            let label = def.label.clone();
            match CompiledAdapter::from_definition(def) {
                Ok(c) => compiled.push(c),
                Err(err) => compile_errors.push((id, label, err.to_string())),
            }
        }
        Self {
            compiled,
            compile_errors,
        }
    }

    /// Number of enabled definitions whose regex compiled successfully.
    pub fn compiled_count(&self) -> usize {
        self.compiled.len()
    }

    /// Number of enabled definitions whose regex failed to compile.
    pub fn compile_error_count(&self) -> usize {
        self.compile_errors.len()
    }
}

impl Adapter for CustomProcessAdapter {
    fn name(&self) -> &str {
        ADAPTER_NAME
    }

    fn capability_level(&self) -> CapabilityLevel {
        CapabilityLevel::Presence
    }

    fn scan(&self, snapshot: &ProcessSnapshot) -> AdapterScanResult {
        let custom_matches = match_snapshot(&self.compiled, snapshot);

        let matches: Vec<AdapterMatch> = custom_matches
            .into_iter()
            .map(|m| AdapterMatch {
                adapter_name: ADAPTER_NAME.to_string(),
                agent_name: m.agent_name,
                capability_level: CapabilityLevel::Presence,
                pid: m.process.pid,
                command: if m.process.cmdline.is_empty() {
                    m.process.name.clone()
                } else {
                    m.process.cmdline.join(" ")
                },
                cwd: m.process.cwd.clone(),
                repo_path: m.process.cwd.clone(),
                status: SessionStatus::Running,
                status_confidence: Confidence::Medium,
                status_source: STATUS_SOURCE.to_string(),
                cost_per_hour_cents: m.cost_per_hour_cents,
                observed_at: snapshot.captured_at,
            })
            .collect();

        let detected_count = matches.len() as u32;
        let failure_reasons: Vec<String> = self
            .compile_errors
            .iter()
            .map(|(id, label, err)| {
                format!("regex compile failed for custom adapter {label:?} ({id}): {err}")
            })
            .collect();

        AdapterScanResult {
            matches,
            diagnostic: AdapterDiagnostic {
                adapter_name: ADAPTER_NAME.to_string(),
                enabled: true,
                capability_level: CapabilityLevel::Presence,
                last_scan_time: snapshot.captured_at,
                detected_count,
                data_sources_used: vec![STATUS_SOURCE.to_string()],
                missing_permissions: Vec::new(),
                failure_reasons,
                confidence: Confidence::Medium,
                known_limitations: vec![
                    "presence-only".to_string(),
                    "no-status-classification".to_string(),
                    "no-usage-tracking".to_string(),
                    "no-control".to_string(),
                ],
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::{CustomAdapter, MatchKind};
    use agentdeck_core::ProcessInfo;
    use agentdeck_process::SnapshotSource;
    use chrono::{DateTime, TimeZone, Utc};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 13, 0, 0).unwrap()
    }

    fn def(label: &str, pattern: &str, enabled: bool) -> CustomAdapter {
        CustomAdapter {
            id: Uuid::new_v4(),
            label: label.into(),
            enabled,
            agent_name: label.into(),
            color: None,
            match_kind: MatchKind::Name,
            pattern: pattern.into(),
            cost_per_hour_cents: None,
            notes: None,
            created_at: now(),
            updated_at: now(),
        }
    }

    fn proc(pid: u32, name: &str, cmd: &[&str], cwd: Option<&str>) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid: None,
            name: name.into(),
            cmdline: cmd.iter().map(|s| s.to_string()).collect(),
            cwd: cwd.map(|s| s.to_string()),
            started_at: now(),
        }
    }

    fn snapshot(processes: Vec<ProcessInfo>) -> ProcessSnapshot {
        ProcessSnapshot {
            captured_at: now(),
            source: SnapshotSource::Mock,
            scan_duration_ms: 0,
            processes,
        }
    }

    #[test]
    fn name_and_level_are_constant() {
        let a = CustomProcessAdapter::from_definitions(Vec::new());
        assert_eq!(a.name(), "custom");
        assert_eq!(a.capability_level(), CapabilityLevel::Presence);
        assert!(a.enabled(), "the custom class is always enabled");
    }

    #[test]
    fn empty_definitions_yield_empty_matches_and_zero_detected() {
        let a = CustomProcessAdapter::from_definitions(Vec::new());
        let snap = snapshot(vec![proc(10, "aider", &["aider"], None)]);
        let result = a.scan(&snap);
        assert!(result.matches.is_empty());
        assert_eq!(result.diagnostic.detected_count, 0);
        assert!(result.diagnostic.failure_reasons.is_empty());
        assert_eq!(result.diagnostic.adapter_name, "custom");
        assert!(result.diagnostic.enabled);
    }

    #[test]
    fn enabled_definition_produces_one_match_per_hit() {
        let a = CustomProcessAdapter::from_definitions(vec![def("my-bot", "bot", true)]);
        assert_eq!(a.compiled_count(), 1);
        let snap = snapshot(vec![
            proc(10, "my-bot", &["my-bot"], Some("/repo")),
            proc(11, "bash", &["bash"], None),
        ]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 1);
        let m = &result.matches[0];
        assert_eq!(m.adapter_name, "custom");
        assert_eq!(m.agent_name, "my-bot");
        assert_eq!(m.pid, 10);
        assert_eq!(m.command, "my-bot");
        assert_eq!(m.cwd.as_deref(), Some("/repo"));
        assert_eq!(m.repo_path.as_deref(), Some("/repo"));
        assert_eq!(m.status, SessionStatus::Running);
        assert_eq!(m.status_confidence, Confidence::Medium);
        assert_eq!(m.status_source, "process-list");
        assert_eq!(m.observed_at, snap.captured_at);
        assert_eq!(result.diagnostic.detected_count, 1);
    }

    #[test]
    fn disabled_definition_is_skipped_at_compile_time() {
        let a = CustomProcessAdapter::from_definitions(vec![def("off", "off", false)]);
        assert_eq!(a.compiled_count(), 0);
        let snap = snapshot(vec![proc(10, "off", &["off"], None)]);
        let result = a.scan(&snap);
        assert!(result.matches.is_empty());
    }

    #[test]
    fn invalid_regex_surfaces_in_failure_reasons() {
        let mut bad = def("bad-row", "x", true);
        bad.pattern = "(unclosed".into();
        let a = CustomProcessAdapter::from_definitions(vec![bad]);
        assert_eq!(a.compiled_count(), 0);
        assert_eq!(a.compile_error_count(), 1);
        let result = a.scan(&snapshot(vec![]));
        assert!(result.matches.is_empty());
        assert_eq!(result.diagnostic.failure_reasons.len(), 1);
        assert!(result.diagnostic.failure_reasons[0].contains("bad-row"));
    }

    #[test]
    fn command_falls_back_to_name_when_cmdline_empty() {
        let a = CustomProcessAdapter::from_definitions(vec![def("agent-row", "agent", true)]);
        let snap = snapshot(vec![proc(10, "agent", &[], None)]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].command, "agent");
    }

    #[test]
    fn cmdline_joined_with_spaces() {
        let a = CustomProcessAdapter::from_definitions(vec![def("py", "py-bot", true)]);
        let snap = snapshot(vec![proc(
            10,
            "py-bot",
            &["python", "-m", "py-bot", "--watch"],
            None,
        )]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].command, "python -m py-bot --watch");
    }

    #[test]
    fn diagnostic_carries_snapshot_timestamp() {
        let a = CustomProcessAdapter::from_definitions(Vec::new());
        let snap = snapshot(vec![]);
        let result = a.scan(&snap);
        assert_eq!(result.diagnostic.last_scan_time, snap.captured_at);
    }

    #[test]
    fn diagnostic_lists_known_limitations() {
        let a = CustomProcessAdapter::from_definitions(Vec::new());
        let result = a.scan(&snapshot(vec![]));
        assert!(result
            .diagnostic
            .known_limitations
            .contains(&"presence-only".to_string()));
        assert!(result
            .diagnostic
            .known_limitations
            .contains(&"no-control".to_string()));
    }

    #[test]
    fn one_adapter_can_match_many_processes() {
        let a = CustomProcessAdapter::from_definitions(vec![def("agent", "agent", true)]);
        let snap = snapshot(vec![
            proc(10, "agent-a", &["agent-a"], None),
            proc(11, "agent-b", &["agent-b"], None),
            proc(12, "bash", &["bash"], None),
        ]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.diagnostic.detected_count, 2);
    }
}
