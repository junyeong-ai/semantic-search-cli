//! Vector store abstraction layer.
//!
//! This module provides a trait-based abstraction over different vector store backends
//! (Qdrant, PostgreSQL/pgvector) allowing seamless switching based on configuration.

mod pgvector;
mod qdrant;

pub use pgvector::PgVectorBackend;
pub use qdrant::QdrantBackend;

use async_trait::async_trait;

use crate::error::VectorStoreError;
use crate::models::{
    DocumentChunk, EmbeddingConfig, SearchResult, SourceType, Tag, VectorDriver, VectorStoreConfig,
};

/// Default embedding dimension (Qwen3-Embedding-0.6B produces 1024-dimensional vectors)
/// This is used when no embedding config is provided
pub const DEFAULT_EMBEDDING_DIM: u64 = 1024;

/// Embedding dimension - alias for backward compatibility
pub const EMBEDDING_DIM: u64 = DEFAULT_EMBEDDING_DIM;

/// Collection/table information
#[derive(Debug, Clone)]
pub struct CollectionInfo {
    pub points_count: u64,
}

/// Abstract trait for vector store operations.
///
/// All vector store backends must implement this trait to enable
/// backend-agnostic vector operations throughout the application.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Check if the vector store is healthy and accessible.
    async fn health_check(&self) -> Result<bool, VectorStoreError>;

    /// Get information about the current collection/table.
    /// Returns None if the collection doesn't exist.
    async fn get_collection_info(&self) -> Result<Option<CollectionInfo>, VectorStoreError>;

    /// Create the collection/table if it doesn't exist.
    async fn create_collection(&self) -> Result<(), VectorStoreError>;

    /// Insert or update document chunks with their embeddings.
    async fn upsert_points(&self, chunks: Vec<DocumentChunk>) -> Result<(), VectorStoreError>;

    /// Search for similar vectors with optional filtering.
    async fn search(
        &self,
        query_vector: Vec<f32>,
        limit: u64,
        tags: &[Tag],
        source_types: &[SourceType],
        min_score: Option<f32>,
    ) -> Result<Vec<SearchResult>, VectorStoreError>;

    /// Delete points by matching tags.
    async fn delete_by_tags(&self, tags: &[Tag]) -> Result<(), VectorStoreError>;

    /// Delete points by document IDs.
    async fn delete_by_document_ids(&self, document_ids: &[String])
    -> Result<(), VectorStoreError>;

    /// Clear all points from the collection.
    async fn clear_collection(&self) -> Result<(), VectorStoreError>;

    /// Delete points by source type.
    async fn delete_by_source_type(&self, source_type: SourceType) -> Result<(), VectorStoreError>;

    /// List all unique tags with their counts.
    async fn list_all_tags(&self) -> Result<Vec<(String, u64)>, VectorStoreError>;

    /// Get the collection/table name.
    fn collection(&self) -> &str;
}

/// Create a vector store backend based on configuration.
///
/// This is the main factory function that returns the appropriate backend
/// implementation based on the configuration.
pub async fn create_backend(
    config: &VectorStoreConfig,
) -> Result<Box<dyn VectorStore>, VectorStoreError> {
    create_backend_with_dimension(config, DEFAULT_EMBEDDING_DIM).await
}

/// Create a vector store backend with custom embedding dimension.
///
/// Use this when you need to specify a custom embedding dimension from the embedding config.
pub async fn create_backend_with_dimension(
    config: &VectorStoreConfig,
    embedding_dim: u64,
) -> Result<Box<dyn VectorStore>, VectorStoreError> {
    match config.driver {
        VectorDriver::Qdrant => {
            let backend = QdrantBackend::new(config, embedding_dim)?;
            Ok(Box::new(backend))
        }
        VectorDriver::PostgreSQL => {
            let backend = PgVectorBackend::new(config, embedding_dim).await?;
            Ok(Box::new(backend))
        }
    }
}

/// Create a vector store backend with embedding configuration.
///
/// This convenience function extracts the dimension from the embedding config.
pub async fn create_backend_with_embedding_config(
    vector_config: &VectorStoreConfig,
    embedding_config: &EmbeddingConfig,
) -> Result<Box<dyn VectorStore>, VectorStoreError> {
    create_backend_with_dimension(vector_config, u64::from(embedding_config.dimension)).await
}

/// Create a vector store backend with default configuration.
pub async fn create_default_backend() -> Result<Box<dyn VectorStore>, VectorStoreError> {
    create_backend(&VectorStoreConfig::default()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_dim() {
        assert_eq!(EMBEDDING_DIM, 1024);
    }
}
