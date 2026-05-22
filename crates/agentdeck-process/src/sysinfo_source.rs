//! Production [`ProcessSource`] backed by the `sysinfo` crate.

use std::sync::Mutex;

use agentdeck_core::{ProcessInfo, ProcessSource};
use chrono::{DateTime, TimeZone, Utc};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, RefreshKind, System, UpdateKind};

/// Real-OS process source. Uses `sysinfo` under the hood and refreshes the
/// process table on every [`Self::list`] call.
///
/// Allocation pattern:
///
/// - A single [`System`] is built lazily on first call and reused. `sysinfo`
///   keeps internal state between refreshes (this is required for accurate
///   CPU usage when we eventually add it). Wrapping it in a `Mutex` is safe
///   because the trait is `&self` and we never hold the lock across awaits.
/// - We deliberately fetch `cmd`, `cwd`, and `exe` on every refresh. They are
///   the data adapters need to match agents; skipping them would mean a
///   second pass per process.
pub struct SysinfoProcessSource {
    system: Mutex<System>,
    refresh_kind: ProcessRefreshKind,
}

impl Default for SysinfoProcessSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SysinfoProcessSource {
    pub fn new() -> Self {
        let refresh_kind = ProcessRefreshKind::nothing()
            .with_cmd(UpdateKind::Always)
            .with_cwd(UpdateKind::Always)
            .with_exe(UpdateKind::Always);

        let mut system =
            System::new_with_specifics(RefreshKind::nothing().with_processes(refresh_kind));
        // Warm the table so the first call to `list` sees real data instead
        // of an empty map on platforms that need two refreshes.
        let _ = system.refresh_processes_specifics(ProcessesToUpdate::All, true, refresh_kind);

        Self {
            system: Mutex::new(system),
            refresh_kind,
        }
    }
}

impl ProcessSource for SysinfoProcessSource {
    fn list(&self) -> Vec<ProcessInfo> {
        let mut guard = match self.system.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                tracing::warn!("sysinfo mutex poisoned; reusing inner state");
                poisoned.into_inner()
            }
        };
        let _ = guard.refresh_processes_specifics(ProcessesToUpdate::All, true, self.refresh_kind);

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
                cwd: process.cwd().map(|c| c.to_string_lossy().into_owned()),
                started_at: epoch_to_utc(process.start_time()),
            })
            .collect();
        out.sort_by_key(|p| p.pid);
        out
    }
}

fn epoch_to_utc(seconds: u64) -> DateTime<Utc> {
    // sysinfo's start_time is documented as "seconds since the UNIX epoch".
    // Values larger than i64::MAX are physically impossible (~3 × 10^11
    // years), but clamp to be safe rather than panic in `from_timestamp`.
    let secs = if seconds > i64::MAX as u64 {
        i64::MAX
    } else {
        seconds as i64
    };
    Utc.timestamp_opt(secs, 0).single().unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_to_utc_round_trip() {
        let t = epoch_to_utc(0);
        assert_eq!(t.timestamp(), 0);
    }

    #[test]
    fn epoch_to_utc_clamps_overflow_inputs() {
        let t = epoch_to_utc(u64::MAX);
        // Just check it does not panic and yields a sane DateTime.
        assert!(t.timestamp() > 0);
    }
}
