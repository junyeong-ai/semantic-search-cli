//! Source model for tracking data origin.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Type of data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    /// Local file system
    Local,
    /// Jira issues
    Jira,
    /// Confluence pages
    Confluence,
    /// Figma designs
    Figma,
    /// User-imported data (JSON/JSONL)
    Custom,
}

impl SourceType {
    /// Get all available source types.
    pub fn all() -> &'static [SourceType] {
        &[
            SourceType::Local,
            SourceType::Jira,
            SourceType::Confluence,
            SourceType::Figma,
            SourceType::Custom,
        ]
    }

    /// Check if this source type is external (requires CLI tool).
    pub fn is_external(&self) -> bool {
        matches!(
            self,
            SourceType::Jira | SourceType::Confluence | SourceType::Figma
        )
    }

    /// Get the CLI command name for external sources.
    pub fn cli_command(&self) -> Option<&'static str> {
        match self {
            SourceType::Jira | SourceType::Confluence => Some("atlassian"),
            SourceType::Figma => Some("figma"),
            _ => None,
        }
    }
}

impl fmt::Display for SourceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceType::Local => write!(f, "local"),
            SourceType::Jira => write!(f, "jira"),
            SourceType::Confluence => write!(f, "confluence"),
            SourceType::Figma => write!(f, "figma"),
            SourceType::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for SourceType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "local" => Ok(SourceType::Local),
            "jira" => Ok(SourceType::Jira),
            "confluence" => Ok(SourceType::Confluence),
            "figma" => Ok(SourceType::Figma),
            "custom" => Ok(SourceType::Custom),
            _ => Err(format!("unknown source type: {}", s)),
        }
    }
}

/// Metadata about data origin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// Type of source
    pub source_type: SourceType,

    /// For local: absolute file path
    /// For external: canonical URL or identifier
    pub location: String,

    /// External source URL (for navigation)
    pub url: Option<String>,
}

impl Source {
    /// Create a new local file source.
    pub fn local(path: impl Into<String>) -> Self {
        Self {
            source_type: SourceType::Local,
            location: path.into(),
            url: None,
        }
    }

    /// Create a new external source with URL.
    pub fn external(
        source_type: SourceType,
        location: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self {
            source_type,
            location: location.into(),
            url: Some(url.into()),
        }
    }

    /// Create a new custom (imported) source.
    pub fn custom(url: impl Into<String>) -> Self {
        let url = url.into();
        Self {
            source_type: SourceType::Custom,
            location: url.clone(),
            url: Some(url),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_type_display() {
        assert_eq!(SourceType::Local.to_string(), "local");
        assert_eq!(SourceType::Jira.to_string(), "jira");
    }

    #[test]
    fn test_source_type_parse() {
        assert_eq!("local".parse::<SourceType>().unwrap(), SourceType::Local);
        assert_eq!("JIRA".parse::<SourceType>().unwrap(), SourceType::Jira);
    }

    #[test]
    fn test_source_local() {
        let source = Source::local("/path/to/file.rs");
        assert_eq!(source.source_type, SourceType::Local);
        assert_eq!(source.location, "/path/to/file.rs");
        assert!(source.url.is_none());
    }

    #[test]
    fn test_source_external() {
        let source = Source::external(
            SourceType::Jira,
            "PROJ-123",
            "https://jira.example.com/browse/PROJ-123",
        );
        assert_eq!(source.source_type, SourceType::Jira);
        assert!(source.url.is_some());
    }

    #[test]
    fn test_is_external() {
        assert!(!SourceType::Local.is_external());
        assert!(SourceType::Jira.is_external());
        assert!(SourceType::Confluence.is_external());
        assert!(SourceType::Figma.is_external());
        assert!(!SourceType::Custom.is_external());
    }
}
