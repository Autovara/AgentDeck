//! SQL helpers over `adapter_diagnostics`.
//!
//! Every function takes a `&Connection` (which a `&Transaction`
//! deref-coerces to). [`crate::record`] wraps a batch in one
//! transaction; [`crate::list`] is a single read.

use agentdeck_adapter::{AdapterDiagnostic, CapabilityLevel, Confidence};
use chrono::{DateTime, Utc};
use rusqlite::{params, types::Type, Connection, Error as SqlError, Row};

const COLUMNS: &str = "adapter_name, enabled, capability_level, last_scan_time, \
     detected_count, data_sources_used, missing_permissions, failure_reasons, \
     confidence, known_limitations, updated_at";

/// Upsert one diagnostic row.
///
/// `INSERT OR REPLACE` enforces the §17 "keep only the most recent
/// scan per adapter" discipline at the SQL level: there is never more
/// than one row per `adapter_name`. The `updated_at` column always
/// becomes the wall-clock time of the upsert (not the scan time) so a
/// reader can spot a stalled adapter by comparing the two timestamps.
pub(crate) fn upsert(conn: &Connection, d: &AdapterDiagnostic) -> rusqlite::Result<()> {
    let updated_at = Utc::now();
    let data_sources = encode_json_array(&d.data_sources_used);
    let missing = encode_json_array(&d.missing_permissions);
    let failures = encode_json_array(&d.failure_reasons);
    let limitations = encode_json_array(&d.known_limitations);

    let written = conn.execute(
        "INSERT OR REPLACE INTO adapter_diagnostics \
            (adapter_name, enabled, capability_level, last_scan_time, \
             detected_count, data_sources_used, missing_permissions, \
             failure_reasons, confidence, known_limitations, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            &d.adapter_name,
            if d.enabled { 1 } else { 0 },
            d.capability_level.as_u8() as i64,
            format_ts(d.last_scan_time),
            d.detected_count as i64,
            data_sources,
            missing,
            failures,
            d.confidence.as_db_str(),
            limitations,
            format_ts(updated_at),
        ],
    )?;
    debug_assert_eq!(
        written, 1,
        "INSERT OR REPLACE on adapter_diagnostics must affect one row"
    );
    Ok(())
}

/// Read every row, oldest `last_scan_time` first.
pub(crate) fn list(conn: &Connection) -> rusqlite::Result<Vec<AdapterDiagnostic>> {
    let query = format!("SELECT {COLUMNS} FROM adapter_diagnostics ORDER BY last_scan_time ASC");
    let mut stmt = conn.prepare(&query)?;
    let iter = stmt.query_map([], row_to_diagnostic)?;
    iter.collect()
}

fn row_to_diagnostic(row: &Row<'_>) -> rusqlite::Result<AdapterDiagnostic> {
    let enabled_int: i64 = row.get("enabled")?;
    let level_int: i64 = row.get("capability_level")?;
    let capability_level = u8::try_from(level_int)
        .ok()
        .and_then(CapabilityLevel::from_u8)
        .ok_or_else(|| enum_err("capability_level", &level_int.to_string()))?;

    let last_scan_str: String = row.get("last_scan_time")?;
    let confidence_str: String = row.get("confidence")?;
    let confidence = Confidence::from_db_str(&confidence_str)
        .ok_or_else(|| enum_err("confidence", &confidence_str))?;

    let detected: i64 = row.get("detected_count")?;
    let detected_count = u32::try_from(detected.max(0)).unwrap_or(0);

    let data_sources_used = decode_json_array(row, "data_sources_used")?;
    let missing_permissions = decode_json_array(row, "missing_permissions")?;
    let failure_reasons = decode_json_array(row, "failure_reasons")?;
    let known_limitations = decode_json_array(row, "known_limitations")?;

    Ok(AdapterDiagnostic {
        adapter_name: row.get("adapter_name")?,
        enabled: enabled_int != 0,
        capability_level,
        last_scan_time: parse_ts(&last_scan_str)?,
        detected_count,
        data_sources_used,
        missing_permissions,
        failure_reasons,
        confidence,
        known_limitations,
    })
}

fn encode_json_array(values: &[String]) -> String {
    // Vec<String> always serialises; we never store NULL because the
    // dashboard prefers an explicit empty array over a Nullable<Vec>.
    serde_json::to_string(values).expect("Vec<String> always serialises to JSON")
}

fn decode_json_array(row: &Row<'_>, column: &str) -> rusqlite::Result<Vec<String>> {
    let raw: Option<String> = row.get(column)?;
    match raw.as_deref() {
        None | Some("") => Ok(Vec::new()),
        Some(json) => serde_json::from_str(json)
            .map_err(|err| SqlError::FromSqlConversionFailure(0, Type::Text, Box::new(err))),
    }
}

fn enum_err(field: &'static str, value: &str) -> SqlError {
    SqlError::FromSqlConversionFailure(
        0,
        Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("unknown {field} {value:?}"),
        )),
    )
}

fn format_ts(ts: DateTime<Utc>) -> String {
    // Millisecond precision RFC-3339 with `Z`, matching the other
    // AgentDeck repositories so lexicographic sort == chronological
    // sort across tables.
    ts.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

fn parse_ts(s: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|err| SqlError::FromSqlConversionFailure(0, Type::Text, Box::new(err)))
}
