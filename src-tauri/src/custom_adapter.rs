//! Bridge between the Tauri shell and `agentdeck-adapter-custom`.
//!
//! Holds an `Arc<Storage>` plus the most recent cached
//! [`CustomAdapterReport`] so the dashboard can refresh on demand.

use std::sync::{Arc, Mutex};

use agentdeck_adapter_custom::{
    group_by_adapter, match_snapshot, CompiledAdapter, CustomAdapter, CustomAdapterError,
    CustomAdapterMatch, CustomAdapterRepository, MatchKind, NewCustomAdapter,
};
use agentdeck_core::SystemClock;
use agentdeck_process::ProcessSnapshot;
use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::process_scanner::ProcessScannerState;

/// Dashboard payload for the "Custom adapters" card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAdapterReport {
    /// `true` when the storage backend is available; `false` when the DB
    /// failed to open at startup. Mirrors `StorageReport.ready`.
    pub ready: bool,
    /// All defined adapters, enabled first.
    pub adapters: Vec<CustomAdapterSummary>,
    /// Whether the report includes match data from the latest snapshot.
    pub last_match_ran: bool,
    /// UTC timestamp the report was assembled.
    pub captured_at: DateTime<Utc>,
    /// Storage / repository / matcher error, if any.
    pub error: Option<String>,
}

/// One adapter row plus its current matches.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAdapterSummary {
    pub id: Uuid,
    pub label: String,
    pub agent_name: String,
    pub enabled: bool,
    pub color: Option<String>,
    pub match_kind: MatchKind,
    pub pattern: String,
    pub cost_per_hour_cents: Option<i64>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// PIDs from the most recent snapshot that matched this adapter.
    pub matched_pids: Vec<u32>,
    /// `true` if the adapter's regex failed to compile from the stored
    /// pattern. Disables matching for the row but does not remove it from
    /// the list.
    pub regex_compile_error: Option<String>,
}

/// Tauri-managed state. Holds the storage handle and the latest report.
pub struct CustomAdapterState {
    inner: Mutex<Inner>,
}

struct Inner {
    storage: Option<Arc<Storage>>,
    last_report: CustomAdapterReport,
}

impl CustomAdapterState {
    pub fn new(storage: Option<Arc<Storage>>) -> Self {
        let ready = storage.is_some();
        Self {
            inner: Mutex::new(Inner {
                storage,
                last_report: CustomAdapterReport {
                    ready,
                    adapters: Vec::new(),
                    last_match_ran: false,
                    captured_at: Utc::now(),
                    error: if ready {
                        None
                    } else {
                        Some("storage unavailable".to_string())
                    },
                },
            }),
        }
    }

    /// Recompute the report by listing all adapters, taking a fresh process
    /// snapshot, and running the matcher.
    pub fn refresh(&self, scanner: &ProcessScannerState) -> CustomAdapterReport {
        let storage = {
            let guard = self.inner.lock().expect("custom adapter state poisoned");
            guard.storage.clone()
        };

        let report = match storage {
            Some(storage) => build_report(&storage, scanner),
            None => CustomAdapterReport {
                ready: false,
                adapters: Vec::new(),
                last_match_ran: false,
                captured_at: Utc::now(),
                error: Some("storage unavailable".to_string()),
            },
        };

        let mut guard = self.inner.lock().expect("custom adapter state poisoned");
        guard.last_report = report.clone();
        report
    }

    /// Insert a new adapter; on success refreshes the cached report.
    pub fn add(
        &self,
        input: NewCustomAdapter,
        scanner: &ProcessScannerState,
    ) -> Result<CustomAdapterReport, String> {
        let storage = self
            .storage_clone()
            .ok_or_else(|| "storage unavailable".to_string())?;

        let validated = input.validate().map_err(|e| e.to_string())?;
        let repo = CustomAdapterRepository::new(&storage);
        repo.add(validated, &SystemClock).map_err(stringify_err)?;
        Ok(self.refresh(scanner))
    }

