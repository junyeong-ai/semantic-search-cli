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

#[derive(Debug, Clone, Default)]
pub struct SyncOptions {
    pub query: Option<String>,
    pub project: Option<String>,
    pub tags: Vec<Tag>,
    pub limit: Option<u32>,
    pub exclude_ancestors: Vec<String>,
}

pub trait DataSource: Send + Sync {
    fn source_type(&self) -> SourceType;
    fn name(&self) -> &str;
    fn check_available(&self) -> Result<bool, SourceError>;
    fn install_instructions(&self) -> &str;
    fn sync(&self, options: SyncOptions) -> Result<Vec<Document>, SourceError>;
}

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

pub fn get_data_source(source_type: SourceType) -> Option<Box<dyn DataSource>> {
    match source_type {
        SourceType::Jira => Some(Box::new(JiraSource::new())),
        SourceType::Confluence => Some(Box::new(ConfluenceSource::new())),
        SourceType::Figma => Some(Box::new(FigmaSource::new())),
        SourceType::Local | SourceType::Other(_) => None,
    }
}
