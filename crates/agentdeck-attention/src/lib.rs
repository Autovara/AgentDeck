//! Attention engine for AgentDeck.
//!
//! This crate maps session-state observations to durable
//! [`AttentionItem`](model::AttentionItem) rows in the `attention_items`
//! table. The engine is intentionally narrow:
//!
//! - It does not own scheduling. The monitor core (currently the Tauri
//!   shell) calls [`engine::AttentionEngine::apply`] once per tick, after
//!   the [`agentdeck_session::SessionStateMachine`] has finished writing
//!   the new state of `sessions`.
//! - It does not own classification. Each rule in [`rules`] is a pure
//!   function over a [`agentdeck_session::Session`]; rules return at most
//!   one [`model::ProposedAttention`] per session.
//! - It does respect `muted_until`. If a user mutes an open item, the
//!   engine stops updating that row (still resolves it when the session
//!   completes or the rule stops firing). Mute is a *user-driven* action;
//!   the engine never sets `muted_until` automatically.
//!
//! The alpha scope is the subset of `planning/alpha-build-plan.md` §10
//! reasons we can derive today without a Level 2+ adapter; rules for the
//! other reasons (approval/auth/context/crashed/budget) will land with
//! their respective adapters.

pub mod action;
pub mod engine;
pub mod error;
pub mod model;
pub mod reason;
mod repository;
pub mod rules;
pub mod severity;

pub use action::RecommendedAction;
pub use engine::{AttentionEngine, AttentionTickReport, OpenAttentionEntry};
pub use error::AttentionError;
pub use model::{AttentionItem, ProposedAttention};
pub use reason::AttentionReason;
pub use rules::propose_from_session;
pub use severity::AttentionSeverity;
