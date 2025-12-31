use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use super::search::OutputFormat;

pub const DEFAULT_QDRANT_URL: &str = "http://localhost:16334";
pub const DEFAULT_COLLECTION: &str = "semantic_search";
pub const DEFAULT_EMBEDDING_MODEL: &str = "JunyeongAI/qwen3-embedding-0.6b-onnx";
pub const DEFAULT_EMBEDDING_DIMENSION: u32 = 1024;
pub const DEFAULT_MAX_TOKENS: u32 = 2048;
pub const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 600;
pub const DEFAULT_METRICS_RETENTION_DAYS: u32 = 30;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigSource {
    #[default]
    Default,
    Global,
    Project,
    Env,
}

impl fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigSource::Default => write!(f, "default"),
            ConfigSource::Global => write!(f, "global"),
            ConfigSource::Project => write!(f, "project"),
            ConfigSource::Env => write!(f, "env"),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConfigSources {
    pub embedding_model_id: ConfigSource,
    pub embedding_dimension: ConfigSource,
    pub embedding_batch_size: ConfigSource,
    pub embedding_max_tokens: ConfigSource,
    pub vector_store_driver: ConfigSource,
    pub vector_store_url: ConfigSource,
    pub vector_store_collection: ConfigSource,
    pub vector_store_api_key: ConfigSource,
    pub indexing_chunk_size: ConfigSource,
    pub indexing_chunk_overlap: ConfigSource,
    pub indexing_exclude_patterns: ConfigSource,
    pub indexing_max_file_size: ConfigSource,
    pub search_default_limit: ConfigSource,
    pub search_default_format: ConfigSource,
    pub daemon_idle_timeout: ConfigSource,
    pub daemon_auto_start: ConfigSource,
    pub metrics_enabled: ConfigSource,
    pub metrics_retention_days: ConfigSource,
}

#[derive(Debug, Clone, Default)]
pub struct ResolvedConfig {
    pub config: Config,
    pub sources: ConfigSources,
    pub project_path: Option<PathBuf>,
    pub global_path: Option<PathBuf>,
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
    const CONFIG_DIR: &'static str = ".ssearch";
    const CONFIG_FILE: &'static str = "config.toml";

