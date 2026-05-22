//! Lightweight "agent candidate" heuristics for the diagnostics UI.
//!
//! **Important caveat.** This module is *not* the adapter framework. Adapters
//! land in a later build-plan step and use richer signals (`exe`, parent
//! process, file watchers, version probes, ...). The heuristics here exist
//! only so the dashboard's process-scanner card can show *something* that
//! looks like an agent before adapters are wired up. The set of patterns is
//! intentionally narrow and case-insensitive substring-only.

use agentdeck_core::ProcessInfo;
use serde::Serialize;

use crate::snapshot::ProcessSnapshot;

/// Substring patterns matched against the process name and the joined
/// command-line. Matches are case-insensitive.
pub const ALPHA_AGENT_PATTERNS: &[&str] = &["aider", "codex", "claude", "ollama", "agent"];

/// One processed candidate plus the patterns it matched.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCandidate {
    pub process: ProcessInfo,
    pub matched_patterns: Vec<&'static str>,
}

/// Scan a [`ProcessSnapshot`] and return every process that matches at least
/// one of the supplied patterns. Pattern matches are case-insensitive
/// substring-only; the join uses a single space.
pub fn extract_candidates(
    snapshot: &ProcessSnapshot,
    patterns: &[&'static str],
) -> Vec<AgentCandidate> {
    let mut out = Vec::new();
    for proc in &snapshot.processes {
        let haystack = build_haystack(proc);
        let matched: Vec<&'static str> = patterns
            .iter()
            .copied()
            .filter(|p| haystack.contains(&p.to_lowercase()))
            .collect();
        if !matched.is_empty() {
            out.push(AgentCandidate {
                process: proc.clone(),
                matched_patterns: matched,
            });
        }
    }
    out.sort_by_key(|c| c.process.pid);
    out
}

fn build_haystack(proc: &ProcessInfo) -> String {
    let mut hay = String::with_capacity(proc.name.len() + 32);
    hay.push_str(&proc.name.to_lowercase());
    for arg in &proc.cmdline {
        hay.push(' ');
        hay.push_str(&arg.to_lowercase());
    }
    hay
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::SnapshotSource;
    use chrono::{TimeZone, Utc};

    fn sample(pid: u32, name: &str, cmd: &[&str]) -> ProcessInfo {
        ProcessInfo {
            pid,
            parent_pid: None,
            name: name.into(),
            cmdline: cmd.iter().map(|s| s.to_string()).collect(),
            cwd: None,
            started_at: Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
        }
    }

    fn snapshot(processes: Vec<ProcessInfo>) -> ProcessSnapshot {
        ProcessSnapshot {
            captured_at: Utc.with_ymd_and_hms(2026, 5, 22, 10, 0, 0).unwrap(),
            source: SnapshotSource::Mock,
            scan_duration_ms: 0,
            processes,
        }
    }

    #[test]
    fn matches_on_process_name() {
        let s = snapshot(vec![sample(10, "aider", &["aider"])]);
        let c = extract_candidates(&s, ALPHA_AGENT_PATTERNS);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].matched_patterns, vec!["aider"]);
    }

    #[test]
    fn matches_on_cmdline_args() {
        let s = snapshot(vec![sample(
            10,
            "node",
            &["node", "/usr/bin/Codex", "--watch"],
        )]);
        let c = extract_candidates(&s, ALPHA_AGENT_PATTERNS);
        assert_eq!(c.len(), 1);
        assert!(c[0].matched_patterns.contains(&"codex"));
    }

    #[test]
    fn case_insensitive() {
        let s = snapshot(vec![sample(10, "CLAUDE-CODE", &["CLAUDE-CODE"])]);
        let c = extract_candidates(&s, ALPHA_AGENT_PATTERNS);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].matched_patterns, vec!["claude"]);
    }

    #[test]
    fn no_match_returns_empty() {
        let s = snapshot(vec![sample(10, "bash", &["bash"])]);
        let c = extract_candidates(&s, ALPHA_AGENT_PATTERNS);
        assert!(c.is_empty());
    }

    #[test]
    fn multiple_patterns_recorded() {
        let s = snapshot(vec![sample(10, "agent-runner", &["agent-runner", "aider"])]);
        let c = extract_candidates(&s, ALPHA_AGENT_PATTERNS);
        assert_eq!(c.len(), 1);
        assert!(c[0].matched_patterns.contains(&"agent"));
        assert!(c[0].matched_patterns.contains(&"aider"));
    }
}
