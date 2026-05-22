//! Built-in Level 1 adapter for [Anthropic's Claude Code](https://github.com/anthropics/claude-code).
//!
//! Claude Code is an interactive TUI distributed as an npm package
//! (`@anthropic-ai/claude-code`) plus a thin entrypoint script on
//! `$PATH`. Like the Aider and Codex CLI adapters (build-plan §15 steps
//! 10–11), this crate ships at **Level 1 (Presence)** per the §10 TUI
//! caveat: we detect the process, identify the cwd/repo, and report
//! `running` while the PID is alive; the session state machine sets
//! `completed` when the PID disappears.
//!
//! Why Level 1 and not Level 2:
//!
//! - Claude Code is a TUI. Distinguishing "thinking" from "waiting for
//!   user approval" or "stalled" reliably from outside the process is
//!   not yet possible.
//! - The attention engine already gates `waiting_for_input` to adapter
//!   level >= `Status`, so the Level 1 declaration is enough to keep
//!   that rule silent for Claude Code sessions.
//! - Higher-tier classification (config dir under `~/.claude/`, session
//!   files, history mtime, etc.) is documented as
//!   `planning/adapter-feasibility.md` §4.1 *TBD (empirical)* work and
//!   can land later without changing the public surface.
//!
//! ## Detection
//!
//! The matcher is **process-only** in the alpha — no filesystem reads.
//! A process is treated as a Claude Code session when any of the
//! following is true:
//!
//! 1. The OS-reported `name` is exactly `claude` or `claude-code`
//!    (or the `.exe` variants on Windows). The current distribution
//!    installs `claude` on `$PATH`; `claude-code` is kept as a fallback
//!    for older builds per `planning/adapter-feasibility.md` §4.1.
//! 2. The OS reports a different binary but `cmdline[0]`'s basename
//!    matches (covers absolute-path invocations like
//!    `/home/me/.npm-global/bin/claude`).
//! 3. The OS reports a `node*` interpreter but `cmdline[1..]` contains
//!    a positional argument whose basename matches (covers
//!    `node /usr/local/lib/claude` direct invocations that bypass the
//!    shim).
//!
//! False positives are theoretically possible — `claude` is a more
//! generic name than `aider` / `codex` — but the matcher requires an
//! exact basename match (not `starts_with("claude")`), so `claudette`,
//! `claude-bot`, `pyclaude`, etc. do not trigger. If a user does hit a
//! false positive they can disable this adapter from the dashboard and
//! rely on the custom adapter instead.

use agentdeck_adapter::{
    Adapter, AdapterDiagnostic, AdapterMatch, AdapterScanResult, CapabilityLevel, Confidence,
    SessionStatus,
};
use agentdeck_process::ProcessSnapshot;

/// Stable adapter name reported into `sessions.adapter_name` and
/// `adapter_diagnostics.adapter_name`.
pub const ADAPTER_NAME: &str = "claude-code";

/// Human-friendly `sessions.agent_name` for every match.
const AGENT_NAME: &str = "claude-code";

const STATUS_SOURCE: &str = "process-list";

/// Built-in Claude Code adapter (Level 1).
///
/// Stateless: every scan re-derives its result purely from the input
/// [`ProcessSnapshot`].
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeCodeAdapter;

impl ClaudeCodeAdapter {
    pub const fn new() -> Self {
        Self
    }
}

