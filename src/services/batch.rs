use anyhow::{Context, Result};

use crate::models::DocumentChunk;
use crate::services::{EmbeddingClient, VectorStore};

/// Process a batch of document chunks: generate embeddings and store in vector store.
///
/// This function accepts any backend that implements the VectorStore trait,
/// enabling backend-agnostic batch processing.
pub async fn process_batch<V: VectorStore + ?Sized>(
    embedding_client: &EmbeddingClient,
    vector_store: &V,
    chunks: &mut Vec<DocumentChunk>,
    texts: &mut Vec<String>,
) -> Result<()> {
    if texts.is_empty() {
        return Ok(());
    }

    let embeddings = embedding_client
        .embed_batch(std::mem::take(texts))
        .await
        .context("failed to generate embeddings")?;

    for (chunk, embedding) in chunks.iter_mut().zip(embeddings.into_iter()) {
        chunk.dense_vector = embedding;
    }

    vector_store
        .upsert_points(std::mem::take(chunks))
        .await
        .context("failed to store chunks")?;

    Ok(())
}
