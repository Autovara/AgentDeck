//! Integration tests for [`agentdeck_session::SessionStateMachine`].
//!
//! Each test opens a fresh in-memory database, runs a synthetic match
//! sequence through the state machine, and asserts both the returned
//! [`TickReport`] and the resulting `sessions` + `session_events` row
//! state.

use std::sync::{Arc, Mutex};

use agentdeck_adapter::{AdapterMatch, CapabilityLevel, Confidence, SessionStatus};
use agentdeck_core::Clock;
use agentdeck_session::{
    SessionStateMachine, EVENT_KIND_DETECTED, EVENT_KIND_PROCESS_EXITED, EVENT_KIND_STATUS_CHANGED,
};
use agentdeck_storage::Storage;
use chrono::{DateTime, Duration, TimeZone, Utc};

/// Test clock that returns its current value and advances on demand.
struct StepClock {
    inner: Mutex<DateTime<Utc>>,
}

impl StepClock {
    fn new(start: DateTime<Utc>) -> Self {
        Self {
            inner: Mutex::new(start),
        }
    }

    fn advance(&self, secs: i64) {
        let mut guard = self.inner.lock().unwrap();
        *guard += Duration::seconds(secs);
    }
}

impl Clock for StepClock {
    fn now(&self) -> DateTime<Utc> {
        *self.inner.lock().unwrap()
    }
}

fn t(s: i64) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 5, 22, 13, 0, 0).unwrap() + Duration::seconds(s)
}

fn make_match(
    adapter: &str,
    pid: u32,
    agent: &str,
    status: SessionStatus,
    snapshot: DateTime<Utc>,
) -> AdapterMatch {
    AdapterMatch {
        adapter_name: adapter.into(),
        agent_name: agent.into(),
        capability_level: CapabilityLevel::Presence,
        pid,
        command: format!("{agent} /repo"),
        cwd: Some("/repo".into()),
        repo_path: Some("/repo".into()),
        status,
        status_confidence: Confidence::Medium,
        status_source: "process-list".into(),
        cost_per_hour_cents: None,
        observed_at: snapshot,
    }
}

fn setup() -> (Arc<Storage>, Arc<StepClock>, SessionStateMachine) {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory db"));
    let clock = Arc::new(StepClock::new(t(0)));
    let sm = SessionStateMachine::new(storage.clone(), clock.clone() as Arc<dyn Clock>);
    (storage, clock, sm)
}

fn count_sessions(storage: &Storage) -> i64 {
    storage
        .with_conn(|c| c.query_row("SELECT count(*) FROM sessions", [], |row| row.get(0)))
        .unwrap()
}

fn count_session_events(storage: &Storage, kind: Option<&str>) -> i64 {
    storage
        .with_conn(|c| match kind {
            Some(k) => c.query_row(
                "SELECT count(*) FROM session_events WHERE event_kind = ?1",
                [k],
                |row| row.get(0),
            ),
            None => c.query_row("SELECT count(*) FROM session_events", [], |row| row.get(0)),
        })
        .unwrap()
}

