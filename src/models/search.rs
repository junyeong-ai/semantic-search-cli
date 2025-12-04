//! Search-related models for queries and results.

use serde::{Deserialize, Serialize};

use super::source::{Source, SourceType};
use super::tag::Tag;

/// Output format for search results.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Human-readable text format
    #[default]
    Text,
    /// Machine-parseable JSON format
    Json,
    /// Documentation-friendly Markdown format
    Markdown,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "text" => Ok(OutputFormat::Text),
            "json" => Ok(OutputFormat::Json),
            "markdown" | "md" => Ok(OutputFormat::Markdown),
            _ => Err(format!("unknown output format: {}", s)),
        }
    }
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Markdown => write!(f, "markdown"),
        }
    }
}

/// User's search request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Natural language query text
    pub query: String,

    /// Maximum results to return
    pub limit: u32,

    /// Tag filters (AND logic)
    pub tags: Vec<Tag>,

    /// Source type filters (OR logic)
    pub source_types: Vec<SourceType>,

    /// Output format preference
    pub format: OutputFormat,

    /// Minimum similarity threshold (0.0-1.0)
    pub min_score: Option<f32>,

    /// Whether to include context around matches
    pub include_context: bool,
}

impl Default for SearchQuery {
    fn default() -> Self {
        Self {
            query: String::new(),
            limit: 10,
            tags: Vec::new(),
            source_types: Vec::new(),
            format: OutputFormat::Text,
            min_score: None,
            include_context: true,
        }
    }
}

impl SearchQuery {
    /// Create a new search query with the given text.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            ..Default::default()
        }
    }

    /// Set the result limit.
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = limit;
        self
    }

    /// Add tag filters.
    pub fn with_tags(mut self, tags: Vec<Tag>) -> Self {
        self.tags = tags;
        self
    }

    /// Add source type filters.
    pub fn with_source_types(mut self, source_types: Vec<SourceType>) -> Self {
        self.source_types = source_types;
        self
    }

    /// Set the output format.
    pub fn with_format(mut self, format: OutputFormat) -> Self {
        self.format = format;
        self
    }

    /// Set the minimum score threshold.
    pub fn with_min_score(mut self, min_score: f32) -> Self {
        self.min_score = Some(min_score);
        self
    }

    /// Set whether to include context.
    pub fn with_context(mut self, include: bool) -> Self {
        self.include_context = include;
        self
    }
}

/// A single search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// Matching chunk ID
    pub chunk_id: String,

    /// Similarity score (0.0-1.0)
    pub score: f32,

    /// Chunk content
    pub content: String,

    /// Source information
    pub source: Source,

    /// Associated tags
    pub tags: Vec<Tag>,

    /// Location hint (file path, line numbers, or URL)
    pub location: String,

    /// Context before matched content
    pub context_before: Option<String>,

    /// Context after matched content
    pub context_after: Option<String>,

    /// Line numbers if available
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
}

/// Collection of search results.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResults {
    /// Query that was executed
    pub query: String,

    /// Matching results
    pub results: Vec<SearchResult>,

    /// Total matches (before limit)
    pub total: u64,

    /// Query execution time in milliseconds
    pub duration_ms: u64,
}

impl SearchResults {
    /// Create a new search results container.
    pub fn new(query: String, results: Vec<SearchResult>, total: u64, duration_ms: u64) -> Self {
        Self {
            query,
            results,
            total,
            duration_ms,
        }
    }

    /// Check if there are no results.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Get the number of results.
    pub fn len(&self) -> usize {
        self.results.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_parse() {
        assert_eq!("text".parse::<OutputFormat>().unwrap(), OutputFormat::Text);
        assert_eq!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json);
        assert_eq!(
            "md".parse::<OutputFormat>().unwrap(),
            OutputFormat::Markdown
        );
    }

    #[test]
    fn test_search_query_builder() {
        let query = SearchQuery::new("authentication")
            .with_limit(20)
            .with_min_score(0.5)
            .with_format(OutputFormat::Json);

        assert_eq!(query.query, "authentication");
        assert_eq!(query.limit, 20);
        assert_eq!(query.min_score, Some(0.5));
        assert_eq!(query.format, OutputFormat::Json);
    }

    #[test]
    fn test_search_results() {
        let results = SearchResults::new("test".to_string(), vec![], 0, 50);
        assert!(results.is_empty());
        assert_eq!(results.duration_ms, 50);
    }
}
