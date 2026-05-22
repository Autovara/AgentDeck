//! Built-in Level 1 adapter for the [Aider](https://aider.chat) CLI.
//!
//! Aider is a Python-based interactive TUI for AI pair-programming. The
//! alpha ships this adapter at **Level 1 (Presence)** per build-plan §7:
//! it detects the process, identifies the cwd/repo, and reports
//! `running` while the PID is alive. The session state machine sets
//! `completed` once the PID disappears.
//!
//! Why Level 1 and not Level 2:
//!
//! - Aider runs as a TUI. The build plan's §10 TUI caveat is explicit:
//!   *"An adapter without a defensible waiting-detection strategy must
//!   stay at Level 1 presence in the alpha, and its sessions must not
//!   surface 'waiting for input' attention items."*
//! - The attention engine (build-plan §15 step 7) already gates the
//!   `waiting_for_input` reason to adapter level >= `Status`, so a Level
//!   1 declaration here is enough to keep that rule silent for Aider
//!   sessions without any further changes.
//! - Status classification (idle / stalled / errored) from external
//!   signals like `.aider.chat.history.md` mtime is straightforward to
//!   add later; deferring it keeps this adapter's surface area small and
//!   the diagnostics card honest.
//!
//! ## Detection
//!
//! The matcher is **process-only** — no filesystem reads in this step.
//! A process is treated as an Aider session when any of the following
//! is true:
//!
//! 1. The OS-reported `name` is exactly `aider` (or `aider.exe` on
//!    Windows). This is the typical case after `pip install aider-chat`,
//!    which installs an entrypoint script of that name on `$PATH`.
//! 2. The OS reports a `python*` interpreter but `cmdline` contains
//!    `-m aider` (or `-m aider.<sub>`). This covers `python -m aider`,
//!    `python3 -m aider`, `python3.11 -m aider.cli`, etc.
//! 3. The OS reports a `python*` interpreter but `cmdline[1]` is a path
//!    whose basename is exactly `aider` (covers users invoking the
//!    entrypoint script directly via the interpreter, e.g.
//!    `python /home/me/.venvs/aider/bin/aider`).
//!
//! False positives are theoretically possible (a non-Aider script
//! literally named `aider`), but the false-positive surface is small
//! and the build plan calls for the simpler signal in the alpha. If a
//! user hits a false positive, they can disable this adapter and rely
//! on the custom adapter instead.

use agentdeck_adapter::{
    Adapter, AdapterDiagnostic, AdapterMatch, AdapterScanResult, CapabilityLevel, Confidence,
    SessionStatus,
};
use agentdeck_process::ProcessSnapshot;

/// Stable adapter name reported into `sessions.adapter_name` and
/// `adapter_diagnostics.adapter_name`.
pub const ADAPTER_NAME: &str = "aider";

/// Human-friendly `sessions.agent_name` for every match.
const AGENT_NAME: &str = "aider";

const STATUS_SOURCE: &str = "process-list";

/// Built-in Aider adapter (Level 1).
///
/// Stateless: every scan re-derives its result purely from the input
/// [`ProcessSnapshot`]. The struct exists so the future build steps can
/// hang configuration (enabled flag, opt-in feature toggles) off it.
#[derive(Debug, Default, Clone, Copy)]
pub struct AiderAdapter;

impl AiderAdapter {
    pub const fn new() -> Self {
        Self
    }
}

impl Adapter for AiderAdapter {
    fn name(&self) -> &str {
        ADAPTER_NAME
    }

    fn capability_level(&self) -> CapabilityLevel {
        CapabilityLevel::Presence
    }

    fn scan(&self, snapshot: &ProcessSnapshot) -> AdapterScanResult {
        let matches: Vec<AdapterMatch> = snapshot
            .processes
            .iter()
            .filter(|p| is_aider_process(&p.name, &p.cmdline))
            .map(|p| AdapterMatch {
                adapter_name: ADAPTER_NAME.to_string(),
                agent_name: AGENT_NAME.to_string(),
                capability_level: CapabilityLevel::Presence,
                pid: p.pid,
                command: if p.cmdline.is_empty() {
                    p.name.clone()
                } else {
                    p.cmdline.join(" ")
                },
                cwd: p.cwd.clone(),
                repo_path: p.cwd.clone(),
                status: SessionStatus::Running,
                status_confidence: Confidence::Medium,
                status_source: STATUS_SOURCE.to_string(),
                observed_at: snapshot.captured_at,
            })
            .collect();

        let detected_count = matches.len() as u32;

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
                failure_reasons: Vec::new(),
                confidence: Confidence::Medium,
                known_limitations: vec![
                    "presence-only".to_string(),
                    "no-status-classification".to_string(),
                    "no-waiting-detection".to_string(),
                    "no-usage-tracking".to_string(),
                    "no-control".to_string(),
                ],
            },
        }
    }
}

