//! Core traits and types for AgentDeck.
//!
//! This crate defines the OS-facing abstractions that the monitor core,
//! adapters, and the dev-time replay harness all depend on. Adapters
//! interact with these traits only; they never call OS APIs directly.
//! That separation is what makes the harness possible: in production the
//! traits are backed by `sysinfo`, `notify`, and the system clock; in
//! tests they are backed by deterministic mocks driven from recorded
//! fixtures.
//!
//! See `planning/agent-harness-notes.md` for the architecture.

pub mod clock;
pub mod error;
pub mod file;
pub mod process;

pub use clock::{Clock, SystemClock};
pub use error::CoreError;
pub use file::{FileEvent, FileEventKind, FileWatcher};
pub use process::{ProcessInfo, ProcessSource};
