//! Tray menu rendering.
//!
//! The tray menu is rebuilt from a [`TrayMenuSnapshot`] on every
//! refresh. We do not mutate menu items in-place because Tauri's tray
//! menu has no per-item update API for label / disabled state — building
//! a fresh `Menu` and calling [`tauri::tray::TrayIcon::set_menu`] is the
//! supported path.
//!
//! The layout follows `planning/alpha-build-plan.md` §13:
//!
//! ```text
//! 3 active · 1 waiting     (disabled, info-only)
//! $— today                 (disabled; cost lands in step 18)
//! ─────────────────────────
//! Attention:               (disabled section header)
//!   [!] claude-7 · billing-api
//!   [w] aider-3 · web-app
//!   [i] codex-2 · api
//! ─────────────────────────
//! Open Dashboard
//! Refresh
//! Pause Alerts / Resume Alerts
//! ─────────────────────────
//! Quit AgentDeck
//! ```
//!
//! Severity prefixes are plain ASCII (`[!]`, `[w]`, `[i]`) rather than
//! emoji to render reliably across the older XEmbed Linux trays we
//! support per `planning/tray-surface-feasibility.md`.

use agentdeck_attention::{AttentionSeverity, OpenAttentionEntry};
use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder};
use tauri::{AppHandle, Runtime};
use uuid::Uuid;

/// Maximum number of attention items shown directly in the tray menu;
/// the remainder is collapsed to `+N more`.
const MAX_ATTENTION_IN_TRAY: usize = 5;

/// Maximum length of any free-form label segment (agent name, repo
/// label) before we truncate with `…`. Some Linux trays truncate harder
/// than others; keeping the label short avoids menu items disappearing.
const MAX_LABEL_SEGMENT: usize = 24;

/// Menu item id for "Open Dashboard".
pub const MENU_ID_OPEN_DASHBOARD: &str = "open_dashboard";

/// Menu item id for "Refresh".
pub const MENU_ID_REFRESH: &str = "refresh_now";

/// Menu item id for "Pause Alerts" / "Resume Alerts".
pub const MENU_ID_PAUSE_ALERTS: &str = "pause_alerts";

/// Menu item id for "Quit AgentDeck".
pub const MENU_ID_QUIT: &str = "quit";

/// Prefix used for attention item menu ids. The full id is
/// `attention:<uuid>` so [`parse_attention_id`] can recover the UUID
/// when the user clicks the row.
pub const ATTENTION_PREFIX: &str = "attention:";

/// Read-only state the menu needs to render. Populated by the tray
/// scheduler before every rebuild.
#[derive(Debug, Clone)]
pub struct TrayMenuSnapshot {
    /// `true` once storage and the attention engine have produced at
    /// least one successful read. `false` triggers a degraded "not
    /// ready" header instead of the summary lines.
    pub ready: bool,
    pub active_sessions: usize,
    pub waiting_sessions: usize,
    /// Open attention items, urgent → warn → info, then oldest first.
    /// Capped to a handful of rows by [`build_menu`].
    pub attention: Vec<OpenAttentionEntry>,
    /// Whether the "Pause Alerts" toggle is currently on. Drives the
    /// label of [`MENU_ID_PAUSE_ALERTS`].
    pub alerts_paused: bool,
}

impl TrayMenuSnapshot {
    /// Snapshot used at startup and whenever storage is unavailable.
    pub fn unavailable(alerts_paused: bool) -> Self {
        Self {
            ready: false,
            active_sessions: 0,
            waiting_sessions: 0,
            attention: Vec::new(),
            alerts_paused,
        }
    }
}

/// Build a fresh tray [`Menu`] from a snapshot.
pub fn build_menu<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &TrayMenuSnapshot,
) -> tauri::Result<Menu<R>> {
    let mut b = MenuBuilder::new(app);

    if snapshot.ready {
        let summary = MenuItemBuilder::with_id(
            "summary_sessions",
            format!(
                "{} active · {} waiting",
                snapshot.active_sessions, snapshot.waiting_sessions
            ),
        )
        .enabled(false)
        .build(app)?;
        let cost = MenuItemBuilder::with_id("summary_cost", "$— today")
            .enabled(false)
            .build(app)?;
        b = b.items(&[&summary, &cost]).separator();

        if snapshot.attention.is_empty() {
            let none = MenuItemBuilder::with_id("attention_empty", "No attention items")
                .enabled(false)
                .build(app)?;
            b = b.item(&none).separator();
        } else {
            let header = MenuItemBuilder::with_id("attention_header", "Attention:")
                .enabled(false)
                .build(app)?;
            b = b.item(&header);

            for entry in snapshot.attention.iter().take(MAX_ATTENTION_IN_TRAY) {
                let id = format!("{ATTENTION_PREFIX}{}", entry.item.id);
                let label = attention_label(entry);
                let item = MenuItemBuilder::with_id(id, label).build(app)?;
                b = b.item(&item);
            }
            if snapshot.attention.len() > MAX_ATTENTION_IN_TRAY {
                let extra = snapshot.attention.len() - MAX_ATTENTION_IN_TRAY;
                let more = MenuItemBuilder::with_id("attention_more", format!("  +{extra} more"))
                    .enabled(false)
                    .build(app)?;
                b = b.item(&more);
            }
            b = b.separator();
        }
    } else {
        let not_ready = MenuItemBuilder::with_id("not_ready", "AgentDeck not ready")
            .enabled(false)
            .build(app)?;
        b = b.item(&not_ready).separator();
    }

    let open = MenuItemBuilder::with_id(MENU_ID_OPEN_DASHBOARD, "Open Dashboard").build(app)?;
    let refresh = MenuItemBuilder::with_id(MENU_ID_REFRESH, "Refresh").build(app)?;
    let pause = MenuItemBuilder::with_id(MENU_ID_PAUSE_ALERTS, pause_alerts_label(snapshot.alerts_paused))
        .build(app)?;
    let quit = MenuItemBuilder::with_id(MENU_ID_QUIT, "Quit AgentDeck").build(app)?;

    b.items(&[&open, &refresh, &pause]).separator().item(&quit).build()
}

