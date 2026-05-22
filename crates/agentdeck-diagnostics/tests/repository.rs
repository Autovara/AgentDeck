//! Integration tests for the adapter-diagnostics repository.
//!
//! Drive the public `record` / `list` API against an in-memory
//! `Storage`. The tests assert the §17 "keep only the most recent
//! scan per adapter" discipline, JSON encoding, and timestamp
//! round-tripping.

use std::sync::Arc;

use agentdeck_adapter::{AdapterDiagnostic, CapabilityLevel, Confidence};
use agentdeck_diagnostics::{list, record};
use agentdeck_storage::Storage;
use chrono::{DateTime, TimeZone, Utc};

fn storage() -> Arc<Storage> {
    Arc::new(Storage::open_in_memory().expect("open in-memory storage"))
}

fn ts(h: u32, m: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 22, h, m, s).unwrap()
}

fn diag(
    name: &str,
    enabled: bool,
    level: CapabilityLevel,
    last_scan: DateTime<Utc>,
    detected: u32,
    confidence: Confidence,
) -> AdapterDiagnostic {
    AdapterDiagnostic {
        adapter_name: name.to_string(),
        enabled,
        capability_level: level,
        last_scan_time: last_scan,
        detected_count: detected,
        data_sources_used: vec!["process-list".to_string()],
        missing_permissions: Vec::new(),
        failure_reasons: Vec::new(),
        confidence,
        known_limitations: vec!["Linux only".to_string()],
    }
}

#[test]
fn empty_batch_is_noop() {
    let s = storage();
    record(&s, &[]).expect("empty record is fine");
    assert!(list(&s).expect("list").is_empty());
}

#[test]
fn record_then_list_round_trips() {
    let s = storage();
    let d1 = diag(
        "custom",
        true,
        CapabilityLevel::Presence,
        ts(12, 0, 0),
        2,
        Confidence::Medium,
    );
    let d2 = diag(
        "aider",
        false,
        CapabilityLevel::Status,
        ts(12, 0, 30),
        0,
        Confidence::Unknown,
    );
    record(&s, &[d1.clone(), d2.clone()]).expect("record batch");

    let rows = list(&s).expect("list");
    assert_eq!(rows.len(), 2);
    // Oldest scan first.
    assert_eq!(rows[0].adapter_name, "custom");
    assert_eq!(rows[1].adapter_name, "aider");
    // Fields preserved.
    assert_eq!(rows[0], d1);
    assert_eq!(rows[1], d2);
}

#[test]
fn second_record_overwrites_same_adapter_row() {
    let s = storage();
    let first = diag(
        "custom",
        true,
        CapabilityLevel::Presence,
        ts(12, 0, 0),
        2,
        Confidence::Medium,
    );
    record(&s, std::slice::from_ref(&first)).expect("record");

    // The second tick reports the same adapter with a newer scan time
    // and a higher detected count. The §17 discipline says we keep only
    // the most recent row.
    let second = AdapterDiagnostic {
        last_scan_time: ts(12, 0, 5),
        detected_count: 5,
        failure_reasons: vec!["regex /badpat/ failed to compile".to_string()],
        ..first.clone()
    };
    record(&s, std::slice::from_ref(&second)).expect("record again");

    let rows = list(&s).expect("list");
    assert_eq!(rows.len(), 1, "no history; one row per adapter_name");
    assert_eq!(rows[0], second);
}

#[test]
fn missing_adapter_in_next_batch_keeps_previous_row() {
    // If an adapter was registered last tick but missing this tick
    // (because the orchestrator chose not to run it), the previously
    // recorded row stays — the user should still see the most recent
    // diagnostic for it.
    let s = storage();
    let custom = diag(
        "custom",
        true,
        CapabilityLevel::Presence,
        ts(12, 0, 0),
        2,
        Confidence::Medium,
    );
    let aider = diag(
        "aider",
        true,
        CapabilityLevel::Status,
        ts(12, 0, 0),
        0,
        Confidence::Unknown,
    );
    record(&s, &[custom.clone(), aider.clone()]).expect("record");

    // Next tick: aider not in the batch.
    let custom2 = AdapterDiagnostic {
        last_scan_time: ts(12, 1, 0),
        ..custom.clone()
    };
    record(&s, std::slice::from_ref(&custom2)).expect("record");

    let mut rows = list(&s).expect("list");
    rows.sort_by(|a, b| a.adapter_name.cmp(&b.adapter_name));
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0], aider, "aider's previous row is preserved");
    assert_eq!(rows[1], custom2, "custom is refreshed");
}

