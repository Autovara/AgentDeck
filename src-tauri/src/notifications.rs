//! Desktop notifications for newly-created urgent attention items.
//!
//! Build-plan §13 calls for desktop notifications as the primary
//! attention surface when no tray is available. The alpha keeps the
//! policy minimal:
//!
//! - Only [`AttentionSeverity::Urgent`] items trigger a notification.
//! - Only *newly created* items trigger one (we never re-notify on
//!   updates or resolutions).
//! - The notification is suppressed when
//!   [`crate::alerts::AlertsPausedState`] is on.
//!
//! All notification failures are logged at `warn` and swallowed — a
//! missing notification is recoverable; failing the tick is not.

use std::collections::HashSet;

use agentdeck_attention::{AttentionSeverity, OpenAttentionEntry};
use tauri::{AppHandle, Runtime};
use tauri_plugin_notification::NotificationExt;
use uuid::Uuid;

/// Send a desktop notification for every newly-created urgent
/// attention item.
///
/// `created_ids` is taken from
/// [`crate::monitor_tick::MonitorTickReport::attention_created`].
/// `open` is the current open-item set (already fetched by the
/// scheduler for the tray menu); we filter it down to the IDs that
/// were created on this tick and to `Urgent` severity.
pub fn notify_for_new_urgent<R: Runtime>(
    app: &AppHandle<R>,
    created_ids: &[Uuid],
    open: &[OpenAttentionEntry],
) {
    if created_ids.is_empty() {
        return;
    }
    let created: HashSet<Uuid> = created_ids.iter().copied().collect();
    for entry in open.iter().filter(|e| created.contains(&e.item.id)) {
        if entry.item.severity != AttentionSeverity::Urgent {
            continue;
        }
        let title = format!("AgentDeck: {}", entry.session.agent_name);
        let body = entry.item.message.clone();
        if let Err(err) = app
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
        {
            tracing::warn!(
                error = %err,
                attention_id = %entry.item.id,
                "failed to send desktop notification",
            );
        }
    }
}
