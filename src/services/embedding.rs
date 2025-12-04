//! Embedding client for generating text embeddings.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::error::EmbeddingError;
use crate::models::EmbeddingConfig;

/// Instruction type for embedding generation.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstructionType {
    /// For indexing documents
    Document,
    /// For search queries
    Query,
}

/// Request body for the /embed endpoint.
#[derive(Debug, Serialize)]
struct EmbedRequest {
    inputs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncate: Option<bool>,
    instruction_type: InstructionType,
}

/// Response from the /embed endpoint.
#[derive(Debug, Deserialize)]
struct EmbedResponse(Vec<Vec<f32>>);

/// Health response from the /health endpoint.
#[derive(Debug, Deserialize)]
pub struct HealthResponse {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub model_id: Option<String>,
}

/// Client for interacting with the embedding server.
#[derive(Debug, Clone)]
pub struct EmbeddingClient {
    client: Client,
    base_url: String,
    batch_size: usize,
}

impl EmbeddingClient {
    /// Create a new embedding client with the given configuration.
    pub fn new(config: &EmbeddingConfig) -> Result<Self, EmbeddingError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .map_err(|e| EmbeddingError::ConnectionError(e.to_string()))?;

        Ok(Self {
            client,
            base_url: config.url.trim_end_matches('/').to_string(),
            batch_size: config.batch_size as usize,
        })
    }

    /// Create a client with default configuration.
    pub fn with_defaults() -> Result<Self, EmbeddingError> {
        Self::new(&EmbeddingConfig::default())
    }

    /// Check if the embedding server is healthy and ready.
    pub async fn health_check(&self) -> Result<HealthResponse, EmbeddingError> {
        let url = format!("{}/health", self.base_url);
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| EmbeddingError::ConnectionError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(EmbeddingError::ServerError(format!(
                "health check failed with status: {}",
                response.status()
            )));
        }

        // Server may return an empty body on health check
        let text = response.text().await.unwrap_or_default();
        if text.is_empty() {
            return Ok(HealthResponse {
                status: Some("healthy".to_string()),
                model_id: None,
            });
        }

        serde_json::from_str(&text)
            .map_err(|e| {
                // If we can't parse it but got 200, assume healthy
                if text.contains("healthy") || text.is_empty() {
                    return EmbeddingError::InvalidResponse("healthy".to_string());
                }
                EmbeddingError::InvalidResponse(e.to_string())
            })
            .or_else(|e| {
                if matches!(e, EmbeddingError::InvalidResponse(ref s) if s == "healthy") {
                    Ok(HealthResponse {
                        status: Some("healthy".to_string()),
                        model_id: None,
                    })
                } else {
                    Err(e)
                }
            })
    }

    /// Generate embeddings for documents (for indexing).
    /// Documents don't need instruction prefix.
    pub async fn embed_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        self.embed_batch_with_type(texts, InstructionType::Document)
            .await
    }

    /// Generate embedding for a query (for searching).
    /// Queries get instruction prefix for better retrieval.
    pub async fn embed_query(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let embeddings = self
            .embed_batch_with_type(vec![text.to_string()], InstructionType::Query)
            .await?;
        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingError::InvalidResponse("empty embedding response".to_string()))
    }

    /// Generate embeddings for a batch of texts with specified instruction type.
    async fn embed_batch_with_type(
        &self,
        texts: Vec<String>,
        instruction_type: InstructionType,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(self.batch_size) {
            let embeddings = self
                .embed_single_batch(chunk.to_vec(), instruction_type)
                .await?;
            all_embeddings.extend(embeddings);
        }

        Ok(all_embeddings)
    }

    /// Internal method to embed a single batch.
    async fn embed_single_batch(
        &self,
        texts: Vec<String>,
        instruction_type: InstructionType,
    ) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        let url = format!("{}/embed", self.base_url);
        let request = EmbedRequest {
            inputs: texts,
            truncate: Some(true),
            instruction_type,
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    EmbeddingError::Timeout
                } else {
                    EmbeddingError::RequestError(e)
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(EmbeddingError::ServerError(format!(
                "status {}: {}",
                status, body
            )));
        }

        let embed_response: EmbedResponse = response
            .json()
            .await
            .map_err(|e| EmbeddingError::InvalidResponse(e.to_string()))?;

        Ok(embed_response.0)
    }

    /// Get the base URL of the embedding server.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let config = EmbeddingConfig::default();
        let client = EmbeddingClient::new(&config);
        assert!(client.is_ok());
    }

    #[test]
    fn test_base_url_trimming() {
        let config = EmbeddingConfig {
            url: "http://localhost:11411/".to_string(),
            ..Default::default()
        };
        let client = EmbeddingClient::new(&config).unwrap();
        assert_eq!(client.base_url(), "http://localhost:11411");
    }
}
