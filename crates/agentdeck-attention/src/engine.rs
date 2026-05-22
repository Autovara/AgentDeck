//! The attention engine.
//!
//! Given the [`TickReport`] from
//! [`agentdeck_session::SessionStateMachine::apply`] and the current
//! session list, the engine:
//!
//! 1. Computes the desired set of open attention items (one per
//!    `(session_id, reason)` pair).
//! 2. Compares against the rows currently open in `attention_items`.
//! 3. Inserts new rows, updates the recommendation on existing ones,
//!    resolves rows whose session has completed or whose rule no longer
//!    fires, and *skips updates* on rows the user has muted.
//!
//! All writes for a single `apply` call happen in one transaction.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agentdeck_core::Clock;
use agentdeck_session::{Session, TickReport};
use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::AttentionError;
use crate::model::{AttentionItem, ProposedAttention, SessionRef};
use crate::reason::AttentionReason;
use crate::repository;
use crate::rules::propose_from_session;

/// One run of [`AttentionEngine::apply`].
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionTickReport {
    /// IDs of items inserted on this tick.
    pub created: Vec<Uuid>,
    /// IDs of items whose severity / message / actions changed.
    pub updated: Vec<Uuid>,
    /// IDs of items resolved on this tick (session completed, rule
    /// stopped firing, or the underlying session vanished).
    pub resolved: Vec<Uuid>,
    /// IDs of items left untouched because they are currently muted.
    pub skipped_muted: Vec<Uuid>,
}

/// Combined attention item + parent-session label, ready for the
/// dashboard.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAttentionEntry {
    pub item: AttentionItem,
    pub session: SessionRef,
}

/// The attention engine. Owns a shared [`Storage`] handle plus a
/// [`Clock`] used for `created_at` / `resolved_at` / mute math.
pub struct AttentionEngine {
    storage: Arc<Storage>,
    clock: Arc<dyn Clock>,
}

impl AttentionEngine {
    pub fn new(storage: Arc<Storage>, clock: Arc<dyn Clock>) -> Self {
        Self { storage, clock }
    }

    /// Apply one tick's worth of session data.
    ///
    /// `tick.completed` is treated as authoritative: every open
    /// attention item attached to a completed session is resolved.
    pub fn apply(
        &self,
        tick: &TickReport,
        sessions: &[Session],
    ) -> Result<AttentionTickReport, AttentionError> {
        let now = self.clock.now();

        // 1. Build the desired set of proposals keyed by (session, reason).
        let mut proposals: HashMap<(Uuid, AttentionReason), ProposedAttention> = HashMap::new();
        for s in sessions {
            if let Some(p) = propose_from_session(s) {
                proposals.insert((p.session_id, p.reason), p);
            }
        }
        let completed: HashSet<Uuid> = tick.completed.iter().copied().collect();

        let report = self.storage.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let open = repository::list_open(&tx)?;
            let mut report = AttentionTickReport::default();
            let mut still_open: HashSet<(Uuid, AttentionReason)> = HashSet::new();

            for item in &open {
                let key = (item.session_id, item.reason);

                if completed.contains(&item.session_id) {
                    repository::mark_resolved(&tx, item.id, now)?;
                    report.resolved.push(item.id);
                    continue;
                }

                match proposals.get(&key) {
                    Some(p) => {
                        if item.is_muted_at(now) {
                            // Keep the item open but do not touch its
                            // fields while the user has it muted.
                            still_open.insert(key);
                            report.skipped_muted.push(item.id);
                        } else if p.differs_from(item) {
                            repository::update_proposal(&tx, item.id, p)?;
                            report.updated.push(item.id);
                            still_open.insert(key);
                        } else {
                            still_open.insert(key);
                        }
                    }
                    None => {
                        repository::mark_resolved(&tx, item.id, now)?;
                        report.resolved.push(item.id);
                    }
                }
            }

            for (key, p) in proposals.iter() {
                if still_open.contains(key) {
                    continue;
                }
                let new_id = Uuid::new_v4();
                let item = p.clone().into_item(new_id, now);
                repository::insert(&tx, &item)?;
                report.created.push(new_id);
            }

            tx.commit()?;
            Ok(report)
        })?;

        Ok(report)
    }

    /// List every open attention item joined with its parent session.
    /// Sorted urgent → warn → info, then oldest first.
    pub fn list_open(&self) -> Result<Vec<OpenAttentionEntry>, AttentionError> {
        let rows = self.storage.with_conn(repository::list_open_with_session)?;
        Ok(rows
            .into_iter()
            .map(|(item, session)| OpenAttentionEntry { item, session })
            .collect())
    }

    /// Set `muted_until` on an open item. Passing `None` clears the
    /// mute.
    pub fn set_mute(
        &self,
        id: Uuid,
        until: Option<DateTime<Utc>>,
    ) -> Result<AttentionItem, AttentionError> {
        let result = self.storage.with_conn_mut(|conn| {
            let affected = repository::set_muted_until(conn, id, until)?;
            let row = repository::fetch(conn, id)?;
            Ok((affected, row))
        })?;
        let (affected, row) = result;
        translate_update_result(id, affected, row)
    }

    /// Resolve an item manually (e.g. user clicked "Resolve" or
    /// `/mute` on Telegram once the engine adds it later).
    pub fn resolve(&self, id: Uuid) -> Result<AttentionItem, AttentionError> {
        let now = self.clock.now();
        let result = self.storage.with_conn_mut(|conn| {
            let affected = repository::mark_resolved(conn, id, now)?;
            let row = repository::fetch(conn, id)?;
            Ok((affected, row))
        })?;
        let (affected, row) = result;
        translate_update_result(id, affected, row)
    }
}

/// Map `(rows_affected, fetched_row)` into the right [`AttentionError`].
///
/// * 0 rows affected and no row found    → [`AttentionError::NotFound`]
/// * 0 rows affected but row exists      → [`AttentionError::AlreadyResolved`]
/// * 1 row affected and row found        → `Ok(item)`
/// * 1 row affected but row missing      → unreachable (we just touched it)
fn translate_update_result(
    id: Uuid,
    affected: usize,
    fetched: Option<AttentionItem>,
) -> Result<AttentionItem, AttentionError> {
    match (affected, fetched) {
        (1.., Some(item)) => Ok(item),
        (1.., None) => {
            // We affected a row but failed to read it back. The most
            // helpful classification is NotFound (something deleted it
            // between the UPDATE and the SELECT).
            Err(AttentionError::NotFound(id))
        }
        (0, Some(item)) if item.resolved_at.is_some() => Err(AttentionError::AlreadyResolved(id)),
        (0, Some(_)) | (0, None) => Err(AttentionError::NotFound(id)),
    }
}
