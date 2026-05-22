//! Engine integration tests against an in-memory `Storage`.
//!
//! These tests exercise the full pipeline: insert a session row, drive
//! `AttentionEngine::apply` with a synthetic `TickReport`, and verify
//! the attention items end up in the right shape.

use std::sync::Arc;

use agentdeck_adapter::{CapabilityLevel, Confidence, CostKind, SessionStatus};
use agentdeck_attention::{AttentionEngine, AttentionReason, AttentionSeverity, RecommendedAction};
use agentdeck_core::Clock;
use agentdeck_session::{Session, SessionStateMachine, TickReport};
use agentdeck_storage::Storage;
use chrono::{DateTime, Duration, TimeZone, Utc};
use rusqlite::params;
use uuid::Uuid;

#[derive(Clone)]
struct FixedClock {
    now: std::sync::Arc<std::sync::Mutex<DateTime<Utc>>>,
}

impl FixedClock {
    fn new(initial: DateTime<Utc>) -> Self {
        Self {
            now: std::sync::Arc::new(std::sync::Mutex::new(initial)),
        }
    }

    fn advance(&self, delta: Duration) {
        let mut g = self.now.lock().unwrap();
        *g += delta;
    }
}

impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
}

fn ts(h: u32, m: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 22, h, m, s).unwrap()
}

fn storage() -> Arc<Storage> {
    Arc::new(Storage::open_in_memory().expect("open in-memory storage"))
}

fn insert_session(storage: &Storage, session: &Session) {
    storage
        .with_conn_mut(|conn| {
            conn.execute(
                "INSERT INTO sessions \
                 (id, agent_name, adapter_name, adapter_level, pid, command, cwd, repo_path, \
                  project_tag, status, status_confidence, attention_reason, start_time, \
                  last_seen_time, last_activity_time, estimated_cost, cost_kind, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
                params![
                    session.id.to_string(),
                    &session.agent_name,
                    &session.adapter_name,
                    session.adapter_level.as_u8() as i64,
                    session.pid.map(|p| p as i64),
                    &session.command,
                    session.cwd.as_deref(),
                    session.repo_path.as_deref(),
                    session.project_tag.as_deref(),
                    session.status.as_db_str(),
                    session.status_confidence.as_db_str(),
                    session.attention_reason.as_deref(),
                    format_ts(session.start_time),
                    format_ts(session.last_seen_time),
                    session.last_activity_time.map(format_ts),
                    session.estimated_cost,
                    session.cost_kind.as_db_str(),
                    format_ts(session.created_at),
                    format_ts(session.updated_at),
                ],
            )?;
            Ok(())
        })
        .expect("insert session row");
}

fn format_ts(t: DateTime<Utc>) -> String {
    t.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn sample_session(
    status: SessionStatus,
    level: CapabilityLevel,
    confidence: Confidence,
) -> Session {
    Session {
        id: Uuid::new_v4(),
        agent_name: "aider".into(),
        adapter_name: "aider".into(),
        adapter_level: level,
        pid: Some(42),
        command: "aider /repo".into(),
        cwd: Some("/repo".into()),
        repo_path: Some("/repo".into()),
        project_tag: None,
        status,
        status_confidence: confidence,
        attention_reason: None,
        start_time: ts(14, 0, 0),
        last_seen_time: ts(14, 0, 0),
        last_activity_time: None,
        estimated_cost: None,
        cost_kind: CostKind::Unknown,
        created_at: ts(14, 0, 0),
        updated_at: ts(14, 0, 0),
    }
}

fn empty_tick(at: DateTime<Utc>) -> TickReport {
    TickReport {
        snapshot_time: at,
        created: vec![],
        updated: vec![],
        completed: vec![],
        status_changes: vec![],
    }
}

#[test]
fn creates_one_item_per_session_reason_pair() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    let s = sample_session(
        SessionStatus::RateLimited,
        CapabilityLevel::Status,
        Confidence::Medium,
    );
    insert_session(&storage, &s);

    let report = engine
        .apply(&empty_tick(ts(14, 0, 0)), std::slice::from_ref(&s))
        .expect("first apply");
    assert_eq!(report.created.len(), 1);
    assert!(report.updated.is_empty());
    assert!(report.resolved.is_empty());
    assert!(report.skipped_muted.is_empty());

    let open = engine.list_open().expect("list open");
    assert_eq!(open.len(), 1);
    let entry = &open[0];
    assert_eq!(entry.session.id, s.id);
    assert_eq!(entry.item.reason, AttentionReason::RateLimit);
    assert_eq!(entry.item.severity, AttentionSeverity::Warn);
    assert!(entry
        .item
        .recommended_actions
        .contains(&RecommendedAction::OpenDashboard));
}

