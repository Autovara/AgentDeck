//! Built-in Level 1 adapter for the [OpenAI Codex CLI](https://github.com/openai/codex).
//!
//! The Codex CLI is an interactive TUI distributed both as a native
//! binary and as an npm package that runs under Node.js. Like the Aider
//! adapter (build-plan §15 step 10), this crate ships at **Level 1
//! (Presence)** per the §10 TUI caveat: we detect the process, identify
//! the cwd/repo, and report `running` while the PID is alive; the
//! session state machine sets `completed` when the PID disappears.
//!
//! Why Level 1 and not Level 2:
//!
//! - Codex CLI is a TUI. Distinguishing "thinking" from "waiting for
//!   user approval" or "stalled" reliably from outside the process is
//!   not yet possible.
//! - The attention engine already gates `waiting_for_input` to adapter
//!   level >= `Status`, so the Level 1 declaration is enough to keep
//!   that rule silent for Codex sessions.
//! - Higher-tier classification can land later without changing the
//!   public surface: every consumer only sees the `Adapter` trait.
//!
//! ## Detection
//!
//! The matcher is **process-only** in the alpha — no filesystem reads.
//! A process is treated as a Codex CLI session when any of the
//! following is true:
//!
//! 1. The OS-reported `name` is exactly `codex` (or `codex.exe` on
//!    Windows). This is the typical case for both the native binary
//!    distribution and the npm-installed `codex` shim on `$PATH`.
//! 2. The OS reports a different binary but `cmdline[0]`'s basename
//!    is `codex` (covers absolute-path invocations like
//!    `/home/me/.local/bin/codex`).
//! 3. The OS reports a `node*` interpreter but `cmdline[1..]` contains
//!    a path whose basename is `codex` (covers `node /path/to/codex`
//!    direct invocations that bypass the shim).
//!
//! False positives are theoretically possible but small: there is no
//! widely-installed namesake binary called `codex`, and the matcher
//! requires an exact basename match (not `starts_with("codex")`), so
//! `codex-cli`, `codexbot`, etc. do not trigger. If a user does hit a
//! false positive they can disable this adapter from the dashboard and
//! rely on the custom adapter instead.

use agentdeck_adapter::{
    Adapter, AdapterDiagnostic, AdapterMatch, AdapterScanResult, CapabilityLevel, Confidence,
    SessionStatus,
};
use agentdeck_process::ProcessSnapshot;

/// Stable adapter name reported into `sessions.adapter_name` and
/// `adapter_diagnostics.adapter_name`.
pub const ADAPTER_NAME: &str = "codex";

/// Human-friendly `sessions.agent_name` for every match.
const AGENT_NAME: &str = "codex";

const STATUS_SOURCE: &str = "process-list";

/// Built-in Codex CLI adapter (Level 1).
///
/// Stateless: every scan re-derives its result purely from the input
/// [`ProcessSnapshot`].
#[derive(Debug, Default, Clone, Copy)]
pub struct CodexAdapter;

impl CodexAdapter {
    pub const fn new() -> Self {
        Self
    }
}

