use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use super::search::OutputFormat;

pub const DEFAULT_QDRANT_URL: &str = "http://localhost:16334";
pub const DEFAULT_COLLECTION: &str = "semantic_search";
pub const DEFAULT_EMBEDDING_MODEL: &str = "JunyeongAI/qwen3-embedding-0.6b-onnx";
pub const DEFAULT_EMBEDDING_DIMENSION: u32 = 1024;
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 600;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VectorDriver {
    #[default]
    Qdrant,
    #[serde(alias = "postgres")]
    PostgreSQL,
}

impl fmt::Display for VectorDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VectorDriver::Qdrant => write!(f, "qdrant"),
            VectorDriver::PostgreSQL => write!(f, "postgresql"),
        }
    }
}

impl FromStr for VectorDriver {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "qdrant" => Ok(VectorDriver::Qdrant),
            "postgresql" | "postgres" | "pg" => Ok(VectorDriver::PostgreSQL),
            _ => Err(format!("unknown vector driver: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub embedding: EmbeddingConfig,

    #[serde(default)]
    pub vector_store: VectorStoreConfig,

    #[serde(default)]
    pub indexing: IndexingConfig,

    #[serde(default)]
    pub search: SearchConfig,

    #[serde(default)]
    pub daemon: DaemonConfig,

    #[serde(default)]
    pub metrics: MetricsConfig,
}

impl Config {
    pub fn config_path() -> Option<PathBuf> {
        // Use XDG Base Directory or ~/.config for all platforms
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|home| PathBuf::from(home).join(".config"))
            })
            .map(|p| p.join("semantic-search-cli").join("config.toml"))
    }

    pub fn load() -> Result<Self, crate::error::ConfigError> {
        if let Some(path) = Self::config_path()
            && path.exists()
        {
            let content = std::fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            return Ok(config);
        }
        Ok(Self::default())
    }

    pub fn save(&self) -> Result<(), crate::error::ConfigError> {
        let path = Self::config_path().ok_or_else(|| {
            crate::error::ConfigError::PathError("could not determine config directory".to_string())
        })?;

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn cache_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|p| p.join(".cache").join("semantic-search-cli"))
    }

    pub fn models_dir() -> Option<PathBuf> {
        Self::cache_dir().map(|p| p.join("models"))
    }

    pub fn socket_path(&self) -> PathBuf {
        self.daemon
            .socket_path
            .clone()
            .unwrap_or_else(default_socket_path)
    }

    pub fn pid_path(&self) -> PathBuf {
        let socket = self.socket_path();
        socket.with_extension("pid")
    }

    pub fn metrics_db_path() -> Option<PathBuf> {
        Self::cache_dir().map(|p| p.join("metrics.db"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default = "default_embedding_model")]
    pub model_id: String,

    #[serde(default)]
    pub model_path: Option<PathBuf>,

    #[serde(default = "default_embedding_dimension")]
    pub dimension: u32,

    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
}

fn default_embedding_model() -> String {
    DEFAULT_EMBEDDING_MODEL.to_string()
}

fn default_embedding_dimension() -> u32 {
    DEFAULT_EMBEDDING_DIMENSION
}

fn default_batch_size() -> u32 {
    8
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_id: default_embedding_model(),
            model_path: None,
            dimension: default_embedding_dimension(),
            batch_size: default_batch_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreConfig {
    #[serde(default)]
    pub driver: VectorDriver,

    #[serde(default = "default_qdrant_url")]
    pub url: String,

    #[serde(default = "default_collection")]
    pub collection: String,

    #[serde(default)]
    pub schema: Option<String>,

    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default = "default_pool_max")]
    pub pool_max: u32,

    #[serde(default = "default_pool_acquire_timeout")]
    pub pool_acquire_timeout: u32,
}

fn default_qdrant_url() -> String {
    DEFAULT_QDRANT_URL.to_string()
}

