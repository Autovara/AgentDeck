use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::TagError;

/// Maximum length of a tag name. Matches the alpha's other user-visible
/// labels (custom adapter `label`, etc.).
pub const MAX_TAG_NAME_LEN: usize = 64;

/// One row of `project_tags`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTag {
    pub id: Uuid,
    pub name: String,
    pub color: Option<String>,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for [`crate::create`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewProjectTag {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug)]
pub(crate) struct ValidatedTag {
    pub(crate) name: String,
    pub(crate) color: Option<String>,
    pub(crate) notes: Option<String>,
}

impl NewProjectTag {
    pub(crate) fn validate(self) -> Result<ValidatedTag, TagError> {
        let name = self.name.trim().to_string();
        if name.is_empty() || name.len() > MAX_TAG_NAME_LEN {
            return Err(TagError::NameLength(name.len()));
        }
        if name.chars().any(|c| c.is_control()) {
            return Err(TagError::NameControlChar);
        }
        if let Some(color) = self.color.as_deref() {
            if !is_valid_color(color) {
                return Err(TagError::InvalidColor(color.to_string()));
            }
        }
        Ok(ValidatedTag {
            name,
            color: self.color,
            notes: self.notes,
        })
    }
}

fn is_valid_color(color: &str) -> bool {
    let bytes = color.as_bytes();
    matches!(bytes.len(), 4 | 7)
        && bytes[0] == b'#'
        && bytes[1..].iter().all(u8::is_ascii_hexdigit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> NewProjectTag {
        NewProjectTag {
            name: "billing".into(),
            color: None,
            notes: None,
        }
    }

    #[test]
    fn validate_accepts_minimal() {
        base().validate().expect("validates");
    }

    #[test]
    fn rejects_empty_name() {
        let mut t = base();
        t.name = "  ".into();
        assert!(matches!(
            t.validate().unwrap_err(),
            TagError::NameLength(0)
        ));
    }

    #[test]
    fn rejects_long_name() {
        let mut t = base();
        t.name = "x".repeat(MAX_TAG_NAME_LEN + 1);
        assert!(matches!(
            t.validate().unwrap_err(),
            TagError::NameLength(_)
        ));
    }

    #[test]
    fn rejects_control_chars() {
        let mut t = base();
        t.name = "bad\tname".into();
        assert!(matches!(
            t.validate().unwrap_err(),
            TagError::NameControlChar
        ));
    }

    #[test]
    fn accepts_valid_hex_colors() {
        for c in ["#abc", "#ABC", "#abcdef", "#ABCDEF"] {
            let mut t = base();
            t.color = Some(c.into());
            t.validate().expect("validates");
        }
    }

    #[test]
    fn rejects_invalid_colors() {
        for c in ["abc", "#gg", "#12345", "#1234567", ""] {
            let mut t = base();
            t.color = Some(c.into());
            assert!(matches!(
                t.validate().unwrap_err(),
                TagError::InvalidColor(_)
            ));
        }
    }
}
