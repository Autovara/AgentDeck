//! Data model and validation for custom adapters.

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Maximum length of the user-visible adapter label.
///
/// Bounded for UI sanity and to keep the SQLite UNIQUE index well-behaved.
pub const MAX_LABEL_LEN: usize = 64;

/// Maximum length of a regex pattern.
///
/// The Rust `regex` crate enforces its own size and complexity limits; this
/// extra ceiling guards against pathological storage payloads sneaking in
/// through `add_custom_adapter`.
pub const MAX_PATTERN_LEN: usize = 1024;

/// One row in the `custom_adapters` table, fully resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomAdapter {
    pub id: Uuid,
    pub label: String,
    pub enabled: bool,
    pub agent_name: String,
    pub color: Option<String>,
    pub match_kind: MatchKind,
    pub pattern: String,
    pub cost_per_hour_cents: Option<i64>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for [`CustomAdapterRepository::add`].
///
/// Distinct from [`CustomAdapter`] because the repository owns the `id`,
/// `created_at`, and `updated_at` fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCustomAdapter {
    pub label: String,
    pub agent_name: String,
    pub match_kind: MatchKind,
    pub pattern: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub cost_per_hour_cents: Option<i64>,
    #[serde(default)]
    pub notes: Option<String>,
}

fn default_enabled() -> bool {
    true
}

/// Which process field the regex is matched against.
///
/// Kept in lock-step with the `match_kind` CHECK in the
/// `0002_custom_adapters.sql` migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchKind {
    /// Match against [`agentdeck_core::ProcessInfo::name`].
    Name,
    /// Match against the joined [`agentdeck_core::ProcessInfo::cmdline`].
    Cmdline,
    /// Match against [`agentdeck_core::ProcessInfo::cwd`]. Processes whose
    /// `cwd` is `None` never match (no implicit empty-string).
    Cwd,
}

impl MatchKind {
    pub fn as_db_str(&self) -> &'static str {
        match self {
            MatchKind::Name => "name",
            MatchKind::Cmdline => "cmdline",
            MatchKind::Cwd => "cwd",
        }
    }

    pub fn from_db_str(s: &str) -> Option<Self> {
        match s {
            "name" => Some(MatchKind::Name),
            "cmdline" => Some(MatchKind::Cmdline),
            "cwd" => Some(MatchKind::Cwd),
            _ => None,
        }
    }
}

