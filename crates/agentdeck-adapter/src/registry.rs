//! The [`AdapterRegistry`] holds every registered [`Adapter`] and drives
//! per-tick scans across the full set.

use agentdeck_process::ProcessSnapshot;
use serde::Serialize;

use crate::adapter::{Adapter, AdapterMatch};
use crate::diagnostics::AdapterDiagnostic;
use crate::types::Confidence;

/// Owns the live set of adapters and runs each enabled adapter against a
/// snapshot.
///
/// The registry is **append-only at runtime**: adapters are registered at
/// startup (the future monitor core constructs and installs each one) and
/// not removed mid-scan. Disabling an adapter is done through its own
/// [`Adapter::enabled`] hook, not by removing it from the registry — the
/// diagnostics card needs disabled adapters to keep appearing so the user
/// can see "this is off and here is why".
#[derive(Default)]
pub struct AdapterRegistry {
    adapters: Vec<Box<dyn Adapter>>,
}

impl AdapterRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an adapter to the registry. Registration order determines the
    /// order of matches and diagnostics in [`Self::scan_all`].
    pub fn register(&mut self, adapter: Box<dyn Adapter>) {
        self.adapters.push(adapter);
    }

    /// Number of registered adapters (enabled or disabled).
    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    /// Borrow the underlying adapter list. The registry intentionally does
    /// not expose mutable access; runtime toggles flow through
    /// [`Adapter::enabled`].
    pub fn adapters(&self) -> &[Box<dyn Adapter>] {
        &self.adapters
    }

    /// Run every adapter against a single snapshot.
    ///
    /// Enabled adapters contribute matches *and* a diagnostic; disabled
    /// adapters contribute only a "disabled" diagnostic so the dashboard
    /// can still render their state. Adapter scan order is preserved
    /// across both lists.
    pub fn scan_all(&self, snapshot: &ProcessSnapshot) -> RegistryScanResult {
        let mut matches: Vec<AdapterMatch> = Vec::new();
        let mut diagnostics: Vec<AdapterDiagnostic> = Vec::with_capacity(self.adapters.len());

        for adapter in &self.adapters {
            if adapter.enabled() {
                let mut result = adapter.scan(snapshot);
                diagnostics.push(result.diagnostic);
                matches.append(&mut result.matches);
            } else {
                diagnostics.push(disabled_diagnostic(adapter.as_ref(), snapshot));
            }
        }

        RegistryScanResult {
            matches,
            diagnostics,
        }
    }
}

/// Output of [`AdapterRegistry::scan_all`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryScanResult {
    /// Every match from every enabled adapter, in registration order.
    pub matches: Vec<AdapterMatch>,
    /// One diagnostic per registered adapter (enabled or disabled), in
    /// registration order.
    pub diagnostics: Vec<AdapterDiagnostic>,
}