impl Adapter for CodexAdapter {
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
            .filter(|p| is_codex_process(&p.name, &p.cmdline))
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
                cost_per_hour_cents: None,
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

/// Pure matcher: returns `true` when the `(name, cmdline)` pair is a
/// Codex CLI session per the detection rules in the module docs.
///
/// Public so the diagnostics tests and any future adapter introspection
/// helper can reuse it.
pub fn is_codex_process(name: &str, cmdline: &[String]) -> bool {
    // (1) Direct binary launch by OS-reported name.
    if matches_codex_bin(name) {
        return true;
    }
    // (2) Direct binary launch but the OS-reported name is the shell
    // / interpreter / loader while cmdline[0] is the entrypoint.
    if let Some(first) = cmdline.first() {
        if matches_codex_bin(basename(first)) {
            return true;
        }
    }
    // (3) Node interpreter running a `codex` script directly. Skip
    // cmdline[0] (the interpreter itself) and look for a positional
    // argument whose basename is `codex`.
    let is_node = is_node_bin(name)
        || cmdline
            .first()
            .map(|a| is_node_bin(basename(a)))
            .unwrap_or(false);
    if !is_node {
        return false;
    }
    // First arg is the interpreter; subsequent positional args are
    // the script + script args. Node has no `-m` equivalent, so we
    // simply scan for a `codex` basename.
    for arg in cmdline.iter().skip(1) {
        // Skip `node` flags that take their own value (--inspect=...,
        // --max-old-space-size, etc.). The cheap heuristic: arguments
        // starting with `-` cannot be the entrypoint.
        if arg.starts_with('-') {
            continue;
        }
        if matches_codex_bin(basename(arg)) {
            return true;
        }
    }
    false
}

fn matches_codex_bin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stripped = lower.strip_suffix(".exe").unwrap_or(lower.as_str());
    stripped == "codex"
}