#[test]
fn json_array_columns_round_trip_with_empty_vec() {
    let s = storage();
    let d = AdapterDiagnostic {
        adapter_name: "custom".to_string(),
        enabled: true,
        capability_level: CapabilityLevel::Presence,
        last_scan_time: ts(12, 0, 0),
        detected_count: 0,
        data_sources_used: Vec::new(),
        missing_permissions: Vec::new(),
        failure_reasons: Vec::new(),
        confidence: Confidence::Medium,
        known_limitations: Vec::new(),
    };
    record(&s, std::slice::from_ref(&d)).expect("record");
    let rows = list(&s).expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], d);
}

#[test]
fn populated_json_array_columns_round_trip() {
    let s = storage();
    let d = AdapterDiagnostic {
        adapter_name: "claude".to_string(),
        enabled: true,
        capability_level: CapabilityLevel::Status,
        last_scan_time: ts(12, 0, 0),
        detected_count: 3,
        data_sources_used: vec!["process-list".to_string(), "tty-buffer".to_string()],
        missing_permissions: vec!["read /proc/<pid>/cwd".to_string()],
        failure_reasons: vec!["log file rotated".to_string()],
        confidence: Confidence::High,
        known_limitations: vec!["Linux only".to_string(), "TTY required".to_string()],
    };
    record(&s, std::slice::from_ref(&d)).expect("record");
    let rows = list(&s).expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0], d);
}

#[test]
fn all_capability_levels_round_trip() {
    let s = storage();
    let batch: Vec<AdapterDiagnostic> = [
        ("a", CapabilityLevel::Presence),
        ("b", CapabilityLevel::Status),
        ("c", CapabilityLevel::Usage),
        ("d", CapabilityLevel::Control),
    ]
    .into_iter()
    .map(|(name, level)| diag(name, true, level, ts(12, 0, 0), 0, Confidence::Medium))
    .collect();
    record(&s, &batch).expect("record");
    let mut rows = list(&s).expect("list");
    rows.sort_by(|a, b| a.adapter_name.cmp(&b.adapter_name));
    for (got, expected) in rows.iter().zip(batch.iter()) {
        assert_eq!(got.capability_level, expected.capability_level);
    }
}

#[test]
fn all_confidence_values_round_trip() {
    let s = storage();
    let batch: Vec<AdapterDiagnostic> = [
        ("a", Confidence::High),
        ("b", Confidence::Medium),
        ("c", Confidence::Low),
        ("d", Confidence::Unknown),
    ]
    .into_iter()
    .map(|(name, conf)| diag(name, true, CapabilityLevel::Presence, ts(12, 0, 0), 0, conf))
    .collect();
    record(&s, &batch).expect("record");
    let mut rows = list(&s).expect("list");
    rows.sort_by(|a, b| a.adapter_name.cmp(&b.adapter_name));
    for (got, expected) in rows.iter().zip(batch.iter()) {
        assert_eq!(got.confidence, expected.confidence);
    }
}

#[test]
fn list_orders_by_scan_time_ascending() {
    let s = storage();
    let early = diag(
        "alpha",
        true,
        CapabilityLevel::Presence,
        ts(11, 0, 0),
        0,
        Confidence::Medium,
    );
    let middle = diag(
        "beta",
        true,
        CapabilityLevel::Presence,
        ts(12, 0, 0),
        0,
        Confidence::Medium,
    );
    let late = diag(
        "gamma",
        true,
        CapabilityLevel::Presence,
        ts(13, 0, 0),
        0,
        Confidence::Medium,
    );
    // Insert in non-monotonic order to make sure the ORDER BY clause
    // does the work, not insertion order.
    record(&s, &[late.clone(), early.clone(), middle.clone()]).expect("record");

    let rows = list(&s).expect("list");
    let names: Vec<&str> = rows.iter().map(|r| r.adapter_name.as_str()).collect();
    assert_eq!(names, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn disabled_flag_round_trips() {
    let s = storage();
    let enabled = diag(
        "on",
        true,
        CapabilityLevel::Presence,
        ts(12, 0, 0),
        0,
        Confidence::Medium,
    );
    let disabled = diag(
        "off",
        false,
        CapabilityLevel::Presence,
        ts(12, 0, 0),
        0,
        Confidence::Unknown,
    );
    record(&s, &[enabled.clone(), disabled.clone()]).expect("record");
    let mut rows = list(&s).expect("list");
    rows.sort_by(|a, b| a.adapter_name.cmp(&b.adapter_name));
    assert!(!rows[0].enabled, "off row");
    assert!(rows[1].enabled, "on row");
}
