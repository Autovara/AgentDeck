//! Command parsing and reply formatting for the read-only Telegram
//! commands listed in `planning/alpha-build-plan.md` §11.2.
//!
//! Every public function in this module is **pure**: it takes
//! pre-fetched session / attention data and returns the string the bot
//! should send. That keeps the formatting trivially unit-testable
//! without a live storage handle or a real teloxide bot.
//!
//! Reply format is **plain text** — no MarkdownV2, no HTML. Telegram's
//! formatters require escaping every `_`, `*`, `[`, `(`, `)`, `~`,
//! ``` ` ```, `>`, `#`, `+`, `-`, `=`, `|`, `{`, `}`, `.`, `!` in
//! MarkdownV2; one missed escape rejects the whole message. Plain text
//! sidesteps that class of bug entirely at the cost of bold / italics
//! that the alpha does not need.

use std::time::Duration;

use agentdeck_adapter::SessionStatus;
use agentdeck_attention::{AttentionItem, AttentionSeverity, OpenAttentionEntry};
use agentdeck_session::Session;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Maximum sessions/attention items shown directly in a list reply.
/// More items collapse to a `+N more (open dashboard)` footer.
pub const MAX_LIST_ROWS: usize = 10;

/// Length of the short session id surfaced in chat output. Six hex
/// chars over UUID4 give ~16M values, more than enough for the alpha.
pub const SHORT_ID_LEN: usize = 6;

// ----- command parsing ----------------------------------------------------

/// Recognised slash command from a Telegram message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BotCommand {
    Help,
    Status,
    Agents,
    Attention,
    /// `/session <id>` — `id` is the raw user-typed prefix.
    Session(String),
    /// `/session` with no argument.
    SessionUsage,
    /// `/foo` where `foo` is not a known command. The string is what
    /// the user typed (without the leading slash) so the reply can
    /// echo it back.
    Unknown(String),
}

/// Parse one message body as a slash command.
///
/// Returns `None` when the text is not a slash command at all (so the
/// caller can route it through other handlers, e.g. `PAIR <code>`).
pub fn parse_command(text: &str) -> Option<BotCommand> {
    let text = text.trim();
    let rest = text.strip_prefix('/')?;
    if rest.is_empty() {
        return Some(BotCommand::Unknown(String::new()));
    }

    let mut parts = rest.splitn(2, char::is_whitespace);
    let head = parts.next().unwrap_or("");
    let arg = parts.next().map(str::trim).unwrap_or("");

    // Strip `@botname` suffix added by some Telegram clients in groups.
    let head = head.split('@').next().unwrap_or(head).to_ascii_lowercase();

    Some(match head.as_str() {
        "help" | "start" => BotCommand::Help,
        "status" => BotCommand::Status,
        "agents" => BotCommand::Agents,
        "attention" => BotCommand::Attention,
        "session" if arg.is_empty() => BotCommand::SessionUsage,
        "session" => BotCommand::Session(arg.to_string()),
        other => BotCommand::Unknown(other.to_string()),
    })
}

// ----- session lookup -----------------------------------------------------

/// Outcome of looking up a short session id against the active set.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionLookup<'a> {
    NotFound,
    Found(&'a Session),
    /// Multiple sessions share the typed prefix.
    Ambiguous(Vec<&'a Session>),
}

/// Render the 6-char short id used everywhere chat output mentions a
/// session.
pub fn short_session_id(session_id: Uuid) -> String {
    let simple = session_id.simple().to_string();
    simple.chars().take(SHORT_ID_LEN).collect()
}

/// Match the user-typed query against the short id of every session in
/// `sessions`. Matching is case-insensitive prefix.
pub fn lookup_session<'a>(query: &str, sessions: &'a [Session]) -> SessionLookup<'a> {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() {
        return SessionLookup::NotFound;
    }
    let matches: Vec<&'a Session> = sessions
        .iter()
        .filter(|s| s.id.simple().to_string().to_ascii_lowercase().starts_with(&q))
        .collect();
    match matches.len() {
        0 => SessionLookup::NotFound,
        1 => SessionLookup::Found(matches[0]),
        _ => SessionLookup::Ambiguous(matches),
    }
}

// ----- formatters: pure functions producing reply strings ----------------