#[test]
fn idempotent_when_proposal_unchanged() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    let s = sample_session(
        SessionStatus::Errored,
        CapabilityLevel::Status,
        Confidence::Medium,
    );
    insert_session(&storage, &s);

    let r1 = engine
        .apply(&empty_tick(ts(14, 0, 0)), std::slice::from_ref(&s))
        .unwrap();
    assert_eq!(r1.created.len(), 1);

    clock.advance(Duration::seconds(30));
    let r2 = engine
        .apply(&empty_tick(ts(14, 0, 30)), std::slice::from_ref(&s))
        .unwrap();
    assert!(r2.created.is_empty());
    assert!(r2.updated.is_empty(), "no fields changed; no update");
    assert!(r2.resolved.is_empty());
}

#[test]
fn updates_when_severity_or_message_changes() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    // Errored at low confidence first.
    let mut s = sample_session(
        SessionStatus::Errored,
        CapabilityLevel::Status,
        Confidence::Low,
    );
    insert_session(&storage, &s);

    let r1 = engine
        .apply(&empty_tick(ts(14, 0, 0)), std::slice::from_ref(&s))
        .unwrap();
    assert_eq!(r1.created.len(), 1);
    let item_id = r1.created[0];

    // Same status, higher confidence -> the confidence field on the
    // attention item must update without re-creating the row.
    s.status_confidence = Confidence::High;
    clock.advance(Duration::seconds(1));
    let r2 = engine
        .apply(&empty_tick(ts(14, 0, 1)), std::slice::from_ref(&s))
        .unwrap();
    assert!(r2.created.is_empty());
    assert_eq!(r2.updated, vec![item_id]);
}

#[test]
fn resolves_when_rule_stops_firing() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    let mut s = sample_session(
        SessionStatus::Stalled,
        CapabilityLevel::Status,
        Confidence::Medium,
    );
    insert_session(&storage, &s);

    let r1 = engine
        .apply(&empty_tick(ts(14, 0, 0)), std::slice::from_ref(&s))
        .unwrap();
    assert_eq!(r1.created.len(), 1);
    let item_id = r1.created[0];

    // Session recovers -> rule no longer fires -> item resolves.
    s.status = SessionStatus::Running;
    clock.advance(Duration::seconds(5));
    let r2 = engine
        .apply(&empty_tick(ts(14, 0, 5)), std::slice::from_ref(&s))
        .unwrap();
    assert!(r2.created.is_empty());
    assert_eq!(r2.resolved, vec![item_id]);

    // Nothing left open.
    let open = engine.list_open().unwrap();
    assert!(open.is_empty());
}

#[test]
fn resolves_when_session_marked_completed_in_tick() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    let s = sample_session(
        SessionStatus::RateLimited,
        CapabilityLevel::Status,
        Confidence::Medium,
    );
    insert_session(&storage, &s);

    let r1 = engine
        .apply(&empty_tick(ts(14, 0, 0)), std::slice::from_ref(&s))
        .unwrap();
    let item_id = r1.created[0];

    // Tick reports the session completed. Even if we still pass it in
    // `sessions`, the engine must close the row.
    let tick = TickReport {
        snapshot_time: ts(14, 0, 1),
        created: vec![],
        updated: vec![],
        completed: vec![s.id],
        status_changes: vec![],
    };
    clock.advance(Duration::seconds(1));
    let r2 = engine.apply(&tick, std::slice::from_ref(&s)).unwrap();
    assert_eq!(r2.resolved, vec![item_id]);
}

#[test]
fn level_one_waiting_input_is_silently_dropped() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    let s = sample_session(
        SessionStatus::WaitingForInput,
        CapabilityLevel::Presence, // Level 1
        Confidence::Medium,
    );
    insert_session(&storage, &s);

    let r = engine
        .apply(&empty_tick(ts(14, 0, 0)), std::slice::from_ref(&s))
        .unwrap();
    assert!(
        r.created.is_empty(),
        "Level 1 adapters cannot emit waiting-for-input attention (build-plan §10)"
    );
    assert!(engine.list_open().unwrap().is_empty());
}

