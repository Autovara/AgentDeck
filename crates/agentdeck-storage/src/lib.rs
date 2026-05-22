//! AgentDeck local SQLite storage.
//!
//! This crate owns the schema, migrations, and the [`Storage`] handle that
//! the rest of the workspace uses to talk to SQLite. It is intentionally
//! conservative:
//!
//! - One SQLite database per AgentDeck install, at the per-OS app-data path
//!   documented in `planning/alpha-build-plan.md` §14.
//! - WAL journal mode and `synchronous = NORMAL` for safe concurrent reads.
//! - All schema changes are migrations (see [`schema`]); migrations are
//!   immutable once shipped.
//! - The only consumer that holds a [`Storage`] is the monitor core. The
//!   Tauri UI and the Telegram engine talk to the monitor core, not to the
//!   database. The shell does cache a [`Storage`] today only so that the
//!   diagnostics card can render the DB path and applied migrations before
//!   the monitor core lands.
//!
//! Most callers should use [`Storage::open_default`] which resolves the
//! platform-specific data directory, creates it if missing, and runs all
//! pending migrations before returning.

mod diagnostics;
mod error;
mod path;
mod schema;
mod storage;

pub use diagnostics::{StorageDiagnostics, TableStat};
pub use error::StorageError;
pub use path::{default_data_dir, default_db_path, DEFAULT_DB_FILENAME};
pub use schema::{applied_migrations, current_schema_version, MigrationSummary};
pub use storage::Storage;