fn default_collection() -> String {
    DEFAULT_COLLECTION.to_string()
}

fn default_pool_max() -> u32 {
    10
}

fn default_pool_acquire_timeout() -> u32 {
    30
}

impl Default for VectorStoreConfig {
    fn default() -> Self {
        Self {
            driver: VectorDriver::default(),
            url: default_qdrant_url(),
            collection: default_collection(),
            schema: None,
            api_key: None,
            pool_max: default_pool_max(),
            pool_acquire_timeout: default_pool_acquire_timeout(),
        }
    }
}

impl VectorStoreConfig {
    /// Get the fully qualified table name for PostgreSQL.
    /// Returns "schema.collection" if schema is set, otherwise just "collection".
    pub fn qualified_table_name(&self) -> String {
        match &self.schema {
            Some(schema) => format!("{}.{}", schema, self.collection),
            None => self.collection.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    #[serde(default = "default_exclude_patterns")]
    pub exclude_patterns: Vec<String>,

    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    #[serde(default = "default_chunk_size")]
    pub chunk_size: u32,

    #[serde(default = "default_chunk_overlap")]
    pub chunk_overlap: u32,
}

fn default_exclude_patterns() -> Vec<String> {
    vec![
        "**/node_modules/**".to_string(),
        "**/target/**".to_string(),
        "**/.git/**".to_string(),
        "**/dist/**".to_string(),
        "**/build/**".to_string(),
        "**/__pycache__/**".to_string(),
        "**/.venv/**".to_string(),
        "**/vendor/**".to_string(),
        "**/*.min.js".to_string(),
        "**/*.min.css".to_string(),
        "**/package-lock.json".to_string(),
        "**/yarn.lock".to_string(),
        "**/pnpm-lock.yaml".to_string(),
        "**/Cargo.lock".to_string(),
    ]
}

fn default_max_file_size() -> u64 {
    10 * 1024 * 1024
}

fn default_chunk_size() -> u32 {
    6000
}

fn default_chunk_overlap() -> u32 {
    500
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            exclude_patterns: default_exclude_patterns(),
            max_file_size: default_max_file_size(),
            chunk_size: default_chunk_size(),
            chunk_overlap: default_chunk_overlap(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchConfig {
    #[serde(default = "default_limit")]
    pub default_limit: u32,

    #[serde(default)]
    pub default_format: OutputFormat,

    #[serde(default)]
    pub default_min_score: Option<f32>,
}

fn default_limit() -> u32 {
    10
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            default_limit: default_limit(),
            default_format: OutputFormat::Text,
            default_min_score: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,

    #[serde(default = "default_auto_start")]
    pub auto_start: bool,

    #[serde(default)]
    pub socket_path: Option<PathBuf>,
}

fn default_idle_timeout() -> u64 {
    DEFAULT_IDLE_TIMEOUT_SECS
}

fn default_auto_start() -> bool {
    true
}

fn default_socket_path() -> PathBuf {
    std::env::temp_dir().join("ssearch.sock")
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            idle_timeout_secs: default_idle_timeout(),
            auto_start: default_auto_start(),
            socket_path: None,
        }
    }
}

pub const DEFAULT_METRICS_RETENTION_DAYS: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    #[serde(default = "default_metrics_enabled")]
    pub enabled: bool,

    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_metrics_enabled() -> bool {
    true
}

fn default_retention_days() -> u32 {
    DEFAULT_METRICS_RETENTION_DAYS
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: default_metrics_enabled(),
            retention_days: default_retention_days(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.embedding.model_id, DEFAULT_EMBEDDING_MODEL);
        assert_eq!(config.vector_store.url, DEFAULT_QDRANT_URL);
    }

    #[test]
    fn test_daemon_config_default() {
        let config = DaemonConfig::default();
        assert_eq!(config.idle_timeout_secs, DEFAULT_IDLE_TIMEOUT_SECS);
        assert!(config.auto_start);
    }
}
