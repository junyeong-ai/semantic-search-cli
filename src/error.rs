use thiserror::Error;

use crate::utils::retry::Retryable;

#[derive(Debug, Error)]
pub enum TagError {
    #[error("invalid tag key: {0}")]
    InvalidKey(String),

    #[error("invalid tag value: {0}")]
    InvalidValue(String),

    #[error("tag parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("model not found: {0}")]
    NotFound(String),

    #[error("model load error: {0}")]
    LoadError(String),

    #[error("tokenizer error: {0}")]
    TokenizerError(String),

    #[error("inference error: {0}")]
    InferenceError(String),

    #[error("download error: {0}")]
    DownloadError(String),
}

#[derive(Debug, Error)]
pub enum DaemonError {
    #[error("daemon not running")]
    NotRunning,

    #[error("daemon already running")]
    AlreadyRunning,

    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    #[error("socket error: {0}")]
    SocketError(String),

    #[error("protocol error: {0}")]
    ProtocolError(String),

    #[error("spawn error: {0}")]
    SpawnError(String),

    #[error("timeout")]
    Timeout,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl Retryable for DaemonError {
    fn is_retryable(&self) -> bool {
        matches!(
            self,
            DaemonError::ConnectionFailed(_) | DaemonError::Timeout | DaemonError::NotRunning
        )
    }
}

#[derive(Debug, Error)]
pub enum EmbeddingError {
    #[error("daemon error: {0}")]
    DaemonError(#[from] DaemonError),

    #[error("model error: {0}")]
    ModelError(#[from] ModelError),

    #[error("invalid response: {0}")]
    InvalidResponse(String),
}

impl Retryable for EmbeddingError {
    fn is_retryable(&self) -> bool {
        match self {
            EmbeddingError::DaemonError(e) => e.is_retryable(),
            _ => false,
        }
    }
}

#[derive(Debug, Error)]
pub enum VectorStoreError {
    #[error("connection error: {0}")]
    ConnectionError(String),

    #[error("collection error: {0}")]
    CollectionError(String),

    #[error("upsert error: {0}")]
    UpsertError(String),

    #[error("search error: {0}")]
    SearchError(String),

    #[error("delete error: {0}")]
    DeleteError(String),

    #[error("client error: {0}")]
    ClientError(String),

    #[error("PostgreSQL error: {0}")]
    PostgresError(String),

    #[error("pgvector extension not installed: {0}")]
    PgVectorExtensionError(String),

    #[error("unsupported backend: {0}")]
    UnsupportedBackend(String),
}

impl Retryable for VectorStoreError {
    fn is_retryable(&self) -> bool {
        match self {
            VectorStoreError::ConnectionError(_) => true,
            VectorStoreError::PostgresError(msg) => {
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("timeout")
                    || msg_lower.contains("connection")
                    || msg_lower.contains("unavailable")
            }
            VectorStoreError::PgVectorExtensionError(_)
            | VectorStoreError::UnsupportedBackend(_) => false,
            VectorStoreError::CollectionError(msg)
            | VectorStoreError::UpsertError(msg)
            | VectorStoreError::SearchError(msg)
            | VectorStoreError::DeleteError(msg)
            | VectorStoreError::ClientError(msg) => {
                let msg_lower = msg.to_lowercase();
                msg_lower.contains("timeout") || msg_lower.contains("connection")
            }
        }
    }
}

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

#[derive(Debug, Error)]
pub enum SearchError {
    #[error("embedding error: {0}")]
    EmbeddingError(#[from] EmbeddingError),

    #[error("vector store error: {0}")]
    VectorStoreError(#[from] VectorStoreError),

    #[error("invalid query: {0}")]
    InvalidQuery(String),
}

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

    #[error("daemon error: {0}")]
    Daemon(#[from] DaemonError),

    #[error("model error: {0}")]
    Model(#[from] ModelError),

    #[error("{0}")]
    Other(String),
}
