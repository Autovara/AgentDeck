//! Dev-time replay harness for AgentDeck.
//!
//! The harness exists so adapters can be developed and tested without
//! running real Claude Code, Codex CLI, or Aider sessions. It supplies
//! mock implementations of [`agentdeck_core::ProcessSource`],
//! [`agentdeck_core::FileWatcher`], and [`agentdeck_core::Clock`] that
//! are driven from JSON fixtures captured from real runs.
//!
//! See `planning/agent-harness-notes.md` for the architecture, fixture
//! format, and redaction rules.

pub mod fixture;
pub mod mock_clock;
pub mod mock_file;
pub mod mock_process;
pub mod redact;

pub use fixture::{
    Fixture, FixtureError, FixtureExpectations, FixtureFileEvent, FixtureFileSpec,
    FixtureProcessSpec,
};
pub use mock_clock::MockClock;
pub use mock_file::MockFileWatcher;
pub use mock_process::MockProcessSource;
pub use redact::{redact_text, Redactor};
