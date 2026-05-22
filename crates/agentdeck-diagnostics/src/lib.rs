//! Adapter diagnostics repository for AgentDeck.
//!
//! The adapter registry already emits one [`agentdeck_adapter::AdapterDiagnostic`]
//! per registered adapter at the end of every scan. This crate is the
//! durability layer for those rows: it upserts each tick's diagnostics
//! into the `adapter_diagnostics` table (one row per `adapter_name`, per
//! the §17 "keep only the most recent scan per adapter" discipline) and
//! reads them back for the dashboard's Diagnostics page.
//!
//! Why a dedicated crate?
//!
//! - The `AdapterDiagnostic` model lives in `agentdeck-adapter`, which is
//!   intentionally a thin trait crate with no SQL dependency. Putting the
//!   `rusqlite` glue there would bind every consumer of the adapter trait
//!   to SQLite.
//! - The storage crate (`agentdeck-storage`) owns *generic* storage
//!   concerns (schema, migrations, the `Storage` handle, storage-health
//!   diagnostics). Adapter-domain SQL belongs next to its model.
//! - Putting it in its own crate matches the pattern already established
//!   by `agentdeck-session` and `agentdeck-attention`, both of which own
//!   exactly one table.
//!
//! Scope intentionally narrow:
//!
//! - No history. The schema only ever holds the most recent row per
//!   `adapter_name`; this crate enforces that by using `INSERT OR REPLACE`.
//! - No business logic. The orchestrator (currently the Tauri shell)
//!   decides *when* to call [`record`]; this crate just persists what it
//!   is given.

mod error;
mod repository;

use std::sync::Arc;

use agentdeck_adapter::AdapterDiagnostic;
use agentdeck_storage::Storage;

pub use error::DiagnosticsError;

/// Persist a fresh batch of adapter diagnostics.
///
/// One call per monitor tick. The whole batch runs inside a single
/// SQLite transaction so an interrupted tick never leaves the
/// `adapter_diagnostics` table in a half-updated state. Each row is
/// upserted by `adapter_name` (the PK on the SQL table), so previously
/// recorded rows for adapters that were *not* in this batch are kept
/// — disabling an adapter for one tick should not erase its last good
/// diagnostic.
pub fn record(
    storage: &Arc<Storage>,
    diagnostics: &[AdapterDiagnostic],
) -> Result<(), DiagnosticsError> {
    if diagnostics.is_empty() {
        return Ok(());
    }
    storage
        .with_conn_mut(|conn| {
            let tx = conn.transaction()?;
            for d in diagnostics {
                repository::upsert(&tx, d)?;
            }
            tx.commit()?;
            Ok::<_, rusqlite::Error>(())
        })
        .map_err(DiagnosticsError::from)?;
    Ok(())
}

/// Read every persisted diagnostic, oldest scan first.
///
/// "Oldest scan first" rather than "oldest adapter first" so the
/// dashboard surfaces stale adapters at the top — if the Aider adapter
/// hasn't run in 30 minutes while the custom adapter ran 2 seconds ago,
/// the user sees the Aider row first and can ask "why didn't this
/// scan?".
pub fn list(storage: &Arc<Storage>) -> Result<Vec<AdapterDiagnostic>, DiagnosticsError> {
    storage
        .with_conn(repository::list)
        .map_err(DiagnosticsError::from)
}
