//! Custom (user-defined) process adapter.
//!
//! Users teach AgentDeck about agents we do not ship native support for by
//! creating a "custom adapter" with a label, an agent name, and a regex
//! matched against one of: process name, joined command line, or current
//! working directory. The adapter is locked to capability **Level 1**
//! (presence detection only) per
//! [`planning/adapter-feasibility.md`](../../planning/adapter-feasibility.md)
//! §4.4.
//!
//! Module layout:
//!
//! - [`definition`] – data model + validation (regex compilation, allowed
//!   colour/hex shapes, length bounds).
//! - [`repository`] – CRUD against `agentdeck-storage`'s `custom_adapters`
//!   table. Used by the Tauri shell and by future monitor-core code.
//! - [`matcher`] – pure-function matcher over a `ProcessSnapshot`. Adapters
//!   are compiled once and re-used across scans.
//!
//! The trait that wraps all this for the future adapter framework lands in
//! a later build-plan step. This crate is intentionally interface-agnostic
//! until then.

pub mod definition;
pub mod matcher;
pub mod repository;
pub mod runtime;

pub use definition::{
    CustomAdapter, CustomAdapterError, MatchKind, NewCustomAdapter, MAX_LABEL_LEN, MAX_PATTERN_LEN,
};
pub use matcher::{group_by_adapter, match_snapshot, CompiledAdapter, CustomAdapterMatch};
pub use repository::CustomAdapterRepository;
pub use runtime::{CustomProcessAdapter, ADAPTER_NAME};