/// `/help` — short, fixed.
pub fn format_help() -> String {
    let mut s = String::new();
    s.push_str("AgentDeck (alpha) — available commands\n\n");
    s.push_str("/help        Show this message\n");
    s.push_str("/status      Headline counts\n");
    s.push_str("/agents      List active sessions\n");
    s.push_str("/attention   List open attention items\n");
    s.push_str("/session <id>  Detail for one session\n\n");
    s.push_str("Session ids are the first 6 hex chars shown in /agents.\n");
    s.push_str("/mute and /stop arrive in later alpha builds.");
    s
}

/// `/status` — single-screen summary.
pub fn format_status(
    sessions: &[Session],
    attention: &[OpenAttentionEntry],
) -> String {
    let counts = AttentionCounts::from(attention);
    let stalled = count_by_status(sessions, SessionStatus::Stalled);
    let waiting = count_by_status(sessions, SessionStatus::WaitingForInput);

    let mut s = String::new();
    s.push_str("AgentDeck status\n\n");
    s.push_str(&format!("Active sessions: {}\n", sessions.len()));
    s.push_str(&format!(
        "Attention: {} ({} urgent, {} warn, {} info)\n",
        counts.total, counts.urgent, counts.warn, counts.info,
    ));
    s.push_str(&format!("Stalled: {}\n", stalled));
    s.push_str(&format!("Waiting for input: {}\n", waiting));
    s.push_str("Today's cost: —");
    s
}

/// `/agents` — list of active sessions, capped.
pub fn format_agents(sessions: &[Session], now: DateTime<Utc>) -> String {
    if sessions.is_empty() {
        return "No active agent sessions.".to_string();
    }
    let mut s = String::new();
    s.push_str(&format!("Active agents ({}):\n\n", sessions.len()));
    for session in sessions.iter().take(MAX_LIST_ROWS) {
        s.push_str(&format_agent_row(session, now));
        s.push('\n');
    }
    if sessions.len() > MAX_LIST_ROWS {
        let extra = sessions.len() - MAX_LIST_ROWS;
        s.push_str(&format!("\n+{extra} more (open AgentDeck dashboard)"));
    }
    s
}

/// `/attention` — open attention items, capped.
pub fn format_attention(items: &[OpenAttentionEntry], now: DateTime<Utc>) -> String {
    if items.is_empty() {
        return "No open attention items.".to_string();
    }
    let mut s = String::new();
    s.push_str(&format!("Open attention ({}):\n\n", items.len()));
    for entry in items.iter().take(MAX_LIST_ROWS) {
        s.push_str(&format_attention_row(entry, now));
        s.push('\n');
    }
    if items.len() > MAX_LIST_ROWS {
        let extra = items.len() - MAX_LIST_ROWS;
        s.push_str(&format!("\n+{extra} more (open AgentDeck dashboard)"));
    }
    s
}

/// `/session <id>` — full detail for one session.
pub fn format_session_detail(
    session: &Session,
    attention_for_session: &[AttentionItem],
    now: DateTime<Utc>,
) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "Session [{}]\n\n",
        short_session_id(session.id)
    ));
    s.push_str(&format!("Agent: {}\n", session.agent_name));
    s.push_str(&format!("Adapter: {}\n", session.adapter_name));
    s.push_str(&format!("Status: {}\n", session.status.as_db_str()));
    s.push_str(&format!(
        "Repo: {}\n",
        session.repo_path.as_deref().unwrap_or("—")
    ));
    s.push_str(&format!(
        "PID: {}\n",
        session
            .pid
            .map(|p| p.to_string())
            .unwrap_or_else(|| "—".to_string())
    ));
    s.push_str(&format!("Command: {}\n", truncate(&session.command, 200)));
    s.push_str(&format!(
        "Last seen: {}\n",
        format_relative(session.last_seen_time, now)
    ));

    if attention_for_session.is_empty() {
        s.push_str("\nNo open attention items for this session.");
    } else {
        s.push_str(&format!(
            "\nAttention ({}):\n",
            attention_for_session.len()
        ));
        for item in attention_for_session {
            s.push_str(&format!(
                "- {} {} ({})\n",
                severity_prefix(item.severity),
                item.message,
                item.reason.as_db_str(),
            ));
        }
    }
    s
}

/// Reply when `/session <id>` matches nothing in the active set.
pub fn format_session_not_found(query: &str) -> String {
    format!(
        "No active session id starts with {query:?}. Try /agents to see available ids."
    )
}

