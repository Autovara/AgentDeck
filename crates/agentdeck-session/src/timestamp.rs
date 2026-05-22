//! RFC-3339 helpers used by every read/write of a `TEXT` timestamp column.
//!
//! The format here matches `strftime('%Y-%m-%dT%H:%M:%fZ', 'now')` used by
//! the seed row in `0001_initial.sql` and the helpers in
//! `agentdeck-adapter-custom/src/repository.rs`. Keeping the format
//! identical across writers means columns sort lexicographically and the
//! `DateTime::parse_from_rfc3339` reader accepts every value we write.

use chrono::{DateTime, Utc};

pub fn format_ts(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

pub fn parse_ts(s: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
        })
}
