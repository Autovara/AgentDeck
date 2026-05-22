//! Cross-platform process scanning for AgentDeck.
//!
//! The alpha keeps a strict separation between *sourcing* process data and
//! *deciding* what to do with it. This crate owns the former. It exposes:
//!
//! - [`SysinfoProcessSource`], a [`agentdeck_core::ProcessSource`] backed by
//!   the `sysinfo` crate, used in production builds.
//! - [`ProcessScanner`], which wraps any [`agentdeck_core::ProcessSource`]
//!   and adds wall-clock timestamping, scan-duration measurement, and an
//!   in-memory cache of the previous snapshot so callers can compute diffs.
//! - [`ProcessSnapshot`] / [`ProcessSnapshotDiff`] data types.
//! - A lightweight [`extract_candidates`] heuristic for surfacing
//!   possibly-interesting processes in the diagnostics UI. This is *not* the
//!   adapter framework; it is a temporary diagnostic that adapters will
//!   replace.
//!
//! Adapters must depend on [`agentdeck_core::ProcessSource`] (the trait), not
//! on the concrete [`SysinfoProcessSource`], so the harness can keep
//! substituting [`agentdeck_harness::MockProcessSource`] in tests.

mod candidates;
mod scanner;
mod snapshot;
mod sysinfo_source;

pub use candidates::{extract_candidates, AgentCandidate, ALPHA_AGENT_PATTERNS};
pub use scanner::{ProcessScanResult, ProcessScanner};
pub use snapshot::{ProcessChange, ProcessSnapshot, ProcessSnapshotDiff, SnapshotSource};
pub use sysinfo_source::SysinfoProcessSource;
