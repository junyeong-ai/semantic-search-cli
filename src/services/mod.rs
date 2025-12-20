mod batch;
mod chunker;
mod embedding;
mod metrics;
pub mod vector_store;

pub use batch::process_batch;
pub use chunker::{TextChunker, estimate_tokens};
pub use embedding::EmbeddingClient;
pub use metrics::{MetricsStore, MetricsSummary};

pub use vector_store::{
    CollectionInfo, EMBEDDING_DIM, PgVectorBackend, QdrantBackend, VectorStore, create_backend,
};