#[test]
fn muted_items_are_skipped_for_updates_until_unmuted() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    let mut s = sample_session(
        SessionStatus::Errored,
        CapabilityLevel::Status,
        Confidence::Low,
    );
    insert_session(&storage, &s);

    let r1 = engine
        .apply(&empty_tick(ts(14, 0, 0)), std::slice::from_ref(&s))
        .unwrap();
    let item_id = r1.created[0];

    // User mutes the item for 30 minutes.
    let mute_until = ts(14, 30, 0);
    let muted = engine.set_mute(item_id, Some(mute_until)).unwrap();
    assert_eq!(muted.muted_until, Some(mute_until));

    // Confidence climbs to High during the mute window. The engine
    // must not touch the item; report.skipped_muted must contain it.
    s.status_confidence = Confidence::High;
    clock.advance(Duration::minutes(5));
    let r2 = engine
        .apply(&empty_tick(ts(14, 5, 0)), std::slice::from_ref(&s))
        .unwrap();
    assert!(r2.updated.is_empty(), "muted items must not be updated");
    assert_eq!(r2.skipped_muted, vec![item_id]);

    // After mute expires, the next tick updates the item.
    clock.advance(Duration::hours(1));
    let r3 = engine
        .apply(&empty_tick(ts(15, 5, 0)), std::slice::from_ref(&s))
        .unwrap();
    assert_eq!(r3.updated, vec![item_id]);
}

#[test]
fn resolve_user_action_marks_resolved_at_and_rejects_double_resolve() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    let s = sample_session(
        SessionStatus::Errored,
        CapabilityLevel::Status,
        Confidence::Medium,
    );
    insert_session(&storage, &s);

    let r1 = engine
        .apply(&empty_tick(ts(14, 0, 0)), std::slice::from_ref(&s))
        .unwrap();
    let item_id = r1.created[0];

    let resolved = engine.resolve(item_id).unwrap();
    assert!(resolved.resolved_at.is_some());
    assert!(engine.list_open().unwrap().is_empty());

    let err = engine.resolve(item_id).unwrap_err();
    assert!(
        matches!(err, agentdeck_attention::AttentionError::AlreadyResolved(id) if id == item_id),
        "second resolve must report AlreadyResolved, got {err:?}"
    );
}

#[test]
fn resolve_unknown_id_returns_not_found() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());
    let err = engine.resolve(Uuid::new_v4()).unwrap_err();
    assert!(matches!(
        err,
        agentdeck_attention::AttentionError::NotFound(_)
    ));
}

#[test]
fn list_open_sorts_urgent_before_warn_before_info() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    // Stalled -> info
    let mut s1 = sample_session(
        SessionStatus::Stalled,
        CapabilityLevel::Status,
        Confidence::Medium,
    );
    s1.agent_name = "agent-stalled".into();
    insert_session(&storage, &s1);

    // RateLimited -> warn
    let mut s2 = sample_session(
        SessionStatus::RateLimited,
        CapabilityLevel::Status,
        Confidence::Medium,
    );
    s2.agent_name = "agent-rate".into();
    insert_session(&storage, &s2);

    // Both fire on first apply.
    engine
        .apply(&empty_tick(ts(14, 0, 0)), &[s1.clone(), s2.clone()])
        .unwrap();

    let open = engine.list_open().unwrap();
    assert_eq!(open.len(), 2);
    assert_eq!(open[0].item.severity, AttentionSeverity::Warn);
    assert_eq!(open[1].item.severity, AttentionSeverity::Info);
}

#[test]
fn engine_integrates_cleanly_with_session_state_machine() {
    // Smoke-test that the engine consumes the real `TickReport` produced
    // by the session state machine without surprises. We do not assert
    // on attention contents here — `SessionStatus::Running` produces no
    // proposals — only that the engine call succeeds end-to-end.
    let storage = storage();
    let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let state_machine = SessionStateMachine::new(storage.clone(), clock.clone());
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    let tick = state_machine.apply(ts(14, 0, 0), &[]).unwrap();
    let report = engine.apply(&tick, &[]).unwrap();
    assert!(report.created.is_empty());
    assert!(report.resolved.is_empty());
}

#[test]
fn set_mute_none_clears_existing_mute() {
    let storage = storage();
    let clock = Arc::new(FixedClock::new(ts(14, 0, 0)));
    let engine = AttentionEngine::new(storage.clone(), clock.clone());

    let s = sample_session(
        SessionStatus::Errored,
        CapabilityLevel::Status,
        Confidence::Medium,
    );
    insert_session(&storage, &s);

    let r1 = engine
        .apply(&empty_tick(ts(14, 0, 0)), std::slice::from_ref(&s))
        .unwrap();
    let item_id = r1.created[0];
    engine.set_mute(item_id, Some(ts(15, 0, 0))).unwrap();
    let cleared = engine.set_mute(item_id, None).unwrap();
    assert!(cleared.muted_until.is_none());
}
