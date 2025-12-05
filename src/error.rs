//! Error types for the semantic search CLI.

use thiserror::Error;

use crate::utils::retry::Retryable;

/// Errors related to tag parsing and validation.
#[derive(Debug, Error)]
pub enum TagError {
    #[error("invalid tag key: {0}")]
    InvalidKey(String),

    #[error("invalid tag value: {0}")]
    InvalidValue(String),

    #[error("tag parse error: {0}")]
    ParseError(String),
}

/// Errors related to embedding operations.
#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("failed to connect to embedding server: {0}")]
    ConnectionError(String),

    #[error("embedding server error: {0}")]
    ServerError(String),

    #[error("embedding request failed: {0}")]
    RequestError(#[from] reqwest::Error),

    #[error("invalid embedding response: {0}")]
    InvalidResponse(String),

    #[error("embedding timeout")]
    Timeout,
}

impl Retryable for EmbeddingError {
    fn is_retryable(&self) -> bool {
        match self {
            // Connection and timeout errors are retryable
            EmbeddingError::ConnectionError(_) | EmbeddingError::Timeout => true,
            // Server errors might be transient (e.g., 503 Service Unavailable)
            EmbeddingError::ServerError(msg) => {
                msg.contains("503")
                    || msg.contains("502")
                    || msg.contains("504")
                    || msg.contains("429")
                    || msg.to_lowercase().contains("unavailable")
                    || msg.to_lowercase().contains("too many requests")
            }
            // Request errors depend on the underlying cause
            EmbeddingError::RequestError(e) => e.is_timeout() || e.is_connect(),
            // Invalid responses are not retryable
            EmbeddingError::InvalidResponse(_) => false,
        }
    }
}

/// Errors related to vector store operations.
#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("failed to connect to Qdrant: {0}")]
    ConnectionError(String),

    #[error("collection error: {0}")]
    CollectionError(String),

    #[error("upsert error: {0}")]
    UpsertError(String),

    #[error("search error: {0}")]
    SearchError(String),

    #[error("delete error: {0}")]
    DeleteError(String),

    #[error("Qdrant client error: {0}")]
    ClientError(String),
}

impl Retryable for VectorStoreError {
    fn is_retryable(&self) -> bool {
        match self {
            // Connection errors are always retryable
            VectorStoreError::ConnectionError(_) => true,
            // Other errors might be transient
            VectorStoreError::CollectionError(msg)
            | VectorStoreError::UpsertError(msg)
            | VectorStoreError::SearchError(msg)
            | VectorStoreError::DeleteError(msg)
            | VectorStoreError::ClientError(msg) => {
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("timeout")
                    || msg_lower.contains("connection")
                    || msg_lower.contains("unavailable")
                    || msg_lower.contains("too many")
            }
        }
    }
}

/// Errors related to indexing operations.
#[derive(Debug, Error)]
pub enum IndexError {
    #[error("file read error: {0}")]
    FileReadError(String),

    #[error("directory walk error: {0}")]
    WalkError(String),

    #[error("chunking error: {0}")]
    ChunkError(String),

    #[error("embedding error: {0}")]
    EmbeddingError(#[from] EmbeddingError),

    #[error("vector store error: {0}")]
    VectorStoreError(#[from] VectorStoreError),

    #[error("no files found")]
    NoFilesFound,
}

/// Errors related to configuration.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    TomlParseError(#[from] toml::de::Error),

    #[error("TOML serialize error: {0}")]
    TomlSerializeError(#[from] toml::ser::Error),

    #[error("path error: {0}")]
    PathError(String),

    #[error("validation error: {0}")]
    ValidationError(String),
}

/// Errors related to data source operations.
#[derive(Debug, Error)]
pub enum SourceError {
    #[error("CLI not found: {0}")]
    CliNotFound(String),

    #[error("CLI execution error: {0}")]
    ExecutionError(String),

    #[error("parse error: {0}")]
    ParseError(String),

    #[error("sync error: {0}")]
    SyncError(String),

    #[error("unsupported source type: {0}")]
    UnsupportedSource(String),
}

/// Errors related to import operations.
#[derive(Debug, Error)]
pub enum ImportError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    JsonParseError(#[from] serde_json::Error),

    #[error("validation error: {0}")]
    ValidationError(String),

    #[error("no documents found")]
    NoDocuments,
}

/// Errors related to search operations.
#[derive(Debug, Error)]
pub enum SearchError {
    #[error("embedding error: {0}")]
    EmbeddingError(#[from] EmbeddingError),

    #[error("vector store error: {0}")]
    VectorStoreError(#[from] VectorStoreError),

    #[error("invalid query: {0}")]
    InvalidQuery(String),
}

/// Application-level errors that wrap domain errors.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("tag error: {0}")]
    Tag(#[from] TagError),

    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    #[error("index error: {0}")]
    Index(#[from] IndexError),

    #[error("search error: {0}")]
    Search(#[from] SearchError),

    #[error("source error: {0}")]
    Source(#[from] SourceError),

    #[error("import error: {0}")]
    Import(#[from] ImportError),

    #[error("infrastructure not running: {0}")]
    InfrastructureError(String),

    #[error("{0}")]
    Other(String),
}
