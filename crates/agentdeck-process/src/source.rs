//! Live [`ProcessSource`] backed by the `sysinfo` crate.

use std::sync::Mutex;

use agentdeck_core::{ProcessInfo, ProcessSource};
use chrono::{TimeZone, Utc};
use sysinfo::{ProcessesToUpdate, System};

/// `ProcessSource` implementation that wraps a single `sysinfo::System`.
///
/// The `sysinfo::System` cache is mutable (each call to `refresh_processes`
/// mutates the table), but `ProcessSource::list` takes `&self`. The mutex
/// gives us interior mutability without forcing callers to deal with a
/// `Mutex` directly. Contention is not a real concern: only the scanner
/// task calls `list`, and any future ad-hoc caller pays at most a few ms.
pub struct SysinfoProcessSource {
    system: Mutex<System>,
}

impl SysinfoProcessSource {
    /// Construct a fresh source. The internal `sysinfo::System` is created
    /// empty; the first call to [`ProcessSource::list`] populates it.
    pub fn new() -> Self {
        Self {
            system: Mutex::new(System::new()),
        }
    }
}

impl Default for SysinfoProcessSource {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessSource for SysinfoProcessSource {
    fn list(&self) -> Vec<ProcessInfo> {
        let mut guard = match self.system.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!("sysinfo mutex poisoned; recovering");
                poisoned.into_inner()
            }
        };

        // Refresh every process and prune any that have exited since the
        // last refresh. The cost is dominated by syscalls on Linux
        // (/proc/*/...) and stays in the low single-digit milliseconds
        // even for several hundred processes.
        guard.refresh_processes(ProcessesToUpdate::All, true);

        let mut out: Vec<ProcessInfo> = guard
            .processes()
            .iter()
            .map(|(pid, process)| ProcessInfo {
                pid: pid.as_u32(),
                parent_pid: process.parent().map(|p| p.as_u32()),
                name: process.name().to_string_lossy().into_owned(),
                cmdline: process
                    .cmd()
                    .iter()
                    .map(|s| s.to_string_lossy().into_owned())
                    .collect(),
                cwd: process
                    .cwd()
                    .map(|p| p.to_string_lossy().into_owned()),
                started_at: Utc
                    .timestamp_opt(process.start_time() as i64, 0)
                    .single()
                    .unwrap_or_else(Utc::now),
            })
            .collect();

        out.sort_by_key(|p| p.pid);
        out
    }
}