impl Adapter for ClaudeCodeAdapter {
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
            .filter(|p| is_claude_code_process(&p.name, &p.cmdline))
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
/// Claude Code session per the detection rules in the module docs.
///
/// Public so the diagnostics tests and any future adapter introspection
/// helper can reuse it.
pub fn is_claude_code_process(name: &str, cmdline: &[String]) -> bool {
    // (1) Direct binary launch by OS-reported name.
    if matches_claude_bin(name) {
        return true;
    }
    // (2) Direct binary launch but the OS-reported name is the shell /
    // interpreter / loader while cmdline[0] is the entrypoint.
    if let Some(first) = cmdline.first() {
        if matches_claude_bin(basename(first)) {
            return true;
        }
    }
    // (3) Node interpreter running a `claude` / `claude-code` script
    // directly. Skip cmdline[0] (the interpreter itself) and look for a
    // positional argument whose basename matches.
    let is_node = is_node_bin(name)
        || cmdline
            .first()
            .map(|a| is_node_bin(basename(a)))
            .unwrap_or(false);
    if !is_node {
        return false;
    }
    for arg in cmdline.iter().skip(1) {
        // Skip `node` flags that take their own value (--inspect=...,
        // --max-old-space-size, etc.). Arguments starting with `-`
        // cannot be the entrypoint.
        if arg.starts_with('-') {
            continue;
        }
        if matches_claude_bin(basename(arg)) {
            return true;
        }
    }
    false
}

fn matches_claude_bin(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let stripped = lower.strip_suffix(".exe").unwrap_or(lower.as_str());
    stripped == "claude" || stripped == "claude-code"
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
    fn matches_bare_claude_binary_name() {
        assert!(is_claude_code_process("claude", &["claude".into()]));
    }

    #[test]
    fn matches_legacy_claude_code_binary_name() {
        assert!(is_claude_code_process(
            "claude-code",
            &["claude-code".into()]
        ));
    }

    #[test]
    fn matches_claude_with_args() {
        assert!(is_claude_code_process(
            "claude",
            &["claude".into(), "--print".into(), "hello".into()]
        ));
    }

    #[test]
    fn matches_claude_exe_on_windows() {
        assert!(is_claude_code_process("claude.exe", &["claude.exe".into()]));
        // Case-insensitive Windows binary name
        assert!(is_claude_code_process("CLAUDE.EXE", &["CLAUDE.EXE".into()]));
    }

    #[test]
    fn matches_claude_code_exe_on_windows() {
        assert!(is_claude_code_process(
            "claude-code.exe",
            &["claude-code.exe".into()]
        ));
    }

    #[test]
    fn matches_claude_via_absolute_path_cmdline() {
        // OS reports a different name but cmdline[0] is an absolute
        // path to the claude entrypoint script.
        assert!(is_claude_code_process(
            "sh",
            &["/home/me/.npm-global/bin/claude".into(), "--yes".into()]
        ));
    }

    #[test]
    fn matches_node_running_claude_script() {
        assert!(is_claude_code_process(
            "node",
            &["node".into(), "/usr/local/lib/claude".into()]
        ));
    }

    #[test]
    fn matches_node_running_claude_code_script() {
        assert!(is_claude_code_process(
            "node",
            &["node".into(), "/usr/local/lib/claude-code".into()]
        ));
    }

    #[test]
    fn matches_node_with_flag_before_claude_path() {
        // `node --enable-source-maps /opt/claude/bin/claude ...`
        assert!(is_claude_code_process(
            "node",
            &[
                "node".into(),
                "--enable-source-maps".into(),
                "/opt/claude/bin/claude".into(),
                "--ask".into()
            ]
        ));
    }

    #[test]
    fn matches_nodejs_alias() {
        // Debian/Ubuntu ship the binary as `nodejs`.
        assert!(is_claude_code_process(
            "nodejs",
            &["nodejs".into(), "/usr/lib/claude".into()]
        ));
    }

    #[test]
    fn matches_windows_node_backslash_path() {
        assert!(is_claude_code_process(
            "node.exe",
            &[
                "node.exe".into(),
                "C:\\Program Files\\claude-code\\claude".into()
            ]
        ));
    }

    #[test]
    fn does_not_match_unrelated_node_process() {
        assert!(!is_claude_code_process(
            "node",
            &["node".into(), "server.js".into()]
        ));
    }

    #[test]
    fn does_not_match_unrelated_binary() {
        assert!(!is_claude_code_process("bash", &["bash".into()]));
        assert!(!is_claude_code_process(
            "python3",
            &["python3".into(), "manage.py".into()]
        ));
    }

    #[test]
    fn does_not_match_claude_prefix_lookalikes() {
        // Guard against a `starts_with("claude")` regression on the
        // binary check.
        assert!(!is_claude_code_process("claudette", &["claudette".into()]));
        assert!(!is_claude_code_process("claudine", &["claudine".into()]));
        assert!(!is_claude_code_process(
            "claude-bot",
            &["claude-bot".into()]
        ));
        assert!(!is_claude_code_process(
            "node",
            &["node".into(), "/opt/claude-bot".into()]
        ));
    }

    #[test]
    fn does_not_match_claude_suffix_lookalikes() {
        // `pyclaude` ends with the word but the binary is not claude.
        assert!(!is_claude_code_process("pyclaude", &["pyclaude".into()]));
    }

    #[test]
    fn does_not_match_node_lookalike() {
        assert!(!is_claude_code_process(
            "nodemon",
            &["nodemon".into(), "/opt/claude".into()]
        ));
    }

    #[test]
    fn does_not_match_node_with_no_script_args() {
        assert!(!is_claude_code_process("node", &["node".into()]));
    }

    #[test]
    fn does_not_match_node_with_only_flag_args() {
        // Pathological: every arg starts with `-`. Should not panic
        // and should not match.
        assert!(!is_claude_code_process(
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
        let a = ClaudeCodeAdapter::new();
        assert_eq!(a.name(), "claude-code");
        assert_eq!(a.capability_level(), CapabilityLevel::Presence);
        assert!(a.enabled());
    }

    #[test]
    fn empty_snapshot_yields_zero_detected_and_a_diagnostic() {
        let a = ClaudeCodeAdapter::new();
        let result = a.scan(&snapshot(Vec::new()));
        assert!(result.matches.is_empty());
        assert_eq!(result.diagnostic.adapter_name, "claude-code");
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
    fn one_claude_process_yields_one_match() {
        let a = ClaudeCodeAdapter::new();
        let snap = snapshot(vec![
            proc(701, "claude", &["claude", "--yes"], Some("/repo/api")),
            proc(702, "bash", &["bash"], None),
        ]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.diagnostic.detected_count, 1);

        let m = &result.matches[0];
        assert_eq!(m.adapter_name, "claude-code");
        assert_eq!(m.agent_name, "claude-code");
        assert_eq!(m.capability_level, CapabilityLevel::Presence);
        assert_eq!(m.pid, 701);
        assert_eq!(m.command, "claude --yes");
        assert_eq!(m.cwd.as_deref(), Some("/repo/api"));
        assert_eq!(m.repo_path.as_deref(), Some("/repo/api"));
        assert_eq!(m.status, SessionStatus::Running);
        assert_eq!(m.status_confidence, Confidence::Medium);
        assert_eq!(m.status_source, STATUS_SOURCE);
        assert_eq!(m.observed_at, snap.captured_at);
    }

    #[test]
    fn multiple_claude_processes_each_emit_one_match() {
        let a = ClaudeCodeAdapter::new();
        let snap = snapshot(vec![
            proc(801, "claude", &["claude"], Some("/work/api")),
            proc(
                802,
                "node",
                &["node", "/usr/local/lib/claude"],
                Some("/work/web"),
            ),
            proc(
                803,
                "claude-code",
                &["claude-code", "--no-stream"],
                Some("/work/cli"),
            ),
            proc(804, "node", &["node", "server.js"], Some("/work/api")),
        ]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 3);
        assert_eq!(result.diagnostic.detected_count, 3);

        let pids: Vec<u32> = result.matches.iter().map(|m| m.pid).collect();
        assert!(pids.contains(&801));
        assert!(pids.contains(&802));
        assert!(pids.contains(&803));
        for m in &result.matches {
            assert_eq!(m.adapter_name, "claude-code");
            assert_eq!(m.capability_level, CapabilityLevel::Presence);
            assert_eq!(m.status, SessionStatus::Running);
        }
    }

    #[test]
    fn match_command_falls_back_to_name_when_cmdline_empty() {
        let a = ClaudeCodeAdapter::new();
        let snap = snapshot(vec![proc(900, "claude", &[], None)]);
        let result = a.scan(&snap);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].command, "claude");
        assert!(result.matches[0].cwd.is_none());
        assert!(result.matches[0].repo_path.is_none());
    }

    #[test]
    fn diagnostic_last_scan_time_matches_snapshot() {
        let a = ClaudeCodeAdapter::new();
        let snap = snapshot(vec![proc(1, "claude", &["claude"], None)]);
        let result = a.scan(&snap);
        assert_eq!(result.diagnostic.last_scan_time, snap.captured_at);
    }
}
