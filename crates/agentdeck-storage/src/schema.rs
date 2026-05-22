//! Schema migrations.
//!
//! Migrations are static SQL files in `migrations/`. Each migration is given a
//! human-readable label here; the label is surfaced through
//! [`MigrationSummary`] and shown on the storage diagnostics card.
//!
//! ## Adding a migration
//!
//! 1. Add a new file `migrations/NNNN_short_name.sql` (numbered strictly
//!    upward; never reuse a number).
//! 2. Append a new [`Step`] entry to [`STEPS`] in the order it should run.
//! 3. Add a regression test in `tests/migrate.rs` that exercises the new
//!    behaviour.
//!
//! Once a migration has shipped to a user, it is **immutable**. New schema
//! changes must always land in a new file.

use rusqlite_migration::{Migrations, M};
use serde::Serialize;

/// Display metadata for a single shipped migration.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationSummary {
    /// 1-based version; matches `PRAGMA user_version` after this step.
    pub version: u32,
    /// Human-readable label for the migration, suitable for UI display.
    pub label: &'static str,
}

struct Step {
    label: &'static str,
    up_sql: &'static str,
}

const STEPS: &[Step] = &[
    Step {
        label: "Initial schema (sessions, events, attention, diagnostics, usage, tags, remote commands, audit, settings)",
        up_sql: include_str!("../migrations/0001_initial.sql"),
    },
    Step {
        label: "Custom adapters (user-defined Level 1 process matchers)",
        up_sql: include_str!("../migrations/0002_custom_adapters.sql"),
    },
];

/// Build the [`Migrations`] manifest for the current binary.
pub(crate) fn build() -> Migrations<'static> {
    Migrations::new(STEPS.iter().map(|s| M::up(s.up_sql)).collect())
}

/// Human-readable list of every migration that ships with this binary.
pub fn applied_migrations() -> Vec<MigrationSummary> {
    STEPS
        .iter()
        .enumerate()
        .map(|(idx, step)| MigrationSummary {
            version: (idx + 1) as u32,
            label: step.label,
        })
        .collect()
}

/// The schema version a freshly-migrated database should report.
pub fn current_schema_version() -> u32 {
    STEPS.len() as u32
}
