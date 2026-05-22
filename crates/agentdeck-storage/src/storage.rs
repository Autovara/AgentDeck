//! The [`Storage`] handle.
//!
//! `Storage` wraps a single [`rusqlite::Connection`] behind a `Mutex`. The
//! alpha has a single Tokio runtime with a single monitor-core task that
//! serialises all writes; this is enough to be correct without a connection
//! pool. When the workload grows (Telegram + UI + monitor concurrent reads)
//! we will revisit and likely add `r2d2_sqlite`.
//!
//! All public methods are synchronous. Callers running inside an async
//! context should wrap their use of [`Storage::with_conn`] or
//! [`Storage::with_conn_mut`] in `tokio::task::spawn_blocking`.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::Utc;
use rusqlite::{Connection, OpenFlags};

use crate::diagnostics::{StorageDiagnostics, TableStat, KNOWN_TABLES};
use crate::path::default_db_path;
use crate::schema;
use crate::StorageError;

/// Handle to the AgentDeck SQLite database.
pub struct Storage {
    path: PathBuf,
    conn: Mutex<Connection>,
}

impl Storage {
    /// Open the database at the platform-default location, creating the
    /// directory and running any pending migrations.
    pub fn open_default() -> Result<Self, StorageError> {
        let path = default_db_path()?;
        Self::open(path)
    }

    /// Open the database at `path`, creating the parent directory and running
    /// any pending migrations.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let path = path.as_ref().to_path_buf();

        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|source| StorageError::CreateDir {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }
        }

        let mut conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|source| StorageError::OpenDatabase {
            path: path.clone(),
            source,
        })?;

        configure_pragmas(&conn)?;
        schema::build().to_latest(&mut conn)?;

        Ok(Self {
            path,
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory database, run all migrations, and return the handle.
    /// Useful for tests and previews; the database has no persistent backing.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let mut conn =
            Connection::open_in_memory().map_err(|source| StorageError::OpenDatabase {
                path: PathBuf::from(":memory:"),
                source,
            })?;
        configure_pragmas(&conn)?;
        schema::build().to_latest(&mut conn)?;
        Ok(Self {
            path: PathBuf::from(":memory:"),
            conn: Mutex::new(conn),
        })
    }

    /// Filesystem path of the database file, or `":memory:"` for in-memory
    /// databases.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Borrow the underlying connection for a single read-only or
    /// transactional operation.
    pub fn with_conn<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&Connection) -> rusqlite::Result<R>,
    {
        let guard = self.conn.lock().map_err(|_| StorageError::Poisoned)?;
        Ok(f(&guard)?)
    }

    /// Borrow the underlying connection mutably (required for transactions).
    pub fn with_conn_mut<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut Connection) -> rusqlite::Result<R>,
    {
        let mut guard = self.conn.lock().map_err(|_| StorageError::Poisoned)?;
        Ok(f(&mut guard)?)
    }

    /// Compute a [`StorageDiagnostics`] snapshot.
    pub fn diagnostics(&self) -> Result<StorageDiagnostics, StorageError> {
        let captured_at = Utc::now();
        let size_bytes = read_size(&self.path);

        let (schema_version, journal_mode, tables) = self.with_conn(|conn| {
            let schema_version: u32 =
                conn.query_row("PRAGMA user_version;", [], |row| row.get::<_, i64>(0))? as u32;

            let journal_mode: String =
                conn.query_row("PRAGMA journal_mode;", [], |row| row.get(0))?;

            let mut tables = Vec::with_capacity(KNOWN_TABLES.len());
            for name in KNOWN_TABLES {
                let sql = format!("SELECT count(*) FROM {name}");
                let row_count: i64 = conn.query_row(&sql, [], |row| row.get(0))?;
                tables.push(TableStat { name, row_count });
            }
            Ok((schema_version, journal_mode, tables))
        })?;

        Ok(StorageDiagnostics {
            path: self.path.clone(),
            size_bytes,
            schema_version,
            journal_mode,
            applied_migrations: schema::applied_migrations(),
            tables,
            captured_at,
        })
    }
}

fn configure_pragmas(conn: &Connection) -> Result<(), StorageError> {
    // WAL gives us non-blocking readers and is the standard recommendation
    // for desktop apps. NORMAL synchronous is the WAL-friendly default; FULL
    // is unnecessary for the alpha's risk profile.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // Treat busy DBs as a transient error with a short timeout; the storage
    // layer should never block the UI thread for more than a few hundred ms.
    conn.busy_timeout(std::time::Duration::from_millis(750))?;
    Ok(())
}

fn read_size(path: &Path) -> Option<u64> {
    if path.as_os_str() == ":memory:" {
        return None;
    }
    fs::metadata(path).ok().map(|m| m.len())
}
