use serde::{Deserialize, Serialize};

use crate::services::MetricsSummary;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Ping,
    Shutdown,
    Status,
    Embed(EmbedRequest),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedRequest {
    pub texts: Vec<String>,
    pub is_query: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Response {
    Pong,
    ShutdownAck,
    Status(StatusResponse),
    Embed(EmbedResponse),
    Error(ErrorResponse),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub running: bool,
    pub embedding_model: String,
    pub idle_secs: u64,
    pub requests_served: u64,
    pub metrics: Option<MetricsSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbedResponse {
    pub embeddings: Vec<Vec<f32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
}

impl Response {
    pub fn error(message: impl Into<String>) -> Self {
        Response::Error(ErrorResponse {
            message: message.into(),
        })
    }
}

pub fn encode_message(msg: &impl Serialize) -> Result<Vec<u8>, serde_json::Error> {
    let json = serde_json::to_vec(msg)?;
    let len = (json.len() as u32).to_be_bytes();
    let mut buf = Vec::with_capacity(4 + json.len());
    buf.extend_from_slice(&len);
    buf.extend_from_slice(&json);
    Ok(buf)
}

pub fn decode_length(buf: &[u8; 4]) -> usize {
    u32::from_be_bytes(*buf) as usize
}
