use anyhow::{Context, Result};

use crate::models::DocumentChunk;
use crate::services::{EmbeddingClient, VectorStoreClient};

pub async fn process_batch(
    embedding_client: &EmbeddingClient,
    vector_client: &VectorStoreClient,
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

    vector_client
        .upsert_points(std::mem::take(chunks))
        .await
        .context("failed to store chunks")?;

    Ok(())
}
