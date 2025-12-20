use crate::client::DaemonClient;
use crate::error::EmbeddingError;
use crate::models::Config;

pub struct EmbeddingClient {
    client: DaemonClient,
    batch_size: usize,
}

impl EmbeddingClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: DaemonClient::new(config),
            batch_size: config.embedding.batch_size as usize,
        }
    }

    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(self.batch_size) {
            let embeddings = self
                .client
                .embed(chunk.to_vec(), false)
                .await
                .map_err(EmbeddingError::DaemonError)?;
            all_embeddings.extend(embeddings);
        }

        Ok(all_embeddings)
    }

    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let embeddings = self
            .client
            .embed(vec![text.to_string()], true)
            .await
            .map_err(EmbeddingError::DaemonError)?;

        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingError::InvalidResponse("empty response".to_string()))
    }

    pub fn is_daemon_running(&self) -> bool {
        self.client.is_running()
    }
}
