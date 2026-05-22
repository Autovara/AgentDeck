//! Integration tests for the `CustomAdapterRepository`.
//!
//! Each test opens a fresh in-memory database with the full migration set,
//! exercises one slice of CRUD behaviour, and asserts both the in-memory
//! result and the resulting row count.

use agentdeck_adapter_custom::{
    CustomAdapterError, CustomAdapterRepository, MatchKind, NewCustomAdapter,
};
use agentdeck_core::Clock;
use agentdeck_storage::Storage;
use chrono::{DateTime, TimeZone, Utc};

struct FixedClock(DateTime<Utc>);
impl Clock for FixedClock {
    fn now(&self) -> DateTime<Utc> {
        self.0
    }
}

fn new_adapter(label: &str) -> NewCustomAdapter {
    NewCustomAdapter {
        label: label.into(),
        agent_name: label.into(),
        match_kind: MatchKind::Name,
        pattern: label.into(),
        enabled: true,
        color: Some("#4c6ef5".into()),
        cost_per_hour_cents: None,
        notes: None,
    }
}

fn open() -> (Storage, FixedClock) {
    (
        Storage::open_in_memory().expect("in-memory db"),
        FixedClock(Utc.with_ymd_and_hms(2026, 5, 22, 13, 0, 0).unwrap()),
    )
}

#[test]
fn add_then_list_returns_inserted_row() {
    let (storage, clock) = open();
    let repo = CustomAdapterRepository::new(&storage);

    let validated = new_adapter("aider-dev").validate().expect("validates");
    let added = repo.add(validated, &clock).expect("adds");
    assert_eq!(added.label, "aider-dev");
    assert!(added.enabled);
    assert_eq!(added.match_kind, MatchKind::Name);

    let list = repo.list().expect("list");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].id, added.id);
}

#[test]
fn add_duplicate_label_returns_duplicate_error() {
    let (storage, clock) = open();
    let repo = CustomAdapterRepository::new(&storage);

    let first = new_adapter("dup").validate().expect("validates");
    repo.add(first, &clock).expect("first insert");

    let second = new_adapter("dup").validate().expect("validates");
    let err = repo.add(second, &clock).expect_err("second must fail");
    assert!(matches!(err, CustomAdapterError::DuplicateLabel(ref s) if s == "dup"));
}

#[test]
fn delete_unknown_id_returns_not_found() {
    let (storage, _clock) = open();
    let repo = CustomAdapterRepository::new(&storage);
    let bogus = uuid::Uuid::new_v4();
    let err = repo.delete(bogus).expect_err("delete must fail");
    assert!(matches!(err, CustomAdapterError::NotFound(id) if id == bogus));
}

#[test]
fn delete_removes_existing_row() {
    let (storage, clock) = open();
    let repo = CustomAdapterRepository::new(&storage);
    let added = repo
        .add(new_adapter("kill-me").validate().unwrap(), &clock)
        .expect("add");
    repo.delete(added.id).expect("delete");
    assert!(repo.list().expect("list").is_empty());
}

#[test]
fn set_enabled_toggles_flag() {
    let (storage, clock) = open();
    let repo = CustomAdapterRepository::new(&storage);
    let added = repo
        .add(new_adapter("toggle").validate().unwrap(), &clock)
        .expect("add");
    assert!(added.enabled);

    repo.set_enabled(added.id, false, &clock).expect("disable");
    let after = repo.get(added.id).expect("get");
    assert!(!after.enabled);
    assert!(after.updated_at >= added.updated_at);

    repo.set_enabled(added.id, true, &clock).expect("enable");
    assert!(repo.get(added.id).expect("get").enabled);
}

#[test]
fn list_enabled_filters_out_disabled() {
    let (storage, clock) = open();
    let repo = CustomAdapterRepository::new(&storage);
    let a = repo
        .add(new_adapter("on").validate().unwrap(), &clock)
        .expect("add a");
    let b = repo
        .add(new_adapter("off").validate().unwrap(), &clock)
        .expect("add b");
    repo.set_enabled(b.id, false, &clock).expect("disable b");

    let enabled = repo.list_enabled().expect("list enabled");
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].id, a.id);

    // `list` still returns everyone.
    assert_eq!(repo.list().expect("list").len(), 2);
}

#[test]
fn round_trip_preserves_optional_fields() {
    let (storage, clock) = open();
    let repo = CustomAdapterRepository::new(&storage);
    let mut input = new_adapter("with-notes");
    input.notes = Some("private notes".into());
    input.cost_per_hour_cents = Some(42_50); // $0.42/hr
    input.color = Some("#abc".into());
    input.match_kind = MatchKind::Cmdline;
    input.pattern = "my-bot".into();
    let added = repo.add(input.validate().unwrap(), &clock).expect("add");

    let fetched = repo.get(added.id).expect("get");
    assert_eq!(fetched.notes.as_deref(), Some("private notes"));
    assert_eq!(fetched.cost_per_hour_cents, Some(4250));
    assert_eq!(fetched.color.as_deref(), Some("#abc"));
    assert_eq!(fetched.match_kind, MatchKind::Cmdline);
    assert_eq!(fetched.pattern, "my-bot");
}

#[test]
fn list_orders_enabled_first_then_by_created_at() {
    let (storage, _) = open();
    let repo = CustomAdapterRepository::new(&storage);

    // Insert disabled-first, then enabled-second. Use distinct clocks so
    // created_at differs.
    let earlier = FixedClock(Utc.with_ymd_and_hms(2026, 5, 22, 12, 0, 0).unwrap());
    let later = FixedClock(Utc.with_ymd_and_hms(2026, 5, 22, 12, 0, 1).unwrap());
    let disabled = {
        let mut input = new_adapter("disabled-first");
        input.enabled = false;
        repo.add(input.validate().unwrap(), &earlier).expect("add")
    };
    let enabled = repo
        .add(new_adapter("enabled-later").validate().unwrap(), &later)
        .expect("add");

    let list = repo.list().expect("list");
    assert_eq!(list[0].id, enabled.id, "enabled rows come first");
    assert_eq!(list[1].id, disabled.id);
}