/// Errors raised while validating, persisting, or matching custom adapters.
#[derive(Debug, Error)]
pub enum CustomAdapterError {
    #[error("custom adapter label must be 1..={MAX_LABEL_LEN} characters; got {0} characters")]
    LabelLength(usize),
    #[error("custom adapter label must not contain control characters")]
    LabelControlChar,
    #[error("agent name must be 1..=64 characters; got {0} characters")]
    AgentNameLength(usize),
    #[error("pattern must be 1..={MAX_PATTERN_LEN} characters; got {0} characters")]
    PatternLength(usize),
    #[error("pattern is not a valid Rust regex: {0}")]
    InvalidRegex(#[from] regex::Error),
    #[error("colour must be a #RGB or #RRGGBB hex string; got {0:?}")]
    InvalidColor(String),
    #[error("a custom adapter with label {0:?} already exists")]
    DuplicateLabel(String),
    #[error("no custom adapter found with id {0}")]
    NotFound(Uuid),
    #[error("storage error: {0}")]
    Storage(#[from] rusqlite::Error),
    #[error("storage layer error: {0}")]
    StorageLayer(#[from] agentdeck_storage::StorageError),
}

impl NewCustomAdapter {
    /// Validate the user-supplied input, normalising trimming-whitespace
    /// where appropriate and compiling the regex once. The resulting
    /// [`ValidatedNewCustomAdapter`] is the only thing
    /// [`CustomAdapterRepository::add`] will accept.
    pub fn validate(mut self) -> Result<ValidatedNewCustomAdapter, CustomAdapterError> {
        self.label = self.label.trim().to_string();
        self.agent_name = self.agent_name.trim().to_string();
        self.pattern = self.pattern.trim().to_string();

        validate_label(&self.label)?;
        validate_agent_name(&self.agent_name)?;
        validate_pattern(&self.pattern)?;
        if let Some(color) = &self.color {
            validate_color(color)?;
        }

        let compiled = Regex::new(&self.pattern)?;
        Ok(ValidatedNewCustomAdapter {
            input: self,
            compiled,
        })
    }
}

/// A [`NewCustomAdapter`] that has passed validation and carries the
/// compiled regex. Construct via [`NewCustomAdapter::validate`].
#[derive(Debug)]
pub struct ValidatedNewCustomAdapter {
    pub(crate) input: NewCustomAdapter,
    pub(crate) compiled: Regex,
}

impl ValidatedNewCustomAdapter {
    pub fn label(&self) -> &str {
        &self.input.label
    }
    pub fn agent_name(&self) -> &str {
        &self.input.agent_name
    }
    pub fn pattern(&self) -> &str {
        &self.input.pattern
    }
    pub fn compiled(&self) -> &Regex {
        &self.compiled
    }
}

fn validate_label(label: &str) -> Result<(), CustomAdapterError> {
    if label.is_empty() || label.len() > MAX_LABEL_LEN {
        return Err(CustomAdapterError::LabelLength(label.len()));
    }
    if label.chars().any(|c| c.is_control()) {
        return Err(CustomAdapterError::LabelControlChar);
    }
    Ok(())
}

fn validate_agent_name(name: &str) -> Result<(), CustomAdapterError> {
    if name.is_empty() || name.len() > 64 {
        return Err(CustomAdapterError::AgentNameLength(name.len()));
    }
    Ok(())
}

fn validate_pattern(pattern: &str) -> Result<(), CustomAdapterError> {
    if pattern.is_empty() || pattern.len() > MAX_PATTERN_LEN {
        return Err(CustomAdapterError::PatternLength(pattern.len()));
    }
    Ok(())
}

fn validate_color(color: &str) -> Result<(), CustomAdapterError> {
    let bytes = color.as_bytes();
    let hex_ok = matches!(bytes.len(), 4 | 7)
        && bytes[0] == b'#'
        && bytes[1..].iter().all(u8::is_ascii_hexdigit);
    if hex_ok {
        Ok(())
    } else {
        Err(CustomAdapterError::InvalidColor(color.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_input() -> NewCustomAdapter {
        NewCustomAdapter {
            label: "my agent".into(),
            agent_name: "my-agent".into(),
            match_kind: MatchKind::Name,
            pattern: "my-agent".into(),
            enabled: true,
            color: None,
            cost_per_hour_cents: None,
            notes: None,
        }
    }

    #[test]
    fn validate_accepts_minimal_input() {
        base_input().validate().expect("validates");
    }

    #[test]
    fn validate_rejects_empty_label() {
        let mut input = base_input();
        input.label = "".into();
        assert!(matches!(
            input.validate().unwrap_err(),
            CustomAdapterError::LabelLength(0)
        ));
    }

    #[test]
    fn validate_rejects_long_label() {
        let mut input = base_input();
        input.label = "x".repeat(MAX_LABEL_LEN + 1);
        let err = input.validate().unwrap_err();
        assert!(matches!(err, CustomAdapterError::LabelLength(_)));
    }

    #[test]
    fn validate_rejects_control_chars_in_label() {
        let mut input = base_input();
        input.label = "bad\tlabel".into();
        assert!(matches!(
            input.validate().unwrap_err(),
            CustomAdapterError::LabelControlChar
        ));
    }

    #[test]
    fn validate_rejects_invalid_regex() {
        let mut input = base_input();
        input.pattern = "(unclosed".into();
        assert!(matches!(
            input.validate().unwrap_err(),
            CustomAdapterError::InvalidRegex(_)
        ));
    }

    #[test]
    fn validate_accepts_valid_hex_colors() {
        for color in ["#fff", "#FFF", "#abcdef", "#ABCDEF", "#012345"] {
            let mut input = base_input();
            input.color = Some(color.to_string());
            input.validate().expect("validates");
        }
    }

    #[test]
    fn validate_rejects_bad_hex_colors() {
        for color in ["fff", "#gg", "#12345", "#1234567", ""] {
            let mut input = base_input();
            input.color = Some(color.to_string());
            assert!(
                matches!(
                    input.validate().unwrap_err(),
                    CustomAdapterError::InvalidColor(_)
                ),
                "rejected colour {color:?}"
            );
        }
    }

    #[test]
    fn validate_trims_whitespace() {
        let mut input = base_input();
        input.label = "  trimmed  ".into();
        input.agent_name = " name ".into();
        input.pattern = "  pat  ".into();
        let v = input.validate().expect("validates");
        assert_eq!(v.label(), "trimmed");
        assert_eq!(v.agent_name(), "name");
        assert_eq!(v.pattern(), "pat");
    }

    #[test]
    fn match_kind_db_str_round_trips() {
        for kind in [MatchKind::Name, MatchKind::Cmdline, MatchKind::Cwd] {
            assert_eq!(MatchKind::from_db_str(kind.as_db_str()), Some(kind));
        }
        assert!(MatchKind::from_db_str("bogus").is_none());
    }
}