fn is_node_bin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stripped = lower.strip_suffix(".exe").unwrap_or(lower.as_str());
    // Canonical Node binary names across platforms / package managers.
    matches!(stripped, "node" | "nodejs")
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
    fn matches_bare_codex_binary_name() {
        assert!(is_codex_process("codex", &["codex".into()]));
    }

    #[test]
    fn matches_codex_with_args() {
        assert!(is_codex_process(
            "codex",
            &["codex".into(), "--model".into(), "gpt-5".into()]
        ));
    }

    #[test]
    fn matches_codex_exe_on_windows() {
        assert!(is_codex_process("codex.exe", &["codex.exe".into()]));
        // Case-insensitive Windows binary name
        assert!(is_codex_process("CODEX.EXE", &["CODEX.EXE".into()]));
    }

    #[test]
    fn matches_codex_via_absolute_path_cmdline() {
        // OS reports a different name but cmdline[0] is an absolute
        // path to the codex entrypoint script.
        assert!(is_codex_process(
            "sh",
            &["/home/me/.local/bin/codex".into(), "--yes".into()]
        ));
    }

    #[test]
    fn matches_node_running_codex_script() {
        assert!(is_codex_process(
            "node",
            &["node".into(), "/usr/local/lib/codex".into()]
        ));
    }

    #[test]
    fn matches_node_with_flag_before_codex_path() {
        // `node --enable-source-maps /opt/codex/bin/codex ...`
        assert!(is_codex_process(
            "node",
            &[
                "node".into(),
                "--enable-source-maps".into(),
                "/opt/codex/bin/codex".into(),
                "--ask".into()
            ]
        ));
    }

    #[test]
    fn matches_nodejs_alias() {
        // Debian/Ubuntu ship the binary as `nodejs`.
        assert!(is_codex_process(
            "nodejs",
            &["nodejs".into(), "/usr/lib/codex".into()]
        ));
    }

    #[test]
    fn matches_windows_node_backslash_path() {
        assert!(is_codex_process(
            "node.exe",
            &["node.exe".into(), "C:\\Program Files\\codex\\codex".into()]
        ));
    }

    #[test]
    fn does_not_match_unrelated_node_process() {
        assert!(!is_codex_process(
            "node",
            &["node".into(), "server.js".into()]
        ));
    }

    #[test]
    fn does_not_match_unrelated_binary() {
        assert!(!is_codex_process("bash", &["bash".into()]));
        assert!(!is_codex_process(
            "python3",
            &["python3".into(), "manage.py".into()]
        ));
    }

    #[test]
    fn does_not_match_codex_prefix_lookalikes() {
        // Guard against a `starts_with("codex")` regression on the
        // binary check.
        assert!(!is_codex_process("codex-cli", &["codex-cli".into()]));
        assert!(!is_codex_process("codexbot", &["codexbot".into()]));
        assert!(!is_codex_process(
            "node",
            &["node".into(), "/opt/codex-cli".into()]
        ));
    }

    #[test]
    fn does_not_match_node_lookalike() {
        assert!(!is_codex_process(
            "nodemon",
            &["nodemon".into(), "/opt/codex".into()]
        ));
    }

    #[test]
    fn does_not_match_node_with_no_script_args() {
        assert!(!is_codex_process("node", &["node".into()]));
    }

    #[test]
    fn does_not_match_node_with_only_flag_args() {
        // Pathological: every arg starts with `-`. Should not panic
        // and should not match (the matcher's `starts_with('-')` skip
        // would otherwise loop forever if buggy).
        assert!(!is_codex_process(
            "node",
            &[
                "node".into(),
                "--inspect=9229".into(),
                "--max-old-space-size=4096".into()
            ]
        ));
    }

    // --- Adapter impl tests ---

    #[test]
    fn name_and_level_are_stable() {
        let a = CodexAdapter::new();
        assert_eq!(a.name(), "codex");
        assert_eq!(a.capability_level(), CapabilityLevel::Presence);
        assert!(a.enabled());
    }

    #[test]
    fn empty_snapshot_yields_zero_detected_and_a_diagnostic() {
        let a = CodexAdapter::new();
        let result = a.scan(&snapshot(Vec::new()));
        assert!(result.matches.is_empty());
        assert_eq!(result.diagnostic.adapter_name, "codex");
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
    fn one_codex_process_yields_one_match() {
        let a = CodexAdapter::new();
        let snap = snapshot(vec![
            proc(401, "codex", &["codex", "--yes"], Some("/repo/api")),
            proc(402, "bash", &["bash"], None),
        ]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.diagnostic.detected_count, 1);

        let m = &result.matches[0];
        assert_eq!(m.adapter_name, "codex");
        assert_eq!(m.agent_name, "codex");
        assert_eq!(m.capability_level, CapabilityLevel::Presence);
        assert_eq!(m.pid, 401);
        assert_eq!(m.command, "codex --yes");
        assert_eq!(m.cwd.as_deref(), Some("/repo/api"));
        assert_eq!(m.repo_path.as_deref(), Some("/repo/api"));
        assert_eq!(m.status, SessionStatus::Running);
        assert_eq!(m.status_confidence, Confidence::Medium);
        assert_eq!(m.status_source, STATUS_SOURCE);
        assert_eq!(m.observed_at, snap.captured_at);
    }

    #[test]
    fn multiple_codex_processes_each_emit_one_match() {
        let a = CodexAdapter::new();
        let snap = snapshot(vec![
            proc(501, "codex", &["codex"], Some("/work/api")),
            proc(
                502,
                "node",
                &["node", "/usr/local/lib/codex"],
                Some("/work/web"),
            ),
            proc(503, "node", &["node", "server.js"], Some("/work/api")),
        ]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 2);
        assert_eq!(result.diagnostic.detected_count, 2);

        let pids: Vec<u32> = result.matches.iter().map(|m| m.pid).collect();
        assert!(pids.contains(&501));
        assert!(pids.contains(&502));
        for m in &result.matches {
            assert_eq!(m.adapter_name, "codex");
            assert_eq!(m.capability_level, CapabilityLevel::Presence);
            assert_eq!(m.status, SessionStatus::Running);
        }
    }

    #[test]
    fn match_command_falls_back_to_name_when_cmdline_empty() {
        let a = CodexAdapter::new();
        let snap = snapshot(vec![proc(600, "codex", &[], None)]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].command, "codex");
        assert!(result.matches[0].cwd.is_none());
        assert!(result.matches[0].repo_path.is_none());
    }

    #[test]
    fn diagnostic_last_scan_time_matches_snapshot() {
        let a = CodexAdapter::new();
        let snap = snapshot(vec![proc(1, "codex", &["codex"], None)]);
        let result = a.scan(&snap);
        assert_eq!(result.diagnostic.last_scan_time, snap.captured_at);
    }
}
