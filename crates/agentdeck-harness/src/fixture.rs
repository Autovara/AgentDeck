use std::path::Path;

use agentdeck_core::FileEventKind;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A captured session fixture used by the replay harness.
///
/// The on-disk format is documented in `planning/agent-harness-notes.md`
/// §3. The descriptor lives at `fixtures/sessions/<name>/fixture.json`;
/// content files referenced by `content_ref` live alongside it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fixture {
    pub name: String,
    pub agent: String,
    pub agent_version: String,
    pub captured_at: DateTime<Utc>,
    pub capture_os: String,
    pub duration_ms: u64,
    pub clock_start: DateTime<Utc>,
    #[serde(default)]
    pub processes: Vec<FixtureProcessSpec>,
    #[serde(default)]
    pub files: Vec<FixtureFileSpec>,
    #[serde(default)]
    pub expectations: Option<FixtureExpectations>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureProcessSpec {
    pub appears_at_ms: u64,
    pub disappears_at_ms: Option<u64>,
    pub pid: u32,
    pub parent_pid: Option<u32>,
    pub name: String,
    pub cmdline: Vec<String>,
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureFileSpec {
    pub path: String,
    #[serde(default)]
    pub events: Vec<FixtureFileEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureFileEvent {
    pub at_ms: u64,
    pub kind: FileEventKind,
    #[serde(default)]
    pub content_ref: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FixtureExpectations {
    #[serde(default)]
    pub final_status: Option<String>,
    #[serde(default)]
    pub min_attention_items: Option<u32>,
    #[serde(default)]
    pub must_emit_events: Vec<String>,
}

#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("could not read fixture {path}")]
    Read {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("could not parse fixture {path}")]
    Parse {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

impl Fixture {
    /// Load a fixture descriptor from disk. Content files referenced by
    /// `content_ref` are not eagerly loaded; callers resolve them
    /// relative to the descriptor's directory at replay time.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, FixtureError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| FixtureError::Read {
            path: path.display().to_string(),
            source,
        })?;
        let fixture: Fixture =
            serde_json::from_str(&text).map_err(|source| FixtureError::Parse {
                path: path.display().to_string(),
                source,
            })?;
        Ok(fixture)
    }
}
