//! Tauri-facing bridge for the project-tag CRUD + per-session
//! assignment commands.

use std::sync::Arc;

use agentdeck_storage::Storage;
use agentdeck_tags::{self, NewProjectTag, ProjectTag, TagError};
use chrono::{DateTime, Utc};
use serde::Serialize;
use uuid::Uuid;

/// Dashboard payload for the Settings → Project tags card.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTagsReport {
    pub ready: bool,
    pub tags: Vec<ProjectTag>,
    pub captured_at: DateTime<Utc>,
    pub error: Option<String>,
}

impl ProjectTagsReport {
    fn unavailable(message: impl Into<String>) -> Self {
        Self {
            ready: false,
            tags: Vec::new(),
            captured_at: Utc::now(),
            error: Some(message.into()),
        }
    }
}

pub struct ProjectTagsState {
    storage: Option<Arc<Storage>>,
}

impl ProjectTagsState {
    pub fn new(storage: Option<Arc<Storage>>) -> Self {
        Self { storage }
    }
}

pub fn snapshot(state: &ProjectTagsState) -> ProjectTagsReport {
    let Some(storage) = state.storage.as_ref() else {
        return ProjectTagsReport::unavailable("storage unavailable");
    };
    match agentdeck_tags::list(storage) {
        Ok(tags) => ProjectTagsReport {
            ready: true,
            tags,
            captured_at: Utc::now(),
            error: None,
        },
        Err(err) => {
            tracing::warn!(error = %err, "tags: list failed");
            ProjectTagsReport::unavailable(format!("read failed: {err}"))
        }
    }
}

pub fn create_tag(
    state: &ProjectTagsState,
    input: NewProjectTag,
) -> Result<ProjectTagsReport, String> {
    let storage = state.storage.as_ref().ok_or("storage unavailable")?;
    agentdeck_tags::create(storage, input).map_err(stringify)?;
    Ok(snapshot(state))
}

pub fn delete_tag(
    state: &ProjectTagsState,
    id: Uuid,
) -> Result<ProjectTagsReport, String> {
    let storage = state.storage.as_ref().ok_or("storage unavailable")?;
    agentdeck_tags::delete(storage, id).map_err(stringify)?;
    Ok(snapshot(state))
}

pub fn assign_session_tag(
    state: &ProjectTagsState,
    session_id: Uuid,
    tag_name: &str,
) -> Result<(), String> {
    let storage = state.storage.as_ref().ok_or("storage unavailable")?;
    agentdeck_tags::assign_session_tag(storage, session_id, tag_name).map_err(stringify)
}

pub fn clear_session_tag(
    state: &ProjectTagsState,
    session_id: Uuid,
) -> Result<(), String> {
    let storage = state.storage.as_ref().ok_or("storage unavailable")?;
    agentdeck_tags::clear_session_tag(storage, session_id).map_err(stringify)
}

fn stringify(err: TagError) -> String {
    err.to_string()
}
