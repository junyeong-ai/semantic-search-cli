//! Source model for tracking data origin.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

/// Type of data source.
///
/// Known source types have dedicated variants for type safety.
/// Arbitrary source types are supported via `Other(String)`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum SourceType {
    /// Local file system
    #[default]
    Local,
    /// Jira issues
    Jira,
    /// Confluence pages
    Confluence,
    /// Figma designs
    Figma,
    /// Any other source type (e.g., "notion", "slack", "github")
    Other(String),
}

impl SourceType {
    /// Get the CLI command name for sources with integration.
    ///
    /// Returns `None` for sources without CLI integration.
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
            SourceType::Other(s) => write!(f, "{}", s),
        }
    }
}

impl std::str::FromStr for SourceType {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "local" => SourceType::Local,
            "jira" => SourceType::Jira,
            "confluence" => SourceType::Confluence,
            "figma" => SourceType::Figma,
            other => SourceType::Other(other.to_string()),
        })
    }
}

impl Serialize for SourceType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SourceType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(s.parse().unwrap())
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

    /// Source URL (for navigation)
    pub url: Option<String>,
}

impl Source {
    /// Create a new source.
    pub fn new(source_type: SourceType, location: impl Into<String>, url: Option<String>) -> Self {
        Self {
            source_type,
            location: location.into(),
            url,
        }
    }

    /// Create a new local file source.
    pub fn local(path: impl Into<String>) -> Self {
        Self::new(SourceType::Local, path, None)
    }

    /// Create a source with URL.
    pub fn with_url(
        source_type: SourceType,
        location: impl Into<String>,
        url: impl Into<String>,
    ) -> Self {
        Self::new(source_type, location, Some(url.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_type_display() {
        assert_eq!(SourceType::Local.to_string(), "local");
        assert_eq!(SourceType::Jira.to_string(), "jira");
        assert_eq!(SourceType::Confluence.to_string(), "confluence");
        assert_eq!(SourceType::Figma.to_string(), "figma");
        assert_eq!(
            SourceType::Other("notion".to_string()).to_string(),
            "notion"
        );
    }

    #[test]
    fn test_source_type_parse() {
        assert_eq!("local".parse::<SourceType>().unwrap(), SourceType::Local);
        assert_eq!("JIRA".parse::<SourceType>().unwrap(), SourceType::Jira);
        assert_eq!(
            "Confluence".parse::<SourceType>().unwrap(),
            SourceType::Confluence
        );
        assert_eq!(
            "notion".parse::<SourceType>().unwrap(),
            SourceType::Other("notion".to_string())
        );
        assert_eq!(
            "GitHub".parse::<SourceType>().unwrap(),
            SourceType::Other("github".to_string())
        );
    }

    #[test]
    fn test_source_type_serde() {
        // Known types
        let jira = SourceType::Jira;
        let json = serde_json::to_string(&jira).unwrap();
        assert_eq!(json, "\"jira\"");
        let parsed: SourceType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, jira);

        // Other types
        let slack = SourceType::Other("slack".to_string());
        let json = serde_json::to_string(&slack).unwrap();
        assert_eq!(json, "\"slack\"");
        let parsed: SourceType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, slack);
    }

    #[test]
    fn test_cli_command() {
        assert_eq!(SourceType::Jira.cli_command(), Some("atlassian"));
        assert_eq!(SourceType::Confluence.cli_command(), Some("atlassian"));
        assert_eq!(SourceType::Figma.cli_command(), Some("figma"));
        assert_eq!(SourceType::Local.cli_command(), None);
        assert_eq!(SourceType::Other("notion".to_string()).cli_command(), None);
    }

    #[test]
    fn test_source_local() {
        let source = Source::local("/path/to/file.rs");
        assert_eq!(source.source_type, SourceType::Local);
        assert_eq!(source.location, "/path/to/file.rs");
        assert!(source.url.is_none());
    }

    #[test]
    fn test_source_with_url() {
        let source = Source::with_url(
            SourceType::Jira,
            "PROJ-123",
            "https://jira.example.com/browse/PROJ-123",
        );
        assert_eq!(source.source_type, SourceType::Jira);
        assert_eq!(source.location, "PROJ-123");
        assert_eq!(
            source.url,
            Some("https://jira.example.com/browse/PROJ-123".to_string())
        );
    }

    #[test]
    fn test_source_new() {
        let source = Source::new(
            SourceType::Other("notion".to_string()),
            "page-123",
            Some("https://notion.so/page-123".to_string()),
        );
        assert_eq!(source.source_type, SourceType::Other("notion".to_string()));
        assert_eq!(source.location, "page-123");
        assert!(source.url.is_some());

        // Without URL
        let source = Source::new(SourceType::Other("custom".to_string()), "doc-456", None);
        assert_eq!(source.source_type, SourceType::Other("custom".to_string()));
        assert!(source.url.is_none());
    }
}