fn disabled_diagnostic(adapter: &dyn Adapter, snapshot: &ProcessSnapshot) -> AdapterDiagnostic {
    AdapterDiagnostic {
        adapter_name: adapter.name().to_string(),
        enabled: false,
        capability_level: adapter.capability_level(),
        last_scan_time: snapshot.captured_at,
        detected_count: 0,
        data_sources_used: Vec::new(),
        missing_permissions: Vec::new(),
        failure_reasons: Vec::new(),
        confidence: Confidence::Unknown,
        known_limitations: vec!["adapter disabled".to_string()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::AdapterScanResult;
    use crate::types::{CapabilityLevel, SessionStatus};
    use agentdeck_process::SnapshotSource;
    use chrono::{TimeZone, Utc};

    struct StubAdapter {
        name: &'static str,
        level: CapabilityLevel,
        enabled: bool,
        matches: Vec<AdapterMatch>,
    }

    impl Adapter for StubAdapter {
        fn name(&self) -> &str {
            self.name
        }
        fn capability_level(&self) -> CapabilityLevel {
            self.level
        }
        fn enabled(&self) -> bool {
            self.enabled
        }
        fn scan(&self, snapshot: &ProcessSnapshot) -> AdapterScanResult {
            AdapterScanResult {
                matches: self.matches.clone(),
                diagnostic: AdapterDiagnostic {
                    adapter_name: self.name.to_string(),
                    enabled: self.enabled,
                    capability_level: self.level,
                    last_scan_time: snapshot.captured_at,
                    detected_count: self.matches.len() as u32,
                    data_sources_used: vec!["test".to_string()],
                    missing_permissions: Vec::new(),
                    failure_reasons: Vec::new(),
                    confidence: Confidence::Medium,
                    known_limitations: Vec::new(),
                },
            }
        }
    }

    fn empty_snapshot() -> ProcessSnapshot {
        ProcessSnapshot {
            captured_at: Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
            source: SnapshotSource::Mock,
            scan_duration_ms: 0,
            processes: Vec::new(),
        }
    }

    fn sample_match(pid: u32, name: &str) -> AdapterMatch {
        AdapterMatch {
            adapter_name: "stub".into(),
            agent_name: name.into(),
            capability_level: CapabilityLevel::Presence,
            pid,
            command: name.into(),
            cwd: None,
            repo_path: None,
            status: SessionStatus::Running,
            status_confidence: Confidence::Medium,
            status_source: "test".into(),
            cost_per_hour_cents: None,
            observed_at: Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
        }
    }

    #[test]
    fn empty_registry_scans_to_empty() {
        let reg = AdapterRegistry::new();
        let result = reg.scan_all(&empty_snapshot());
        assert!(result.matches.is_empty());
        assert!(result.diagnostics.is_empty());
    }

    #[test]
    fn registered_adapter_contributes_match_and_diagnostic() {
        let mut reg = AdapterRegistry::new();
        reg.register(Box::new(StubAdapter {
            name: "alpha",
            level: CapabilityLevel::Presence,
            enabled: true,
            matches: vec![sample_match(10, "alpha")],
        }));
        let result = reg.scan_all(&empty_snapshot());
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].pid, 10);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].adapter_name, "alpha");
        assert!(result.diagnostics[0].enabled);
        assert_eq!(result.diagnostics[0].detected_count, 1);
    }

    #[test]
    fn disabled_adapter_skipped_for_matches_but_diagnostic_emitted() {
        let mut reg = AdapterRegistry::new();
        reg.register(Box::new(StubAdapter {
            name: "off",
            level: CapabilityLevel::Status,
            enabled: false,
            matches: vec![sample_match(20, "off")],
        }));
        let result = reg.scan_all(&empty_snapshot());
        assert!(result.matches.is_empty());
        assert_eq!(result.diagnostics.len(), 1);
        let d = &result.diagnostics[0];
        assert_eq!(d.adapter_name, "off");
        assert!(!d.enabled);
        assert_eq!(d.detected_count, 0);
        assert_eq!(d.capability_level, CapabilityLevel::Status);
        assert_eq!(d.confidence, Confidence::Unknown);
        assert!(d.known_limitations.iter().any(|l| l.contains("disabled")));
    }

    #[test]
    fn registration_order_is_preserved() {
        let mut reg = AdapterRegistry::new();
        reg.register(Box::new(StubAdapter {
            name: "a",
            level: CapabilityLevel::Presence,
            enabled: true,
            matches: vec![sample_match(1, "a")],
        }));
        reg.register(Box::new(StubAdapter {
            name: "b",
            level: CapabilityLevel::Presence,
            enabled: true,
            matches: vec![sample_match(2, "b")],
        }));
        reg.register(Box::new(StubAdapter {
            name: "c",
            level: CapabilityLevel::Presence,
            enabled: false,
            matches: vec![sample_match(3, "c")],
        }));
        let result = reg.scan_all(&empty_snapshot());

        let match_names: Vec<&str> = result
            .matches
            .iter()
            .map(|m| m.agent_name.as_str())
            .collect();
        assert_eq!(match_names, vec!["a", "b"]);

        let diag_names: Vec<&str> = result
            .diagnostics
            .iter()
            .map(|d| d.adapter_name.as_str())
            .collect();
        assert_eq!(diag_names, vec!["a", "b", "c"]);
        assert!(!result.diagnostics[2].enabled);
    }

    #[test]
    fn registry_len_and_is_empty() {
        let mut reg = AdapterRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        reg.register(Box::new(StubAdapter {
            name: "x",
            level: CapabilityLevel::Presence,
            enabled: true,
            matches: vec![],
        }));
        assert!(!reg.is_empty());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn disabled_diagnostic_uses_snapshot_timestamp() {
        let mut reg = AdapterRegistry::new();
        reg.register(Box::new(StubAdapter {
            name: "z",
            level: CapabilityLevel::Presence,
            enabled: false,
            matches: vec![],
        }));
        let snap = empty_snapshot();
        let result = reg.scan_all(&snap);
        assert_eq!(result.diagnostics[0].last_scan_time, snap.captured_at);
    }
}
