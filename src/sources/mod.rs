//! Data source implementations.
//!
//! This module provides abstractions and implementations for different data sources
//! that can be indexed by the semantic search CLI.

mod confluence;
mod figma;
mod jira;
mod local;

pub use confluence::ConfluenceSource;
pub use figma::FigmaSource;
pub use jira::JiraSource;
pub use local::LocalSource;

use crate::error::SourceError;
use crate::models::{Document, SourceType, Tag};

/// Options for syncing data from a source.
#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    /// Source-specific query (e.g., JQL for Jira, CQL for Confluence)
    pub query: Option<String>,

    /// Project key (for Jira) or space key (for Confluence)
    pub project: Option<String>,

    /// Tags to apply to synced documents
    pub tags: Vec<Tag>,

    /// Maximum items to sync (None = unlimited with --all)
    pub limit: Option<u32>,

    /// Ancestor IDs to exclude (for Confluence)
    pub exclude_ancestors: Vec<String>,
}

/// Trait for external data sources.
pub trait DataSource: Send + Sync {
    /// Get the source type.
    fn source_type(&self) -> SourceType;

    /// Human-readable name.
    fn name(&self) -> &str;

    /// Check if the required CLI tool is available.
    fn check_available(&self) -> Result<bool, SourceError>;

    /// Sync data from the external source.
    fn sync(&self, options: SyncOptions) -> Result<Vec<Document>, SourceError>;

    /// Get installation instructions for the required CLI tool.
    fn install_instructions(&self) -> &str;
}

// Implement DataSource for JiraSource
impl DataSource for JiraSource {
    fn source_type(&self) -> SourceType {
        JiraSource::source_type(self)
    }

    fn name(&self) -> &str {
        JiraSource::name(self)
    }

    fn check_available(&self) -> Result<bool, SourceError> {
        JiraSource::check_available(self)
    }

    fn sync(&self, options: SyncOptions) -> Result<Vec<Document>, SourceError> {
        JiraSource::sync(self, options)
    }

    fn install_instructions(&self) -> &str {
        JiraSource::install_instructions(self)
    }
}

// Implement DataSource for ConfluenceSource
impl DataSource for ConfluenceSource {
    fn source_type(&self) -> SourceType {
        ConfluenceSource::source_type(self)
    }

    fn name(&self) -> &str {
        ConfluenceSource::name(self)
    }

    fn check_available(&self) -> Result<bool, SourceError> {
        ConfluenceSource::check_available(self)
    }

    fn sync(&self, options: SyncOptions) -> Result<Vec<Document>, SourceError> {
        ConfluenceSource::sync(self, options)
    }

    fn install_instructions(&self) -> &str {
        ConfluenceSource::install_instructions(self)
    }
}

// Implement DataSource for FigmaSource
impl DataSource for FigmaSource {
    fn source_type(&self) -> SourceType {
        FigmaSource::source_type(self)
    }

    fn name(&self) -> &str {
        FigmaSource::name(self)
    }

    fn check_available(&self) -> Result<bool, SourceError> {
        FigmaSource::check_available(self)
    }

    fn sync(&self, options: SyncOptions) -> Result<Vec<Document>, SourceError> {
        FigmaSource::sync(self, options)
    }

    fn install_instructions(&self) -> &str {
        FigmaSource::install_instructions(self)
    }
}

/// Get a data source by type.
pub fn get_data_source(source_type: SourceType) -> Option<Box<dyn DataSource>> {
    match source_type {
        SourceType::Jira => Some(Box::new(JiraSource::new())),
        SourceType::Confluence => Some(Box::new(ConfluenceSource::new())),
        SourceType::Figma => Some(Box::new(FigmaSource::new())),
        _ => None,
    }
}
