//! Status command implementation.

use anyhow::Result;

use crate::cli::output::{StatusInfo, get_formatter};
use crate::models::{Config, OutputFormat};
use crate::services::{EmbeddingClient, VectorStoreClient};

/// Handle the status command.
pub async fn handle_status(format: OutputFormat, _verbose: bool) -> Result<()> {
    let config = Config::load()?;
    let formatter = get_formatter(format);

    // Check embedding server
    let embedding_client = EmbeddingClient::new(&config.embedding)?;
    let (embedding_connected, embedding_model) = match embedding_client.health_check().await {
        Ok(health) => (true, health.model_id),
        Err(_) => (false, None),
    };

    // Check Qdrant server
    let qdrant_connected;
    let qdrant_points;

    match VectorStoreClient::new(&config.vector_store).await {
        Ok(client) => {
            qdrant_connected = client.health_check().await.unwrap_or(false);
            qdrant_points = if qdrant_connected {
                client
                    .get_collection_info()
                    .await
                    .ok()
                    .flatten()
                    .map(|info| info.points_count)
                    .unwrap_or(0)
            } else {
                0
            };
        }
        Err(_) => {
            qdrant_connected = false;
            qdrant_points = 0;
        }
    }

    let status = StatusInfo {
        embedding_url: config.embedding.url.clone(),
        embedding_connected,
        embedding_model,
        qdrant_url: config.vector_store.url.clone(),
        qdrant_connected,
        qdrant_points,
        collection: config.vector_store.collection.clone(),
    };

    print!("{}", formatter.format_status(&status));

    // Exit with error if infrastructure is not running
    if !embedding_connected || !qdrant_connected {
        eprintln!();
        if !embedding_connected {
            eprintln!(
                "⚠ Embedding server is not running. Start it with: cd embedding-server && python server.py"
            );
        }
        if !qdrant_connected {
            eprintln!("⚠ Qdrant is not running. Start it with: docker-compose up -d qdrant");
        }
        std::process::exit(1);
    }

    Ok(())
}