    pub fn global_path() -> Option<PathBuf> {
        std::env::var("XDG_CONFIG_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("HOME")
                    .ok()
                    .map(|h| PathBuf::from(h).join(".config"))
            })
            .map(|p| p.join("ssearch").join(Self::CONFIG_FILE))
    }

    pub fn find_project_config() -> Option<PathBuf> {
        let mut current = std::env::current_dir().ok()?;
        loop {
            let config_path = current.join(Self::CONFIG_DIR).join(Self::CONFIG_FILE);
            if config_path.exists() {
                return Some(config_path);
            }
            if !current.pop() {
                return None;
            }
        }
    }

    pub fn project_config_dir() -> Option<PathBuf> {
        std::env::current_dir()
            .ok()
            .map(|p| p.join(Self::CONFIG_DIR))
    }

    pub fn load() -> Result<ResolvedConfig, crate::error::ConfigError> {
        dotenvy::dotenv().ok();

        let mut config = Config::default();
        let mut sources = ConfigSources::default();

        let global_path = Self::global_path();
        if let Some(ref path) = global_path
            && path.exists()
        {
            let partial = Self::load_partial(path)?;
            Self::merge_partial(&mut config, &mut sources, &partial, ConfigSource::Global);
        }

        let project_path = Self::find_project_config();
        if let Some(ref path) = project_path {
            let partial = Self::load_partial(path)?;
            Self::merge_partial(&mut config, &mut sources, &partial, ConfigSource::Project);
        }

        Self::apply_env_overrides(&mut config, &mut sources);

        Ok(ResolvedConfig {
            config,
            sources,
            project_path,
            global_path: global_path.filter(|p| p.exists()),
        })
    }

    fn load_partial(path: &Path) -> Result<PartialConfig, crate::error::ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let partial: PartialConfig = toml::from_str(&content)?;
        Ok(partial)
    }

    fn merge_partial(
        config: &mut Config,
        sources: &mut ConfigSources,
        partial: &PartialConfig,
        source: ConfigSource,
    ) {
        if let Some(ref emb) = partial.embedding {
            if let Some(ref v) = emb.model_id {
                config.embedding.model_id = v.clone();
                sources.embedding_model_id = source;
            }
            if let Some(v) = emb.dimension {
                config.embedding.dimension = v;
                sources.embedding_dimension = source;
            }
            if let Some(v) = emb.batch_size {
                config.embedding.batch_size = v;
                sources.embedding_batch_size = source;
            }
            if let Some(v) = emb.max_tokens {
                config.embedding.max_tokens = v;
                sources.embedding_max_tokens = source;
            }
            if emb.model_path.is_some() {
                config.embedding.model_path = emb.model_path.clone();
            }
        }

        if let Some(ref vs) = partial.vector_store {
            if let Some(v) = vs.driver {
                config.vector_store.driver = v;
                sources.vector_store_driver = source;
            }
            if let Some(ref v) = vs.url {
                config.vector_store.url = v.clone();
                sources.vector_store_url = source;
            }
            if let Some(ref v) = vs.collection {
                config.vector_store.collection = v.clone();
                sources.vector_store_collection = source;
            }
            if vs.api_key.is_some() {
                config.vector_store.api_key = vs.api_key.clone();
                sources.vector_store_api_key = source;
            }
            if vs.schema.is_some() {
                config.vector_store.schema = vs.schema.clone();
            }
            if let Some(v) = vs.pool_max {
                config.vector_store.pool_max = v;
            }
            if let Some(v) = vs.pool_acquire_timeout {
                config.vector_store.pool_acquire_timeout = v;
            }
        }

        if let Some(ref idx) = partial.indexing {
            if let Some(v) = idx.chunk_size {
                config.indexing.chunk_size = v;
                sources.indexing_chunk_size = source;
            }
            if let Some(v) = idx.chunk_overlap {
                config.indexing.chunk_overlap = v;
                sources.indexing_chunk_overlap = source;
            }
            if let Some(ref v) = idx.exclude_patterns {
                config.indexing.exclude_patterns = v.clone();
                sources.indexing_exclude_patterns = source;
            }
            if let Some(v) = idx.max_file_size {
                config.indexing.max_file_size = v;
                sources.indexing_max_file_size = source;
            }
        }

        if let Some(ref s) = partial.search {
            if let Some(v) = s.default_limit {
                config.search.default_limit = v;
                sources.search_default_limit = source;
            }
            if let Some(v) = s.default_format {
                config.search.default_format = v;
                sources.search_default_format = source;
            }
            if s.default_min_score.is_some() {
                config.search.default_min_score = s.default_min_score;
            }
        }

        if let Some(ref d) = partial.daemon {
            if let Some(v) = d.idle_timeout_secs {
                config.daemon.idle_timeout_secs = v;
                sources.daemon_idle_timeout = source;
            }
            if let Some(v) = d.auto_start {
                config.daemon.auto_start = v;
                sources.daemon_auto_start = source;
            }
            if d.socket_path.is_some() {
                config.daemon.socket_path = d.socket_path.clone();
            }
        }

        if let Some(ref m) = partial.metrics {
            if let Some(v) = m.enabled {
                config.metrics.enabled = v;
                sources.metrics_enabled = source;
            }
            if let Some(v) = m.retention_days {
                config.metrics.retention_days = v;
                sources.metrics_retention_days = source;
            }
        }
    }

    fn apply_env_overrides(config: &mut Config, sources: &mut ConfigSources) {
        if let Ok(v) = std::env::var("SSEARCH_EMBEDDING_MODEL") {
            config.embedding.model_id = v;
            sources.embedding_model_id = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_EMBEDDING_DIMENSION")
            && let Ok(dim) = v.parse()
        {
            config.embedding.dimension = dim;
            sources.embedding_dimension = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_BATCH_SIZE")
            && let Ok(size) = v.parse()
        {
            config.embedding.batch_size = size;
            sources.embedding_batch_size = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_MAX_TOKENS")
            && let Ok(tokens) = v.parse()
        {
            config.embedding.max_tokens = tokens;
            sources.embedding_max_tokens = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_VECTOR_DRIVER")
            && let Ok(driver) = v.parse()
        {
            config.vector_store.driver = driver;
            sources.vector_store_driver = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_QDRANT_URL") {
            config.vector_store.url = v;
            sources.vector_store_url = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_COLLECTION") {
            config.vector_store.collection = v;
            sources.vector_store_collection = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_VECTOR_API_KEY") {
            config.vector_store.api_key = Some(v);
            sources.vector_store_api_key = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_CHUNK_SIZE")
            && let Ok(size) = v.parse()
        {
            config.indexing.chunk_size = size;
            sources.indexing_chunk_size = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_CHUNK_OVERLAP")
            && let Ok(overlap) = v.parse()
        {
            config.indexing.chunk_overlap = overlap;
            sources.indexing_chunk_overlap = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_MAX_FILE_SIZE")
            && let Ok(size) = v.parse()
        {
            config.indexing.max_file_size = size;
            sources.indexing_max_file_size = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_DEFAULT_LIMIT")
            && let Ok(limit) = v.parse()
        {
            config.search.default_limit = limit;
            sources.search_default_limit = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_DEFAULT_FORMAT")
            && let Ok(fmt) = v.parse()
        {
            config.search.default_format = fmt;
            sources.search_default_format = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_DAEMON_TIMEOUT")
            && let Ok(timeout) = v.parse()
        {
            config.daemon.idle_timeout_secs = timeout;
            sources.daemon_idle_timeout = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_DAEMON_AUTO_START") {
            config.daemon.auto_start = v.eq_ignore_ascii_case("true") || v == "1";
            sources.daemon_auto_start = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_METRICS_ENABLED") {
            config.metrics.enabled = v.eq_ignore_ascii_case("true") || v == "1";
            sources.metrics_enabled = ConfigSource::Env;
        }
        if let Ok(v) = std::env::var("SSEARCH_METRICS_RETENTION_DAYS")
            && let Ok(days) = v.parse()
        {
            config.metrics.retention_days = days;
            sources.metrics_retention_days = ConfigSource::Env;
        }
    }

    pub fn init_project() -> Result<PathBuf, crate::error::ConfigError> {
        let config_dir = Self::project_config_dir().ok_or_else(|| {
            crate::error::ConfigError::PathError("could not determine project directory".into())
        })?;
        let config_path = config_dir.join(Self::CONFIG_FILE);

        std::fs::create_dir_all(&config_dir)?;

        let partial = PartialConfig {
            indexing: Some(PartialIndexingConfig {
                exclude_patterns: Some(vec![
                    "**/node_modules/**".into(),
                    "**/target/**".into(),
                    "**/.git/**".into(),
                ]),
                ..Default::default()
            }),
            search: Some(PartialSearchConfig {
                default_limit: Some(10),
                ..Default::default()
            }),
            ..Default::default()
        };

        let content = toml::to_string_pretty(&partial)?;
        std::fs::write(&config_path, content)?;

        Ok(config_path)
    }

    pub fn init_global() -> Result<PathBuf, crate::error::ConfigError> {
        let config_path = Self::global_path().ok_or_else(|| {
            crate::error::ConfigError::PathError("could not determine home directory".into())
        })?;

        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let config = Config::default();
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(&config_path, content)?;

        Ok(config_path)
    }

    pub fn save_partial(
        path: &Path,
        partial: &PartialConfig,
    ) -> Result<(), crate::error::ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(partial)?;
        std::fs::write(path, content)?;
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
        self.socket_path().with_extension("pid")
    }

    pub fn metrics_db_path() -> Option<PathBuf> {
        Self::cache_dir().map(|p| p.join("metrics.db"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialConfig {
    pub embedding: Option<PartialEmbeddingConfig>,
    pub vector_store: Option<PartialVectorStoreConfig>,
    pub indexing: Option<PartialIndexingConfig>,
    pub search: Option<PartialSearchConfig>,
    pub daemon: Option<PartialDaemonConfig>,
    pub metrics: Option<PartialMetricsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialEmbeddingConfig {
    pub model_id: Option<String>,
    pub model_path: Option<PathBuf>,
    pub dimension: Option<u32>,
    pub batch_size: Option<u32>,
    pub max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialVectorStoreConfig {
    pub driver: Option<VectorDriver>,
    pub url: Option<String>,
    pub collection: Option<String>,
    pub schema: Option<String>,
    pub api_key: Option<String>,
    pub pool_max: Option<u32>,
    pub pool_acquire_timeout: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialIndexingConfig {
    pub exclude_patterns: Option<Vec<String>>,
    pub max_file_size: Option<u64>,
    pub chunk_size: Option<u32>,
    pub chunk_overlap: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialSearchConfig {
    pub default_limit: Option<u32>,
    pub default_format: Option<OutputFormat>,
    pub default_min_score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialDaemonConfig {
    pub idle_timeout_secs: Option<u64>,
    pub auto_start: Option<bool>,
    pub socket_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PartialMetricsConfig {
    pub enabled: Option<bool>,
    pub retention_days: Option<u32>,
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

    /// Maximum tokens per text for embedding (truncation limit)
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
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

fn default_max_tokens() -> u32 {
    DEFAULT_MAX_TOKENS
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model_id: default_embedding_model(),
            model_path: None,
            dimension: default_embedding_dimension(),
            batch_size: default_batch_size(),
            max_tokens: default_max_tokens(),
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
        "**/node_modules/**".into(),
        "**/target/**".into(),
        "**/.git/**".into(),
        "**/dist/**".into(),
        "**/build/**".into(),
        "**/__pycache__/**".into(),
        "**/.venv/**".into(),
        "**/vendor/**".into(),
        "**/*.min.js".into(),
        "**/*.min.css".into(),
        "**/package-lock.json".into(),
        "**/yarn.lock".into(),
        "**/pnpm-lock.yaml".into(),
        "**/Cargo.lock".into(),
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

    #[test]
    fn test_partial_config_merge() {
        let mut config = Config::default();
        let mut sources = ConfigSources::default();

        let partial = PartialConfig {
            embedding: Some(PartialEmbeddingConfig {
                model_id: Some("custom-model".into()),
                ..Default::default()
            }),
            ..Default::default()
        };

        Config::merge_partial(&mut config, &mut sources, &partial, ConfigSource::Project);

        assert_eq!(config.embedding.model_id, "custom-model");
        assert_eq!(sources.embedding_model_id, ConfigSource::Project);
    }

    #[test]
    fn test_config_source_display() {
        assert_eq!(format!("{}", ConfigSource::Default), "default");
        assert_eq!(format!("{}", ConfigSource::Global), "global");
        assert_eq!(format!("{}", ConfigSource::Project), "project");
        assert_eq!(format!("{}", ConfigSource::Env), "env");
    }
}
