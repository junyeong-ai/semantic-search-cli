use serde::{Deserialize, Serialize};

use super::search::OutputFormat;

pub const DEFAULT_EMBEDDING_URL: &str = "http://localhost:11411";
pub const DEFAULT_QDRANT_URL: &str = "http://localhost:16334";
pub const DEFAULT_COLLECTION: &str = "semantic_search";

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
}

impl Config {
    pub fn config_path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|p| p.join("semantic-search-cli").join("config.toml"))
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    #[serde(default = "default_embedding_url")]
    pub url: String,

    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
}

fn default_embedding_url() -> String {
    DEFAULT_EMBEDDING_URL.to_string()
}

fn default_timeout() -> u64 {
    120
}

fn default_batch_size() -> u32 {
    8
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            url: default_embedding_url(),
            timeout_secs: default_timeout(),
            batch_size: default_batch_size(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorStoreConfig {
    #[serde(default = "default_qdrant_url")]
    pub url: String,

    #[serde(default = "default_collection")]
    pub collection: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

fn default_qdrant_url() -> String {
    DEFAULT_QDRANT_URL.to_string()
}

fn default_collection() -> String {
    DEFAULT_COLLECTION.to_string()
}

impl Default for VectorStoreConfig {
    fn default() -> Self {
        Self {
            url: default_qdrant_url(),
            collection: default_collection(),
            api_key: None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.embedding.url, DEFAULT_EMBEDDING_URL);
        assert_eq!(config.vector_store.url, DEFAULT_QDRANT_URL);
        assert_eq!(config.vector_store.collection, DEFAULT_COLLECTION);
    }

    #[test]
    fn test_config_path() {
        let path = Config::config_path();
        assert!(path.is_some());
    }

    #[test]
    fn test_embedding_config_default() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.timeout_secs, 120);
        assert_eq!(config.batch_size, 8);
    }

    #[test]
    fn test_indexing_config_default() {
        let config = IndexingConfig::default();
        assert!(!config.exclude_patterns.is_empty());
        assert!(config.max_file_size > 0);
    }
}
