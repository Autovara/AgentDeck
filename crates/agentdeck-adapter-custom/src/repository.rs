//! CRUD against the `custom_adapters` SQLite table.
//!
//! All public methods are synchronous and serialise on the shared
//! [`Storage`] mutex. Callers in an async context should wrap calls in
//! `tokio::task::spawn_blocking`. The repository is stateless beyond its
//! borrowed [`Storage`] reference, so it is cheap to construct.

use agentdeck_core::Clock;
use agentdeck_storage::Storage;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
use uuid::Uuid;

use crate::definition::{CustomAdapter, CustomAdapterError, MatchKind, ValidatedNewCustomAdapter};

/// Read/write façade over the `custom_adapters` table.
pub struct CustomAdapterRepository<'a> {
    storage: &'a Storage,
}

impl<'a> CustomAdapterRepository<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    /// All defined adapters in deterministic order: enabled first, then by
    /// `created_at` ascending.
    pub fn list(&self) -> Result<Vec<CustomAdapter>, CustomAdapterError> {
        let rows = self.storage.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, label, enabled, agent_name, color, match_kind, pattern, \
                        cost_per_hour_cents, notes, created_at, updated_at \
                 FROM custom_adapters \
                 ORDER BY enabled DESC, created_at ASC",
            )?;
            let iter = stmt.query_map([], from_row)?;
            iter.collect::<rusqlite::Result<Vec<_>>>()
        })?;
        // Surface row-level parse errors as concrete adapter errors so callers
        // never see opaque rusqlite text.
        rows.into_iter().collect()
    }

    /// Look up a single adapter by id.
    pub fn get(&self, id: Uuid) -> Result<CustomAdapter, CustomAdapterError> {
        let id_str = id.to_string();
        let row = self.storage.with_conn(|conn| {
            conn.query_row(
                "SELECT id, label, enabled, agent_name, color, match_kind, pattern, \
                        cost_per_hour_cents, notes, created_at, updated_at \
                 FROM custom_adapters WHERE id = ?1",
                [&id_str],
                from_row,
            )
            .optional()
        })?;
        row.ok_or(CustomAdapterError::NotFound(id))?
    }

    /// Insert a new adapter. The repository owns id/created_at/updated_at;
    /// validation owns everything else.
    pub fn add(
        &self,
        input: ValidatedNewCustomAdapter,
        clock: &dyn Clock,
    ) -> Result<CustomAdapter, CustomAdapterError> {
        let id = Uuid::new_v4();
        let now = clock.now();
        let now_str = format_ts(now);
        let id_str = id.to_string();

        let input_ref = &input.input;
        let label = input_ref.label.clone();
        let result = self.storage.with_conn(|conn| {
            conn.execute(
                "INSERT INTO custom_adapters \
                 (id, label, enabled, agent_name, color, match_kind, pattern, \
                  cost_per_hour_cents, notes, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    &id_str,
                    &input_ref.label,
                    if input_ref.enabled { 1 } else { 0 },
                    &input_ref.agent_name,
                    input_ref.color.as_deref(),
                    input_ref.match_kind.as_db_str(),
                    &input_ref.pattern,
                    input_ref.cost_per_hour_cents,
                    input_ref.notes.as_deref(),
                    &now_str,
                    &now_str,
                ],
            )
        });

        match result {
            Ok(_) => self.get(id),
            Err(agentdeck_storage::StorageError::Query(err)) if is_unique_violation(&err) => {
                Err(CustomAdapterError::DuplicateLabel(label))
            }
            Err(err) => Err(err.into()),
        }
    }

    /// Toggle the `enabled` flag.
    pub fn set_enabled(
        &self,
        id: Uuid,
        enabled: bool,
        clock: &dyn Clock,
    ) -> Result<(), CustomAdapterError> {
        let id_str = id.to_string();
        let now = format_ts(clock.now());
        let updated = self.storage.with_conn(|conn| {
            conn.execute(
                "UPDATE custom_adapters SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
                params![if enabled { 1 } else { 0 }, &now, &id_str],
            )
        })?;
        if updated == 0 {
            Err(CustomAdapterError::NotFound(id))
        } else {
            Ok(())
        }
    }

    /// Remove an adapter by id. Returns `Ok(())` when the row existed, an
    /// error otherwise.
    pub fn delete(&self, id: Uuid) -> Result<(), CustomAdapterError> {
        let id_str = id.to_string();
        let deleted = self.storage.with_conn(|conn| {
            conn.execute("DELETE FROM custom_adapters WHERE id = ?1", [&id_str])
        })?;
        if deleted == 0 {
            Err(CustomAdapterError::NotFound(id))
        } else {
            Ok(())
        }
    }

    /// All adapters with `enabled = 1`, used by the matcher.
    pub fn list_enabled(&self) -> Result<Vec<CustomAdapter>, CustomAdapterError> {
        let rows = self.storage.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, label, enabled, agent_name, color, match_kind, pattern, \
                        cost_per_hour_cents, notes, created_at, updated_at \
                 FROM custom_adapters \
                 WHERE enabled = 1 \
                 ORDER BY created_at ASC",
            )?;
            let iter = stmt.query_map([], from_row)?;
            iter.collect::<rusqlite::Result<Vec<_>>>()
        })?;
        rows.into_iter().collect()
    }
}

fn from_row(row: &Row<'_>) -> rusqlite::Result<Result<CustomAdapter, CustomAdapterError>> {
    let id_str: String = row.get("id")?;
    let label: String = row.get("label")?;
    let enabled_int: i64 = row.get("enabled")?;
    let agent_name: String = row.get("agent_name")?;
    let color: Option<String> = row.get("color")?;
    let match_kind_str: String = row.get("match_kind")?;
    let pattern: String = row.get("pattern")?;
    let cost_per_hour_cents: Option<i64> = row.get("cost_per_hour_cents")?;
    let notes: Option<String> = row.get("notes")?;
    let created_at_str: String = row.get("created_at")?;
    let updated_at_str: String = row.get("updated_at")?;

    Ok((|| {
        let id = Uuid::parse_str(&id_str).map_err(|err| {
            CustomAdapterError::Storage(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(err),
            ))
        })?;
        let match_kind = MatchKind::from_db_str(&match_kind_str).ok_or_else(|| {
            CustomAdapterError::Storage(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unknown match_kind {match_kind_str:?}"),
                )),
            ))
        })?;
        let created_at = parse_ts(&created_at_str)?;
        let updated_at = parse_ts(&updated_at_str)?;
        Ok(CustomAdapter {
            id,
            label,
            enabled: enabled_int != 0,
            agent_name,
            color,
            match_kind,
            pattern,
            cost_per_hour_cents,
            notes,
            created_at,
            updated_at,
        })
    })())
}

fn parse_ts(s: &str) -> Result<DateTime<Utc>, CustomAdapterError> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| {
            CustomAdapterError::Storage(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(err),
            ))
        })
}

fn format_ts(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn is_unique_violation(err: &rusqlite::Error) -> bool {
    if let rusqlite::Error::SqliteFailure(ffi_err, _) = err {
        ffi_err.code == rusqlite::ErrorCode::ConstraintViolation
            && ffi_err.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    } else {
        false
    }
}
