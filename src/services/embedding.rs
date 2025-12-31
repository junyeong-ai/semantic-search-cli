use crate::client::DaemonClient;
use crate::error::EmbeddingError;
use crate::models::Config;

/// Client for generating embeddings via the daemon service.
/// Batch management is handled by callers (source.rs, index.rs).
pub struct EmbeddingClient {
    client: DaemonClient,
}

impl EmbeddingClient {
    pub fn new(config: &Config) -> Self {
        Self {
            client: DaemonClient::new(config),
        }
    }

    /// Embed a batch of texts. Callers should manage batch sizes.
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        self.client
            .embed(texts, false)
            .await
            .map_err(EmbeddingError::DaemonError)
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
