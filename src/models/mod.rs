mod config;
mod document;
mod search;
mod source;
mod tag;

pub use config::{
    Config, DEFAULT_COLLECTION, DEFAULT_EMBEDDING_DIMENSION, DEFAULT_EMBEDDING_MODEL,
    DEFAULT_IDLE_TIMEOUT_SECS, DEFAULT_METRICS_RETENTION_DAYS, DEFAULT_QDRANT_URL, DaemonConfig,
    EmbeddingConfig, IndexingConfig, MetricsConfig, SearchConfig, VectorDriver, VectorStoreConfig,
};
pub use document::{Document, DocumentChunk, DocumentMetadata};
pub use search::{OutputFormat, SearchQuery, SearchResult, SearchResults};
pub use source::{Source, SourceType};
pub use tag::{Tag, parse_tags};
