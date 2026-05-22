//! The session state machine.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use agentdeck_adapter::{AdapterMatch, SessionStatus};
use agentdeck_core::Clock;
use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::error::SessionError;
use crate::model::{
    detected_event_payload, status_changed_event_payload, Session, EVENT_KIND_DETECTED,
    EVENT_KIND_PROCESS_EXITED, EVENT_KIND_STATUS_CHANGED,
};
use crate::repository;

/// Maps adapter matches to session rows.
///
/// One state machine is shared by the whole monitor; calls to
/// [`Self::apply`] are serialised on the underlying [`Storage`] mutex, so
/// the alpha does not need any further synchronisation.
pub struct SessionStateMachine {
    storage: Arc<Storage>,
    clock: Arc<dyn Clock>,
}

impl SessionStateMachine {
    pub fn new(storage: Arc<Storage>, clock: Arc<dyn Clock>) -> Self {
        Self { storage, clock }
    }

    /// Apply one tick's worth of adapter matches to the database.
    ///
    /// `snapshot_time` is the canonical "as of" timestamp of the tick.
    /// Every insert / update / completion runs inside a single SQLite
    /// transaction so an interrupted call never leaves a half-written
    /// state.
    pub fn apply(
        &self,
        snapshot_time: DateTime<Utc>,
        matches: &[AdapterMatch],
    ) -> Result<TickReport, SessionError> {
        let now = self.clock.now();

        // Collapse duplicate `(adapter_name, pid)` matches; last writer wins.
        // We iterate in input order so the final mapping reflects the last
        // adapter to emit for each PID.
        let mut deduped: HashMap<(String, u32), &AdapterMatch> = HashMap::new();
        for m in matches {
            deduped.insert((m.adapter_name.clone(), m.pid), m);
        }

        let report = self.storage.with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            let mut report = TickReport {
                snapshot_time,
                ..TickReport::default()
            };

            // Build the active-session index keyed by (adapter_name, pid).
            // Sessions whose pid is NULL (defensive: alpha writers always
            // set it, but the column allows NULL) are tracked separately
            // so completion still works against them if they show up.
            let active = repository::list_active(&tx)?;
            let mut active_by_key: HashMap<(String, u32), Session> = HashMap::new();
            let mut pidless_active: Vec<Session> = Vec::new();
            for session in active {
                match session.pid {
                    Some(pid) => {
                        active_by_key.insert((session.adapter_name.clone(), pid), session);
                    }
                    None => pidless_active.push(session),
                }
            }

            let mut touched: HashSet<(String, u32)> = HashSet::new();

            for ((adapter_name, pid), m) in deduped.iter() {
                touched.insert((adapter_name.clone(), *pid));

                if let Some(existing) = active_by_key.get(&(adapter_name.clone(), *pid)) {
                    if existing.status != m.status {
                        report.status_changes.push(StatusChange {
                            session_id: existing.id,
                            from: existing.status,
                            to: m.status,
                        });
                        let payload = status_changed_event_payload(
                            existing.status,
                            m.status,
                            m.status_confidence,
                            &m.status_source,
                        );
                        repository::append_event(
                            &tx,
                            existing.id,
                            EVENT_KIND_STATUS_CHANGED,
                            Some(&payload),
                            now,
                        )?;
                    }
                    repository::update_from_match(&tx, existing.id, m, snapshot_time, now)?;
                    report.updated.push(existing.id);
                } else {
                    let new_id = Uuid::new_v4();
                    let session = Session::from_match_initial(new_id, m, snapshot_time, now);
                    repository::insert(&tx, &session)?;
                    let payload = detected_event_payload(m);
                    repository::append_event(
                        &tx,
                        new_id,
                        EVENT_KIND_DETECTED,
                        Some(&payload),
                        now,
                    )?;
                    report.created.push(new_id);
                }
            }

            // Anything that was active but is no longer matched -> completed.
            for ((adapter_name, pid), session) in &active_by_key {
                if !touched.contains(&(adapter_name.clone(), *pid)) {
                    repository::mark_completed(&tx, session.id, now)?;
                    repository::append_event(
                        &tx,
                        session.id,
                        EVENT_KIND_PROCESS_EXITED,
                        None,
                        now,
                    )?;
                    report.completed.push(session.id);
                }
            }

            // Pidless active rows are not expected today, but if one ever
            // ends up in the DB (manual edit, future migration) the tick
            // should not silently leak it. We complete it on first sight
            // so the active set stays clean.
            for session in &pidless_active {
                repository::mark_completed(&tx, session.id, now)?;
                repository::append_event(
                    &tx,
                    session.id,
                    EVENT_KIND_PROCESS_EXITED,
                    None,
                    now,
                )?;
                report.completed.push(session.id);
                tracing::warn!(
                    session_id = %session.id,
                    "Completed a session with NULL pid; this should not happen in the alpha.",
                );
            }

            tx.commit()?;
            Ok(report)
        })?;

        Ok(report)
    }
}

/// Summary of what changed in a single tick. Consumed by the attention
/// engine (next build-plan step) to decide which attention items to
/// create or resolve.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TickReport {
    /// "As of" timestamp of the tick that produced this report.
    pub snapshot_time: DateTime<Utc>,
    /// Sessions inserted for the first time during this tick.
    pub created: Vec<Uuid>,
    /// Sessions touched (last_seen_time refreshed) during this tick.
    pub updated: Vec<Uuid>,
    /// Sessions marked completed during this tick because their
    /// `(adapter_name, pid)` did not appear in the match set.
    pub completed: Vec<Uuid>,
    /// Status transitions observed during this tick. A subset of
    /// `updated`; one entry per session whose status field changed.
    pub status_changes: Vec<StatusChange>,
}

/// One status transition.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusChange {
    pub session_id: Uuid,
    pub from: SessionStatus,
    pub to: SessionStatus,
}