/// Reply when `/session <id>` matches more than one session.
pub fn format_session_ambiguous(query: &str, candidates: &[&Session]) -> String {
    let mut s = String::new();
    s.push_str(&format!(
        "{} sessions match {query:?}:\n",
        candidates.len()
    ));
    for c in candidates.iter().take(5) {
        s.push_str(&format!(
            "- [{}] {} · {}\n",
            short_session_id(c.id),
            c.agent_name,
            c.repo_path.as_deref().unwrap_or("—"),
        ));
    }
    if candidates.len() > 5 {
        s.push_str(&format!("…and {} more\n", candidates.len() - 5));
    }
    s.push_str("\nUse a longer id prefix.");
    s
}

/// Reply for `/session` with no argument.
pub fn format_session_usage() -> String {
    "Usage: /session <id> (id is the 6-char prefix shown in /agents)".to_string()
}

/// Reply for any unknown slash command.
pub fn format_unknown_command(command: &str) -> String {
    if command.is_empty() {
        "Unknown command. Try /help.".to_string()
    } else {
        format!("Unknown command /{command}. Try /help.")
    }
}

/// Reply for rate-limited callers.
pub fn format_rate_limited(retry_after: Duration) -> String {
    let secs = retry_after.as_secs().max(1);
    format!("Too many commands. Try again in {secs}s.")
}

// ----- internal helpers ---------------------------------------------------

fn format_agent_row(session: &Session, now: DateTime<Utc>) -> String {
    let short = short_session_id(session.id);
    let repo = session.repo_path.as_deref().unwrap_or("—");
    format!(
        "- [{}] {} · {}\n    status: {} · last seen {}",
        short,
        session.agent_name,
        repo,
        session.status.as_db_str(),
        format_relative(session.last_seen_time, now),
    )
}

fn format_attention_row(entry: &OpenAttentionEntry, now: DateTime<Utc>) -> String {
    let prefix = severity_prefix(entry.item.severity);
    let repo = entry.session.repo_path.as_deref().unwrap_or("—");
    let session_short = short_session_id(entry.session.id);
    format!(
        "- {} [{}] {} · {}\n    {} ({}, {})",
        prefix,
        session_short,
        entry.session.agent_name,
        repo,
        entry.item.message,
        entry.item.reason.as_db_str(),
        format_relative(entry.item.created_at, now),
    )
}

fn severity_prefix(severity: AttentionSeverity) -> &'static str {
    match severity {
        AttentionSeverity::Urgent => "[!]",
        AttentionSeverity::Warn => "[w]",
        AttentionSeverity::Info => "[i]",
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let kept: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{kept}…")
    }
}

fn format_relative(ts: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let diff = (now - ts).num_seconds().max(0);
    if diff < 5 {
        return "just now".to_string();
    }
    if diff < 60 {
        return format!("{diff}s ago");
    }
    let minutes = diff / 60;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    format!("{}d ago", hours / 24)
}

fn count_by_status(sessions: &[Session], status: SessionStatus) -> usize {
    sessions.iter().filter(|s| s.status == status).count()
}

/// Severity breakdown of the open attention set; used by `/status`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct AttentionCounts {
    total: usize,
    urgent: usize,
    warn: usize,
    info: usize,
}

