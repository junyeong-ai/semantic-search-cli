pub mod embedding;
pub mod protocol;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::RwLock;

use crate::error::ModelError;
use crate::models::Config;
use crate::server::embedding::{EmbeddingModel, SharedEmbeddingModel};
use crate::server::protocol::{
    EmbedResponse, Request, Response, StatusResponse, decode_length, encode_message,
};
use crate::services::MetricsStore;

pub use embedding::EmbeddingModel as OnnxEmbeddingModel;

pub struct DaemonServer {
    config: Config,
    socket_path: PathBuf,
    embedding_model: SharedEmbeddingModel,
    metrics: Option<MetricsStore>,
    last_request: Arc<RwLock<Instant>>,
    requests_served: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
}

impl DaemonServer {
    pub async fn new(config: Config) -> Result<Self, ModelError> {
        let socket_path = config.socket_path();
        let models_dir = Config::models_dir().ok_or_else(|| {
            ModelError::NotFound("could not determine models directory".to_string())
        })?;

        eprintln!("Loading embedding model: {}", config.embedding.model_id);
        let embedding_dir = config
            .embedding
            .model_path
            .clone()
            .unwrap_or_else(|| models_dir.join(model_dir_name(&config.embedding.model_id)));
        let embedding_model = Arc::new(EmbeddingModel::load(&config.embedding, &embedding_dir)?);
        eprintln!(
            "Embedding model loaded (dim={})",
            embedding_model.dimension()
        );

        let metrics = if config.metrics.enabled {
            if let Some(path) = Config::metrics_db_path() {
                match MetricsStore::open(&path) {
                    Ok(store) => {
                        store.cleanup(config.metrics.retention_days);
                        eprintln!(
                            "Metrics enabled (retention: {} days)",
                            config.metrics.retention_days
                        );
                        Some(store)
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to open metrics database: {}", e);
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            config,
            socket_path,
            embedding_model,
            metrics,
            last_request: Arc::new(RwLock::new(Instant::now())),
            requests_served: Arc::new(AtomicU64::new(0)),
            shutdown: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn run(&self) -> Result<(), std::io::Error> {
        if self.socket_path.exists() {
            std::fs::remove_file(&self.socket_path)?;
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        self.write_pid_file()?;

        eprintln!("Daemon listening on: {}", self.socket_path.display());
        eprintln!("Idle timeout: {}s", self.config.daemon.idle_timeout_secs);

        let idle_timeout = Duration::from_secs(self.config.daemon.idle_timeout_secs);
        let check_interval = Duration::from_secs(10);

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            *self.last_request.write().await = Instant::now();
                            self.handle_connection(stream).await;
                        }
                        Err(e) => {
                            eprintln!("Accept error: {}", e);
                        }
                    }
                }
                _ = tokio::time::sleep(check_interval) => {
                    if self.shutdown.load(Ordering::Relaxed) {
                        break;
                    }
                    let last = *self.last_request.read().await;
                    if last.elapsed() > idle_timeout {
                        eprintln!("Idle timeout reached, shutting down");
                        break;
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    eprintln!("Received SIGINT, shutting down");
                    break;
                }
            }
        }

        self.cleanup();
        Ok(())
    }

    async fn handle_connection(&self, mut stream: tokio::net::UnixStream) {
        let mut len_buf = [0u8; 4];

        while stream.read_exact(&mut len_buf).await.is_ok() {
            let len = decode_length(&len_buf);
            if len > 10 * 1024 * 1024 {
                break;
            }

            let mut msg_buf = vec![0u8; len];
            if stream.read_exact(&mut msg_buf).await.is_err() {
                break;
            }

            let request: Request = match serde_json::from_slice(&msg_buf) {
                Ok(r) => r,
                Err(e) => {
                    let response = Response::error(format!("invalid request: {}", e));
                    if let Ok(encoded) = encode_message(&response) {
                        let _ = stream.write_all(&encoded).await;
                    }
                    continue;
                }
            };

            let response = self.handle_request(request).await;
            self.requests_served.fetch_add(1, Ordering::Relaxed);

            if let Ok(encoded) = encode_message(&response)
                && stream.write_all(&encoded).await.is_err()
            {
                break;
            }

            if matches!(response, Response::ShutdownAck) {
                self.shutdown.store(true, Ordering::Relaxed);
                break;
            }
        }
    }

    async fn handle_request(&self, request: Request) -> Response {
        match request {
            Request::Ping => Response::Pong,

            Request::Shutdown => {
                self.shutdown.store(true, Ordering::Relaxed);
                Response::ShutdownAck
            }

            Request::Status => {
                let last = *self.last_request.read().await;
                let metrics_summary = self
                    .metrics
                    .as_ref()
                    .map(|m| m.get_summary(self.config.metrics.retention_days));
                Response::Status(StatusResponse {
                    running: true,
                    embedding_model: self.config.embedding.model_id.clone(),
                    idle_secs: last.elapsed().as_secs(),
                    requests_served: self.requests_served.load(Ordering::Relaxed),
                    metrics: metrics_summary,
                })
            }

            Request::Embed(req) => {
                let start = Instant::now();
                let result = self.embedding_model.embed(&req.texts, req.is_query);
                let latency_ms = start.elapsed().as_millis() as u64;
                let success = result.is_ok();
                if let Some(ref metrics) = self.metrics {
                    metrics.record(latency_ms, success);
                }
                match result {
                    Ok(embeddings) => Response::Embed(EmbedResponse { embeddings }),
                    Err(e) => Response::error(e.to_string()),
                }
            }
        }
    }

    fn write_pid_file(&self) -> Result<(), std::io::Error> {
        let pid_path = self.config.pid_path();
        std::fs::write(&pid_path, std::process::id().to_string())
    }

    fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.socket_path);
        let _ = std::fs::remove_file(self.config.pid_path());
        eprintln!("Daemon stopped");
    }
}

fn model_dir_name(model_id: &str) -> String {
    model_id.replace('/', "--")
}

pub async fn run_daemon(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let server = DaemonServer::new(config).await?;
    server.run().await?;
    Ok(())
}