/// Pure matcher: returns `true` when the `(name, cmdline)` pair is an
/// Aider session per the detection rules in the module docs.
///
/// Public so the diagnostics tests and any future adapter introspection
/// helper can reuse it; the trait impl wraps this with the
/// snapshot-driven plumbing.
pub fn is_aider_process(name: &str, cmdline: &[String]) -> bool {
    // (1) Direct binary launch by name.
    if matches_aider_bin(name) {
        return true;
    }
    // Some OSes (notably macOS) report the OS-level name as the script
    // interpreter but cmdline[0] still reflects the entrypoint script.
    if let Some(first) = cmdline.first() {
        if matches_aider_bin(basename(first)) {
            return true;
        }
    }
    // (2)/(3) Python interpreter cases.
    let is_python = is_python_bin(name)
        || cmdline
            .first()
            .map(|a| is_python_bin(basename(a)))
            .unwrap_or(false);
    if !is_python {
        return false;
    }
    let mut iter = cmdline.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            // `python -m aider [...]` or `python -m aider.cli [...]`
            "-m" => {
                if let Some(next) = iter.next() {
                    if next == "aider" || next.starts_with("aider.") {
                        return true;
                    }
                }
            }
            // `python /path/to/aider [...]` — the second positional arg
            // is the entrypoint script. We skip cmdline[0] (the python
            // interpreter) implicitly because the iterator has already
            // consumed it.
            other => {
                if matches_aider_bin(basename(other)) {
                    return true;
                }
            }
        }
    }
    false
}

fn matches_aider_bin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stripped = lower.strip_suffix(".exe").unwrap_or(lower.as_str());
    stripped == "aider"
}

fn is_python_bin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stripped = lower.strip_suffix(".exe").unwrap_or(lower.as_str());
    matches!(stripped, "python" | "python3") || stripped.starts_with("python3.")
}

