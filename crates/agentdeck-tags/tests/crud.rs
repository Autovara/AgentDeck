//! Integration tests for the tag CRUD helpers + session tag assignment.

use std::sync::Arc;

use agentdeck_storage::Storage;
use agentdeck_tags::{
    assign_session_tag, clear_session_tag, create, delete, get, list, NewProjectTag, TagError,
};
use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

fn open() -> Arc<Storage> {
    Arc::new(Storage::open_in_memory().expect("in-memory db"))
}

fn insert_session(storage: &Storage, id: Uuid, status: &str) {
    let ts = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    storage
        .with_conn(|conn| {
            conn.execute(
                "INSERT INTO sessions (id, agent_name, adapter_name, adapter_level, command, \
                    status, status_confidence, start_time, last_seen_time, created_at, updated_at) \
                 VALUES (?1, 'aider', 'aider', 1, 'aider', ?2, 'medium', ?3, ?3, ?3, ?3)",
                params![id.to_string(), status, ts],
            )
        })
        .unwrap();
}

fn session_tag(storage: &Storage, id: Uuid) -> Option<String> {
    storage
        .with_conn(|conn| {
            conn.query_row(
                "SELECT project_tag FROM sessions WHERE id = ?1",
                [id.to_string()],
                |r| r.get::<_, Option<String>>(0),
            )
        })
        .unwrap()
}

#[test]
fn create_then_list_returns_inserted_tag() {
    let storage = open();
    let tag = create(
        &storage,
        NewProjectTag {
            name: "billing".into(),
            color: Some("#4c6ef5".into()),
            notes: Some("billing API project".into()),
        },
    )
    .expect("create");
    assert_eq!(tag.name, "billing");

    let all = list(&storage).expect("list");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, tag.id);
    assert_eq!(all[0].color.as_deref(), Some("#4c6ef5"));
}

#[test]
fn duplicate_name_returns_duplicate_error() {
    let storage = open();
    create(
        &storage,
        NewProjectTag {
            name: "dup".into(),
            color: None,
            notes: None,
        },
    )
    .expect("first");
    let err = create(
        &storage,
        NewProjectTag {
            name: "dup".into(),
            color: None,
            notes: None,
        },
    )
    .expect_err("second");
    assert!(matches!(err, TagError::DuplicateName(s) if s == "dup"));
}

#[test]
fn delete_removes_row_and_clears_session_label() {
    let storage = open();
    let session_id = Uuid::new_v4();
    insert_session(&storage, session_id, "running");
    let tag = create(
        &storage,
        NewProjectTag {
            name: "billing".into(),
            color: None,
            notes: None,
        },
    )
    .expect("create");
    assign_session_tag(&storage, session_id, &tag.name).expect("assign");
    assert_eq!(session_tag(&storage, session_id).as_deref(), Some("billing"));

    delete(&storage, tag.id).expect("delete");
    assert!(get(&storage, tag.id).unwrap().is_none());
    assert!(
        session_tag(&storage, session_id).is_none(),
        "deleting a tag must clear it from any session that referenced it"
    );
}

#[test]
fn delete_unknown_tag_errors() {
    let storage = open();
    let bogus = Uuid::new_v4();
    let err = delete(&storage, bogus).expect_err("delete");
    assert!(matches!(err, TagError::NotFound(id) if id == bogus));
}

#[test]
fn assign_unknown_tag_errors() {
    let storage = open();
    let session_id = Uuid::new_v4();
    insert_session(&storage, session_id, "running");
    let err = assign_session_tag(&storage, session_id, "nope").expect_err("assign");
    assert!(matches!(err, TagError::UnknownTagName(s) if s == "nope"));
}

#[test]
fn assign_unknown_session_errors() {
    let storage = open();
    create(
        &storage,
        NewProjectTag {
            name: "billing".into(),
            color: None,
            notes: None,
        },
    )
    .expect("create");
    let bogus = Uuid::new_v4();
    let err = assign_session_tag(&storage, bogus, "billing").expect_err("assign");
    assert!(matches!(err, TagError::UnknownSession(id) if id == bogus));
}

#[test]
fn clear_session_tag_sets_null() {
    let storage = open();
    let session_id = Uuid::new_v4();
    insert_session(&storage, session_id, "running");
    let tag = create(
        &storage,
        NewProjectTag {
            name: "billing".into(),
            color: None,
            notes: None,
        },
    )
    .expect("create");
    assign_session_tag(&storage, session_id, &tag.name).expect("assign");
    clear_session_tag(&storage, session_id).expect("clear");
    assert!(session_tag(&storage, session_id).is_none());
}