impl AttentionCounts {
    fn from(items: &[OpenAttentionEntry]) -> Self {
        let mut c = Self {
            total: items.len(),
            ..Self::default()
        };
        for e in items {
            match e.item.severity {
                AttentionSeverity::Urgent => c.urgent += 1,
                AttentionSeverity::Warn => c.warn += 1,
                AttentionSeverity::Info => c.info += 1,
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentdeck_adapter::{CapabilityLevel, Confidence, CostKind};
    use agentdeck_attention::model::SessionRef;
    use agentdeck_attention::{AttentionReason, RecommendedAction};
    use chrono::TimeZone;

    fn ts(secs: i64) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 5, 22, 16, 0, 0).unwrap() + chrono::Duration::seconds(secs)
    }

    fn session(agent: &str, status: SessionStatus, repo: Option<&str>) -> Session {
        Session {
            id: Uuid::new_v4(),
            agent_name: agent.into(),
            adapter_name: agent.into(),
            adapter_level: CapabilityLevel::Presence,
            pid: Some(101),
            command: format!("{agent} /repo"),
            cwd: repo.map(str::to_string),
            repo_path: repo.map(str::to_string),
            project_tag: None,
            status,
            status_confidence: Confidence::Medium,
            attention_reason: None,
            start_time: ts(-300),
            last_seen_time: ts(-30),
            last_activity_time: None,
            estimated_cost: None,
            cost_kind: CostKind::Unknown,
            created_at: ts(-300),
            updated_at: ts(-30),
        }
    }

    fn open_attention(session: &Session, severity: AttentionSeverity) -> OpenAttentionEntry {
        OpenAttentionEntry {
            item: AttentionItem {
                id: Uuid::new_v4(),
                session_id: session.id,
                reason: AttentionReason::RateLimit,
                severity,
                message: format!("{} hit a rate limit", session.agent_name),
                source: "session-status:rate_limited".into(),
                confidence: Confidence::Medium,
                recommended_actions: vec![RecommendedAction::Mute],
                created_at: ts(-60),
                resolved_at: None,
                muted_until: None,
            },
            session: SessionRef {
                id: session.id,
                agent_name: session.agent_name.clone(),
                adapter_name: session.adapter_name.clone(),
                status: session.status,
                pid: session.pid,
                repo_path: session.repo_path.clone(),
                project_tag: None,
            },
        }
    }

    // ----- parser -----------------------------------------------------

    #[test]
    fn parse_returns_none_for_non_commands() {
        assert!(parse_command("hello world").is_none());
        assert!(parse_command("PAIR ABCDEF").is_none());
        assert!(parse_command("").is_none());
    }

    #[test]
    fn parse_recognises_each_command() {
        assert_eq!(parse_command("/help"), Some(BotCommand::Help));
        assert_eq!(parse_command("/start"), Some(BotCommand::Help));
        assert_eq!(parse_command("/status"), Some(BotCommand::Status));
        assert_eq!(parse_command("/agents"), Some(BotCommand::Agents));
        assert_eq!(parse_command("/attention"), Some(BotCommand::Attention));
        assert_eq!(
            parse_command("/session abc123"),
            Some(BotCommand::Session("abc123".into()))
        );
        assert_eq!(parse_command("/session"), Some(BotCommand::SessionUsage));
    }

    #[test]
    fn parse_strips_botname_suffix() {
        assert_eq!(
            parse_command("/status@MyAgentDeckBot"),
            Some(BotCommand::Status)
        );
        assert_eq!(
            parse_command("/session@MyAgentDeckBot abc123"),
            Some(BotCommand::Session("abc123".into()))
        );
    }

    #[test]
    fn parse_is_case_insensitive_on_command_name() {
        assert_eq!(parse_command("/HELP"), Some(BotCommand::Help));
        assert_eq!(parse_command("/Status"), Some(BotCommand::Status));
    }

    #[test]
    fn parse_unknown_command_carries_name() {
        assert_eq!(
            parse_command("/foo bar baz"),
            Some(BotCommand::Unknown("foo".into()))
        );
    }

    // ----- short id + lookup ------------------------------------------

    #[test]
    fn short_session_id_is_six_hex_chars() {
        let s = session("aider", SessionStatus::Running, None);
        let sid = short_session_id(s.id);
        assert_eq!(sid.len(), 6);
        assert!(sid.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn lookup_finds_unique_prefix() {
        let s = session("aider", SessionStatus::Running, None);
        let prefix = short_session_id(s.id);
        let result = lookup_session(&prefix, std::slice::from_ref(&s));
        match result {
            SessionLookup::Found(found) => assert_eq!(found.id, s.id),
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn lookup_not_found_for_empty_or_no_match() {
        let s = session("aider", SessionStatus::Running, None);
        assert!(matches!(
            lookup_session("", std::slice::from_ref(&s)),
            SessionLookup::NotFound
        ));
        assert!(matches!(
            lookup_session("zzzzzz", std::slice::from_ref(&s)),
            SessionLookup::NotFound
        ));
    }

    #[test]
    fn lookup_ambiguous_when_two_sessions_share_prefix() {
        // Hand-construct two sessions with overlapping short ids.
        let mut a = session("aider", SessionStatus::Running, None);
        let mut b = session("codex", SessionStatus::Running, None);
        a.id = Uuid::parse_str("abc123de-0000-4000-8000-000000000001").unwrap();
        b.id = Uuid::parse_str("abc123de-0000-4000-8000-000000000002").unwrap();
        let sessions = vec![a, b];
        let result = lookup_session("abc123", &sessions);
        match result {
            SessionLookup::Ambiguous(candidates) => assert_eq!(candidates.len(), 2),
            other => panic!("expected Ambiguous, got {other:?}"),
        }
    }

    // ----- formatters --------------------------------------------------

    #[test]
    fn help_mentions_each_command() {
        let h = format_help();
        for c in ["/help", "/status", "/agents", "/attention", "/session"] {
            assert!(h.contains(c), "/help should mention {c}; got: {h}");
        }
    }

    #[test]
    fn status_includes_counts_and_cost_placeholder() {
        let a = session("aider", SessionStatus::Running, Some("/repo/a"));
        let s = session("aider", SessionStatus::Stalled, Some("/repo/b"));
        let w = session("aider", SessionStatus::WaitingForInput, Some("/repo/c"));
        let att = vec![
            open_attention(&s, AttentionSeverity::Warn),
            open_attention(&w, AttentionSeverity::Urgent),
        ];
        let reply = format_status(&[a, s, w], &att);
        assert!(reply.contains("Active sessions: 3"));
        assert!(reply.contains("Attention: 2"));
        assert!(reply.contains("1 urgent"));
        assert!(reply.contains("1 warn"));
        assert!(reply.contains("Stalled: 1"));
        assert!(reply.contains("Waiting for input: 1"));
        assert!(reply.contains("Today's cost: —"));
    }

    #[test]
    fn agents_lists_sessions_with_short_id() {
        let a = session("aider", SessionStatus::Running, Some("/repo/a"));
        let reply = format_agents(std::slice::from_ref(&a), ts(0));
        assert!(reply.contains("Active agents (1)"));
        assert!(reply.contains(&short_session_id(a.id)));
        assert!(reply.contains("aider"));
        assert!(reply.contains("/repo/a"));
    }

    #[test]
    fn agents_empty_reply_is_explicit() {
        let reply = format_agents(&[], ts(0));
        assert_eq!(reply, "No active agent sessions.");
    }

    #[test]
    fn agents_caps_to_ten_with_more_footer() {
        let sessions: Vec<Session> = (0..12)
            .map(|_| session("aider", SessionStatus::Running, Some("/repo")))
            .collect();
        let reply = format_agents(&sessions, ts(0));
        assert!(reply.contains("Active agents (12)"));
        assert!(reply.contains("+2 more"));
    }

    #[test]
    fn attention_lists_items_with_severity_prefix() {
        let s = session("aider", SessionStatus::RateLimited, Some("/repo/a"));
        let entry = open_attention(&s, AttentionSeverity::Urgent);
        let reply = format_attention(&[entry], ts(0));
        assert!(reply.contains("Open attention (1)"));
        assert!(reply.contains("[!]"));
        assert!(reply.contains("rate_limit"));
        assert!(reply.contains("aider"));
    }

    #[test]
    fn attention_empty_reply_is_explicit() {
        let reply = format_attention(&[], ts(0));
        assert_eq!(reply, "No open attention items.");
    }

    #[test]
    fn session_detail_includes_every_field() {
        let s = session("aider", SessionStatus::Running, Some("/repo/billing"));
        let att = vec![open_attention(&s, AttentionSeverity::Warn).item];
        let reply = format_session_detail(&s, &att, ts(0));
        assert!(reply.contains(&short_session_id(s.id)));
        assert!(reply.contains("Agent: aider"));
        assert!(reply.contains("Adapter: aider"));
        assert!(reply.contains("Status: running"));
        assert!(reply.contains("/repo/billing"));
        assert!(reply.contains("PID: 101"));
        assert!(reply.contains("Last seen"));
        assert!(reply.contains("[w]"));
    }

    #[test]
    fn session_detail_with_no_attention_says_so() {
        let s = session("aider", SessionStatus::Running, None);
        let reply = format_session_detail(&s, &[], ts(0));
        assert!(reply.contains("No open attention items for this session"));
    }

    #[test]
    fn session_not_found_quotes_query() {
        let r = format_session_not_found("zzzzzz");
        assert!(r.contains("\"zzzzzz\""));
    }

    #[test]
    fn session_ambiguous_lists_candidates() {
        let a = session("aider", SessionStatus::Running, Some("/a"));
        let b = session("codex", SessionStatus::Running, Some("/b"));
        let r = format_session_ambiguous("abc", &[&a, &b]);
        assert!(r.contains("2 sessions match"));
        assert!(r.contains("aider"));
        assert!(r.contains("codex"));
        assert!(r.contains("Use a longer id prefix"));
    }

    #[test]
    fn unknown_command_includes_name_when_present() {
        assert!(format_unknown_command("foo").contains("/foo"));
        assert!(format_unknown_command("").contains("Unknown command"));
    }

    #[test]
    fn rate_limited_reply_includes_seconds_floor_one() {
        let r = format_rate_limited(Duration::from_millis(200));
        assert!(r.contains("1s"));
        let r = format_rate_limited(Duration::from_secs(42));
        assert!(r.contains("42s"));
    }
}