    pub fn delete(
        &self,
        id: Uuid,
        scanner: &ProcessScannerState,
    ) -> Result<CustomAdapterReport, String> {
        let storage = self
            .storage_clone()
            .ok_or_else(|| "storage unavailable".to_string())?;
        let repo = CustomAdapterRepository::new(&storage);
        repo.delete(id).map_err(stringify_err)?;
        Ok(self.refresh(scanner))
    }

    pub fn set_enabled(
        &self,
        id: Uuid,
        enabled: bool,
        scanner: &ProcessScannerState,
    ) -> Result<CustomAdapterReport, String> {
        let storage = self
            .storage_clone()
            .ok_or_else(|| "storage unavailable".to_string())?;
        let repo = CustomAdapterRepository::new(&storage);
        repo.set_enabled(id, enabled, &SystemClock)
            .map_err(stringify_err)?;
        Ok(self.refresh(scanner))
    }

    fn storage_clone(&self) -> Option<Arc<Storage>> {
        self.inner
            .lock()
            .ok()
            .and_then(|g| g.storage.as_ref().cloned())
    }
}

fn build_report(storage: &Storage, scanner: &ProcessScannerState) -> CustomAdapterReport {
    let repo = CustomAdapterRepository::new(storage);
    let all = match repo.list() {
        Ok(rows) => rows,
        Err(err) => {
            return CustomAdapterReport {
                ready: true,
                adapters: Vec::new(),
                last_match_ran: false,
                captured_at: Utc::now(),
                error: Some(stringify_err(err)),
            };
        }
    };

    // Compile only enabled adapters' regexes; disabled rows still appear in
    // the report (so the UI can show them) but never participate in matching.
    let mut compile_errors: std::collections::HashMap<Uuid, String> =
        std::collections::HashMap::new();
    let mut compiled: Vec<CompiledAdapter> = Vec::new();
    for row in &all {
        if !row.enabled {
            continue;
        }
        match CompiledAdapter::from_definition(row.clone()) {
            Ok(c) => compiled.push(c),
            Err(err) => {
                compile_errors.insert(row.id, err.to_string());
            }
        }
    }

    // Always run a fresh scan when the user opens the card; this keeps the
    // matched-PID column accurate even when the process-scanner card has not
    // been refreshed recently. `scan_once` is at most a few ms on Linux.
    let snapshot: ProcessSnapshot = scanner.scan_once_for_matching();
    let matches = match_snapshot(&compiled, &snapshot);
    let grouped = group_by_adapter(&matches);

    let adapters = all
        .into_iter()
        .map(|row| summarise(row, &grouped, &compile_errors))
        .collect();

    CustomAdapterReport {
        ready: true,
        adapters,
        last_match_ran: true,
        captured_at: Utc::now(),
        error: None,
    }
}

fn summarise(
    row: CustomAdapter,
    grouped: &std::collections::BTreeMap<Uuid, Vec<&CustomAdapterMatch>>,
    compile_errors: &std::collections::HashMap<Uuid, String>,
) -> CustomAdapterSummary {
    let matched_pids: Vec<u32> = grouped
        .get(&row.id)
        .map(|matches| {
            let mut pids: Vec<u32> = matches.iter().map(|m| m.process.pid).collect();
            pids.sort_unstable();
            pids
        })
        .unwrap_or_default();
    let regex_compile_error = compile_errors.get(&row.id).cloned();
    CustomAdapterSummary {
        id: row.id,
        label: row.label,
        agent_name: row.agent_name,
        enabled: row.enabled,
        color: row.color,
        match_kind: row.match_kind,
        pattern: row.pattern,
        cost_per_hour_cents: row.cost_per_hour_cents,
        notes: row.notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
        matched_pids,
        regex_compile_error,
    }
}

fn stringify_err(err: CustomAdapterError) -> String {
    err.to_string()
}