fn read_status(storage: &Storage, id: uuid::Uuid) -> String {
    storage
        .with_conn(|c| {
            c.query_row(
                "SELECT status FROM sessions WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
        })
        .unwrap()
}

fn read_last_seen(storage: &Storage, id: uuid::Uuid) -> String {
    storage
        .with_conn(|c| {
            c.query_row(
                "SELECT last_seen_time FROM sessions WHERE id = ?1",
                [id.to_string()],
                |row| row.get::<_, String>(0),
            )
        })
        .unwrap()
}

#[test]
fn empty_matches_produce_empty_report() {
    let (storage, _clock, sm) = setup();
    let report = sm.apply(t(0), &[]).expect("apply");
    assert_eq!(report.snapshot_time, t(0));
    assert!(report.created.is_empty());
    assert!(report.updated.is_empty());
    assert!(report.completed.is_empty());
    assert!(report.status_changes.is_empty());
    assert_eq!(count_sessions(&storage), 0);
}

#[test]
fn first_tick_creates_session_and_detected_event() {
    let (storage, _clock, sm) = setup();
    let m = make_match("custom", 100, "aider", SessionStatus::Running, t(0));
    let report = sm.apply(t(0), &[m]).expect("apply");

    assert_eq!(report.created.len(), 1);
    assert!(report.updated.is_empty());
    assert!(report.completed.is_empty());
    assert!(report.status_changes.is_empty());

    let session_id = report.created[0];
    assert_eq!(count_sessions(&storage), 1);
    assert_eq!(read_status(&storage, session_id), "running");
    assert_eq!(count_session_events(&storage, Some(EVENT_KIND_DETECTED)), 1);
    assert_eq!(count_session_events(&storage, None), 1);
}

#[test]
fn stable_tick_updates_last_seen_only() {
    let (storage, clock, sm) = setup();

    let m1 = make_match("custom", 100, "aider", SessionStatus::Running, t(0));
    let first = sm.apply(t(0), &[m1]).expect("apply first");
    let id = first.created[0];
    let first_seen = read_last_seen(&storage, id);

    clock.advance(30);
    let m2 = make_match("custom", 100, "aider", SessionStatus::Running, t(30));
    let report = sm.apply(t(30), &[m2]).expect("apply second");

    assert!(report.created.is_empty());
    assert_eq!(report.updated, vec![id]);
    assert!(report.completed.is_empty());
    assert!(report.status_changes.is_empty());

    let second_seen = read_last_seen(&storage, id);
    assert_ne!(first_seen, second_seen, "last_seen_time must advance");
    // No new events written for a stable tick.
    assert_eq!(count_session_events(&storage, None), 1);
}

#[test]
fn missing_match_marks_session_completed_with_event() {
    let (storage, clock, sm) = setup();
    let m = make_match("custom", 100, "aider", SessionStatus::Running, t(0));
    let first = sm.apply(t(0), &[m]).expect("apply first");
    let id = first.created[0];

    clock.advance(60);
    let report = sm.apply(t(60), &[]).expect("apply empty");

    assert!(report.created.is_empty());
    assert!(report.updated.is_empty());
    assert_eq!(report.completed, vec![id]);
    assert!(report.status_changes.is_empty());

    assert_eq!(read_status(&storage, id), "completed");
    assert_eq!(
        count_session_events(&storage, Some(EVENT_KIND_PROCESS_EXITED)),
        1
    );
}

#[test]
fn status_change_emits_status_changed_event() {
    let (storage, clock, sm) = setup();
    let m1 = make_match("custom", 100, "aider", SessionStatus::Running, t(0));
    let first = sm.apply(t(0), &[m1]).expect("apply first");
    let id = first.created[0];

    clock.advance(30);
    let m2 = make_match(
        "custom",
        100,
        "aider",
        SessionStatus::WaitingForInput,
        t(30),
    );
    let report = sm.apply(t(30), &[m2]).expect("apply second");

    assert_eq!(report.updated, vec![id]);
    assert_eq!(report.status_changes.len(), 1);
    let change = &report.status_changes[0];
    assert_eq!(change.session_id, id);
    assert_eq!(change.from, SessionStatus::Running);
    assert_eq!(change.to, SessionStatus::WaitingForInput);

    assert_eq!(read_status(&storage, id), "waiting_for_input");
    assert_eq!(
        count_session_events(&storage, Some(EVENT_KIND_STATUS_CHANGED)),
        1
    );
}

#[test]
fn duplicate_matches_collapse_to_one_session() {
    let (storage, _clock, sm) = setup();
    let m1 = make_match("custom", 100, "rule-a", SessionStatus::Running, t(0));
    let m2 = make_match("custom", 100, "rule-b", SessionStatus::Running, t(0));
    let report = sm.apply(t(0), &[m1, m2]).expect("apply");

    assert_eq!(report.created.len(), 1, "one session per (adapter, pid)");
    assert_eq!(count_sessions(&storage), 1);

    // Last-writer-wins: rule-b's agent_name is what stuck.
    let agent_name: String = storage
        .with_conn(|c| {
            c.query_row(
                "SELECT agent_name FROM sessions WHERE id = ?1",
                [report.created[0].to_string()],
                |row| row.get(0),
            )
        })
        .unwrap();
    assert_eq!(agent_name, "rule-b");
}

#[test]
fn pid_reuse_after_completion_creates_a_new_session() {
    let (storage, clock, sm) = setup();

    let m1 = make_match("custom", 100, "aider", SessionStatus::Running, t(0));
    let first = sm.apply(t(0), &[m1]).expect("apply first");
    let first_id = first.created[0];

    clock.advance(30);
    sm.apply(t(30), &[]).expect("complete first");
    assert_eq!(read_status(&storage, first_id), "completed");

    clock.advance(30);
    let m2 = make_match("custom", 100, "aider-take-2", SessionStatus::Running, t(60));
    let report = sm.apply(t(60), &[m2]).expect("apply second");

    assert_eq!(report.created.len(), 1);
    assert_ne!(
        report.created[0], first_id,
        "PID reuse must allocate a brand new session id"
    );
    assert_eq!(count_sessions(&storage), 2);
    assert_eq!(read_status(&storage, first_id), "completed");
    assert_eq!(read_status(&storage, report.created[0]), "running");
}

#[test]
fn mixed_tick_creates_updates_completes_in_one_pass() {
    let (storage, clock, sm) = setup();

    // First tick: two sessions, A (pid=10) and B (pid=20).
    let a1 = make_match("custom", 10, "agent-a", SessionStatus::Running, t(0));
    let b1 = make_match("custom", 20, "agent-b", SessionStatus::Running, t(0));
    let first = sm.apply(t(0), &[a1, b1]).expect("apply first");
    assert_eq!(first.created.len(), 2);

    clock.advance(30);
    // Second tick: A still here (status changes to waiting), B gone, new C (pid=30).
    let a2 = make_match(
        "custom",
        10,
        "agent-a",
        SessionStatus::WaitingForInput,
        t(30),
    );
    let c1 = make_match("custom", 30, "agent-c", SessionStatus::Running, t(30));
    let report = sm.apply(t(30), &[a2, c1]).expect("apply second");

    assert_eq!(report.created.len(), 1, "exactly one new session (C)");
    assert_eq!(report.updated.len(), 1, "exactly one updated session (A)");
    assert_eq!(report.completed.len(), 1, "exactly one completed (B)");
    assert_eq!(report.status_changes.len(), 1, "A changed status");

    assert_eq!(count_sessions(&storage), 3);
}

#[test]
fn different_adapters_with_same_pid_get_separate_sessions() {
    let (storage, _clock, sm) = setup();

    // Two adapters happen to match the same OS pid (unusual but valid).
    let custom = make_match("custom", 100, "custom-agent", SessionStatus::Running, t(0));
    let aider = make_match("aider", 100, "aider", SessionStatus::Running, t(0));
    let report = sm.apply(t(0), &[custom, aider]).expect("apply");

    assert_eq!(report.created.len(), 2);
    assert_eq!(count_sessions(&storage), 2);
}

#[test]
fn no_event_written_for_stable_status() {
    let (storage, clock, sm) = setup();
    let m1 = make_match("custom", 100, "aider", SessionStatus::Running, t(0));
    sm.apply(t(0), &[m1]).expect("apply first");
    assert_eq!(count_session_events(&storage, None), 1);

    clock.advance(30);
    let m2 = make_match("custom", 100, "aider", SessionStatus::Running, t(30));
    sm.apply(t(30), &[m2]).expect("apply second");

    // Still just the original `detected` event.
    assert_eq!(count_session_events(&storage, None), 1);
    assert_eq!(count_session_events(&storage, Some(EVENT_KIND_DETECTED)), 1);
}

#[test]
fn completion_sets_confidence_to_high() {
    let (storage, clock, sm) = setup();
    let m = make_match("custom", 100, "aider", SessionStatus::Running, t(0));
    let first = sm.apply(t(0), &[m]).expect("apply first");
    let id = first.created[0];

    clock.advance(30);
    sm.apply(t(30), &[]).expect("complete");

    let conf: String = storage
        .with_conn(|c| {
            c.query_row(
                "SELECT status_confidence FROM sessions WHERE id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
        })
        .unwrap();
    assert_eq!(conf, "high");
}

#[test]
fn snapshot_time_is_carried_through_to_report() {
    let (_storage, _clock, sm) = setup();
    let m = make_match("custom", 100, "aider", SessionStatus::Running, t(42));
    let report = sm.apply(t(42), &[m]).expect("apply");
    assert_eq!(report.snapshot_time, t(42));
}
