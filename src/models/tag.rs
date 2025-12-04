//! Tag model for document classification and filtering.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::TagError;

/// Key-value pair for document classification and filtering.
///
/// Tags follow the format `key:value` and are used to categorize
/// documents for filtering during search operations.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Tag {
    /// Tag key (e.g., "project", "team", "env")
    pub key: String,
    /// Tag value (e.g., "myapp", "backend", "prod")
    pub value: String,
}

impl Tag {
    /// Create a new tag with the given key and value.
    ///
    /// # Errors
    ///
    /// Returns `TagError::InvalidKey` if the key is invalid.
    /// Returns `TagError::InvalidValue` if the value is invalid.
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Result<Self, TagError> {
        let key = key.into();
        let value = value.into();

        Self::validate_key(&key)?;
        Self::validate_value(&value)?;

        Ok(Self { key, value })
    }

    /// Validate a tag key.
    ///
    /// Keys must be 1-50 characters, alphanumeric with underscore/hyphen.
    fn validate_key(key: &str) -> Result<(), TagError> {
        if key.is_empty() {
            return Err(TagError::InvalidKey("key cannot be empty".to_string()));
        }
        if key.len() > 50 {
            return Err(TagError::InvalidKey(
                "key cannot exceed 50 characters".to_string(),
            ));
        }
        if !key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(TagError::InvalidKey(
                "key must be alphanumeric with underscore or hyphen".to_string(),
            ));
        }
        Ok(())
    }

    /// Validate a tag value.
    ///
    /// Values must be 1-100 characters, alphanumeric with underscore/hyphen/dot.
    fn validate_value(value: &str) -> Result<(), TagError> {
        if value.is_empty() {
            return Err(TagError::InvalidValue("value cannot be empty".to_string()));
        }
        if value.len() > 100 {
            return Err(TagError::InvalidValue(
                "value cannot exceed 100 characters".to_string(),
            ));
        }
        if !value
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '.')
        {
            return Err(TagError::InvalidValue(
                "value must be alphanumeric with underscore, hyphen, or dot".to_string(),
            ));
        }
        Ok(())
    }

    /// Convert to the string format used in Qdrant payload.
    pub fn to_payload_string(&self) -> String {
        format!("{}:{}", self.key, self.value)
    }
}

impl fmt::Display for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.key, self.value)
    }
}

impl FromStr for Tag {
    type Err = TagError;

    /// Parse a tag from the string format "key:value".
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(TagError::ParseError(format!(
                "invalid tag format '{}', expected 'key:value'",
                s
            )));
        }
        Tag::new(parts[0], parts[1])
    }
}

/// Parse multiple tags from a comma-separated string.
///
/// # Example
///
/// ```ignore
/// let tags = parse_tags("project:myapp,team:backend")?;
/// ```
pub fn parse_tags(s: &str) -> Result<Vec<Tag>, TagError> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    s.split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .map(|t| t.parse::<Tag>())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_new() {
        let tag = Tag::new("project", "myapp").unwrap();
        assert_eq!(tag.key, "project");
        assert_eq!(tag.value, "myapp");
    }

    #[test]
    fn test_tag_display() {
        let tag = Tag::new("team", "backend").unwrap();
        assert_eq!(tag.to_string(), "team:backend");
    }

    #[test]
    fn test_tag_parse() {
        let tag: Tag = "env:prod".parse().unwrap();
        assert_eq!(tag.key, "env");
        assert_eq!(tag.value, "prod");
    }

    #[test]
    fn test_parse_tags() {
        let tags = parse_tags("project:myapp,team:backend").unwrap();
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].key, "project");
        assert_eq!(tags[1].key, "team");
    }

    #[test]
    fn test_tag_invalid_format() {
        let result: Result<Tag, _> = "invalid".parse();
        assert!(result.is_err());
    }

    #[test]
    fn test_tag_empty_key() {
        let result = Tag::new("", "value");
        assert!(result.is_err());
    }

    #[test]
    fn test_tag_with_dot_in_value() {
        let tag = Tag::new("version", "1.0.0").unwrap();
        assert_eq!(tag.value, "1.0.0");
    }
}