/// Recover the UUID of an attention item from its menu id. Returns
/// `None` for any id that does not start with [`ATTENTION_PREFIX`] or
/// whose suffix is not a valid UUID.
pub fn parse_attention_id(menu_id: &str) -> Option<Uuid> {
    menu_id
        .strip_prefix(ATTENTION_PREFIX)
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Label for the "Pause Alerts" / "Resume Alerts" toggle.
pub fn pause_alerts_label(paused: bool) -> &'static str {
    if paused {
        "Resume Alerts"
    } else {
        "Pause Alerts"
    }
}

/// Build the per-row label `"  [prefix] agent · repo"` for one
/// attention item. Public for unit tests; the menu builder is the only
/// runtime caller.
pub fn attention_label(entry: &OpenAttentionEntry) -> String {
    let prefix = severity_prefix(entry.item.severity);
    let agent = truncate(&entry.session.agent_name, MAX_LABEL_SEGMENT);
    let repo = repo_label(entry.session.repo_path.as_deref());
    format!("  {prefix} {agent} · {repo}")
}

fn severity_prefix(severity: AttentionSeverity) -> &'static str {
    match severity {
        AttentionSeverity::Urgent => "[!]",
        AttentionSeverity::Warn => "[w]",
        AttentionSeverity::Info => "[i]",
    }
}

fn repo_label(repo: Option<&str>) -> String {
    match repo {
        Some(path) => {
            let last = path.rsplit(['/', '\\']).next().unwrap_or(path);
            let s = if last.is_empty() { path } else { last };
            truncate(s, MAX_LABEL_SEGMENT)
        }
        None => "—".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use agentdeck_adapter::{Confidence, SessionStatus};
    use agentdeck_attention::{AttentionItem, AttentionReason, AttentionSeverity};
    use agentdeck_attention::model::SessionRef;
    use agentdeck_attention::RecommendedAction;
    use chrono::{TimeZone, Utc};

    fn entry(
        agent: &str,
        repo: Option<&str>,
        severity: AttentionSeverity,
    ) -> OpenAttentionEntry {
        let id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        OpenAttentionEntry {
            item: AttentionItem {
                id,
                session_id,
                reason: AttentionReason::WaitingForInput,
                severity,
                message: format!("{agent} needs attention"),
                source: "session-status:waiting_for_input".into(),
                confidence: Confidence::Medium,
                recommended_actions: vec![RecommendedAction::OpenDashboard],
                created_at: Utc.with_ymd_and_hms(2026, 5, 22, 14, 0, 0).unwrap(),
                resolved_at: None,
                muted_until: None,
            },
            session: SessionRef {
                id: session_id,
                agent_name: agent.into(),
                adapter_name: agent.into(),
                status: SessionStatus::WaitingForInput,
                pid: Some(101),
                repo_path: repo.map(|r| r.to_string()),
                project_tag: None,
            },
        }
    }

    #[test]
    fn parse_attention_id_round_trips() {
        let id = Uuid::new_v4();
        let menu_id = format!("{ATTENTION_PREFIX}{id}");
        assert_eq!(parse_attention_id(&menu_id), Some(id));
    }

    #[test]
    fn parse_attention_id_rejects_non_attention_ids() {
        assert!(parse_attention_id(MENU_ID_OPEN_DASHBOARD).is_none());
        assert!(parse_attention_id("attention:not-a-uuid").is_none());
        assert!(parse_attention_id("").is_none());
    }

    #[test]
    fn pause_alerts_label_toggles() {
        assert_eq!(pause_alerts_label(false), "Pause Alerts");
        assert_eq!(pause_alerts_label(true), "Resume Alerts");
    }

    #[test]
    fn attention_label_uses_severity_prefix_and_repo_basename() {
        let e = entry("aider", Some("/home/me/work/billing-api"), AttentionSeverity::Urgent);
        let label = attention_label(&e);
        assert!(label.contains("[!]"));
        assert!(label.contains("aider"));
        assert!(label.contains("billing-api"));
        assert!(!label.contains("/home/"), "must use basename, not full path");
    }

    #[test]
    fn attention_label_falls_back_to_em_dash_when_no_repo() {
        let e = entry("aider", None, AttentionSeverity::Warn);
        let label = attention_label(&e);
        assert!(label.contains("[w]"));
        assert!(label.contains("—"));
    }

    #[test]
    fn attention_label_truncates_long_segments() {
        let very_long_repo = "/some/path/this-is-a-very-very-very-long-repository-name";
        let e = entry("aider", Some(very_long_repo), AttentionSeverity::Info);
        let label = attention_label(&e);
        assert!(label.contains("…"), "long segments should be truncated");
    }

    #[test]
    fn snapshot_unavailable_carries_alerts_flag() {
        let s = TrayMenuSnapshot::unavailable(true);
        assert!(!s.ready);
        assert!(s.alerts_paused);
        assert_eq!(s.active_sessions, 0);
    }
}
