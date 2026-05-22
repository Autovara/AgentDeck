//! Adapter trait and registry shared by every concrete AgentDeck adapter.
//!
//! Concrete adapter crates (`agentdeck-adapter-custom`, and later
//! `agentdeck-adapter-aider`, `agentdeck-adapter-codex`,
//! `agentdeck-adapter-claude-code`) implement [`Adapter`]. The monitor core
//! drives them through an [`AdapterRegistry`]: on every scan tick the
//! registry hands the same [`agentdeck_process::ProcessSnapshot`] to every
//! enabled adapter and collects each adapter's matches plus a per-adapter
//! diagnostic.
//!
//! The alpha trait is intentionally thin:
//!
//! - It is synchronous. The monitor core wraps the whole tick in
//!   `tokio::task::spawn_blocking`.
//! - It carries no termination / stop / approve methods. Those land with
//!   the per-OS `/stop` dispatch in a later build-plan step.
//! - It persists nothing. Adapters return matches plus a fresh
//!   [`AdapterDiagnostic`] on every scan; the monitor core owns the
//!   database (`planning/alpha-build-plan.md` §4.5).
//!
//! See `planning/alpha-build-plan.md` §15 step 5 ("Adapter interface") for
//! the role of this crate in the build order.

mod adapter;
mod diagnostics;
mod registry;
mod types;

pub use adapter::{Adapter, AdapterMatch, AdapterScanResult};
pub use diagnostics::AdapterDiagnostic;
pub use registry::{AdapterRegistry, RegistryScanResult};
pub use types::{CapabilityLevel, Confidence, SessionStatus};