fn basename(path: &str) -> &str {
    if let Some((_, last)) = path.rsplit_once('/') {
        return last;
    }
    if let Some((_, last)) = path.rsplit_once('\\') {
        return last;
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentdeck_core::ProcessInfo;
    use agentdeck_process::SnapshotSource;
    use chrono::{DateTime, TimeZone, Utc};

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 13, 0, 0).unwrap()
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

    // --- matcher unit tests ---

    #[test]
    fn matches_bare_aider_binary_name() {
        assert!(is_aider_process("aider", &["aider".into()]));
    }

    #[test]
    fn matches_aider_with_args() {
        assert!(is_aider_process(
            "aider",
            &["aider".into(), "--model".into(), "gpt-4o".into()]
        ));
    }

    #[test]
    fn matches_aider_exe_on_windows() {
        assert!(is_aider_process("aider.exe", &["aider.exe".into()]));
        // Case-insensitive Windows binary name
        assert!(is_aider_process("AIDER.EXE", &["AIDER.EXE".into()]));
    }

    #[test]
    fn matches_aider_via_absolute_path_cmdline() {
        // OS reports a generic name but cmdline[0] is the entrypoint
        // script with an absolute path.
        assert!(is_aider_process(
            "aider",
            &["/home/me/.local/bin/aider".into(), "--no-stream".into()]
        ));
    }

    #[test]
    fn matches_python_dash_m_aider() {
        assert!(is_aider_process(
            "python3",
            &["python3".into(), "-m".into(), "aider".into()]
        ));
    }

    #[test]
    fn matches_python_dash_m_aider_subpackage() {
        assert!(is_aider_process(
            "python",
            &["python".into(), "-m".into(), "aider.cli".into()]
        ));
    }

    #[test]
    fn matches_python311_dash_m_aider() {
        assert!(is_aider_process(
            "python3.11",
            &[
                "python3.11".into(),
                "-m".into(),
                "aider".into(),
                "--yes".into()
            ]
        ));
    }

    #[test]
    fn matches_python_entrypoint_script_path() {
        // `python /home/me/.venvs/aider/bin/aider ...`
        assert!(is_aider_process(
            "python3",
            &[
                "python3".into(),
                "/home/me/.venvs/aider/bin/aider".into(),
                "--yes".into()
            ]
        ));
    }

    #[test]
    fn matches_windows_python_backslash_path() {
        assert!(is_aider_process(
            "python.exe",
            &["python.exe".into(), "C:\\Python311\\Scripts\\aider".into()]
        ));
    }

    #[test]
    fn does_not_match_unrelated_python_process() {
        assert!(!is_aider_process(
            "python3",
            &["python3".into(), "manage.py".into(), "runserver".into()]
        ));
    }

    #[test]
    fn does_not_match_unrelated_binary() {
        assert!(!is_aider_process("bash", &["bash".into()]));
        assert!(!is_aider_process(
            "node",
            &["node".into(), "server.js".into()]
        ));
    }

    #[test]
    fn does_not_match_pythonista_lookalike() {
        // Guard against a `starts_with("python")` regression. Only the
        // canonical interpreter names should pass the Python check.
        assert!(!is_aider_process(
            "pythonista",
            &["pythonista".into(), "-m".into(), "aider".into()]
        ));
    }

    #[test]
    fn does_not_match_aiderbot_lookalike() {
        // Guard against a `starts_with("aider")` regression on the
        // binary check.
        assert!(!is_aider_process("aiderbot", &["aiderbot".into()]));
    }

    #[test]
    fn does_not_match_python_dash_m_unrelated() {
        assert!(!is_aider_process(
            "python3",
            &["python3".into(), "-m".into(), "venv".into()]
        ));
    }

    #[test]
    fn does_not_match_dash_m_without_following_arg() {
        // Pathological cmdline. Should not panic and should not match.
        assert!(!is_aider_process(
            "python3",
            &["python3".into(), "-m".into()]
        ));
    }

    // --- Adapter impl tests ---

    #[test]
    fn name_and_level_are_stable() {
        let a = AiderAdapter::new();
        assert_eq!(a.name(), "aider");
        assert_eq!(a.capability_level(), CapabilityLevel::Presence);
        assert!(a.enabled());
    }

    #[test]
    fn empty_snapshot_yields_zero_detected_and_a_diagnostic() {
        let a = AiderAdapter::new();
        let result = a.scan(&snapshot(Vec::new()));
        assert!(result.matches.is_empty());
        assert_eq!(result.diagnostic.adapter_name, "aider");
        assert!(result.diagnostic.enabled);
        assert_eq!(
            result.diagnostic.capability_level,
            CapabilityLevel::Presence
        );
        assert_eq!(result.diagnostic.detected_count, 0);
        assert!(result.diagnostic.failure_reasons.is_empty());
        assert!(result.diagnostic.missing_permissions.is_empty());
        assert!(result
            .diagnostic
            .known_limitations
            .iter()
            .any(|s| s == "presence-only"));
    }

    #[test]
    fn one_aider_process_yields_one_match() {
        let a = AiderAdapter::new();
        let snap = snapshot(vec![
            proc(101, "aider", &["aider", "--yes"], Some("/repo/api")),
            proc(102, "bash", &["bash"], None),
        ]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.diagnostic.detected_count, 1);

        let m = &result.matches[0];
        assert_eq!(m.adapter_name, "aider");
        assert_eq!(m.agent_name, "aider");
        assert_eq!(m.capability_level, CapabilityLevel::Presence);
        assert_eq!(m.pid, 101);
        assert_eq!(m.command, "aider --yes");
        assert_eq!(m.cwd.as_deref(), Some("/repo/api"));
        assert_eq!(m.repo_path.as_deref(), Some("/repo/api"));
        assert_eq!(m.status, SessionStatus::Running);
        assert_eq!(m.status_confidence, Confidence::Medium);
        assert_eq!(m.status_source, STATUS_SOURCE);
        assert_eq!(m.observed_at, snap.captured_at);
    }

    #[test]
    fn multiple_aider_processes_each_emit_one_match() {
        let a = AiderAdapter::new();
        let snap = snapshot(vec![
            proc(201, "aider", &["aider"], Some("/work/api")),
            proc(
                202,
                "python3",
                &["python3", "-m", "aider"],
                Some("/work/web"),
            ),
            proc(203, "node", &["node", "server.js"], Some("/work/api")),
        ]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.diagnostic.detected_count, 2);

        let pids: Vec<u32> = result.matches.iter().map(|m| m.pid).collect();
        assert!(pids.contains(&201));
        assert!(pids.contains(&202));
        for m in &result.matches {
            assert_eq!(m.adapter_name, "aider");
            assert_eq!(m.capability_level, CapabilityLevel::Presence);
            assert_eq!(m.status, SessionStatus::Running);
        }
    }

    #[test]
    fn match_command_falls_back_to_name_when_cmdline_empty() {
        let a = AiderAdapter::new();
        // cmdline empty but name == "aider" — implausible in practice
        // but the matcher must still produce a sane `command` field.
        let snap = snapshot(vec![proc(300, "aider", &[], None)]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].command, "aider");
        assert!(result.matches[0].cwd.is_none());
        assert!(result.matches[0].repo_path.is_none());
    }

    #[test]
    fn diagnostic_last_scan_time_matches_snapshot() {
        let a = AiderAdapter::new();
        let snap = snapshot(vec![proc(1, "aider", &["aider"], None)]);
        let result = a.scan(&snap);
        assert_eq!(result.diagnostic.last_scan_time, snap.captured_at);
    }
}
