use anyhow::Result;

use crate::cli::output::{StatusInfo, get_formatter};
use crate::client::DaemonClient;
use crate::models::{Config, OutputFormat, VectorDriver};
use crate::services::create_backend;

pub async fn handle_status(format: OutputFormat, _verbose: bool) -> Result<()> {
    let config = Config::load()?;
    let formatter = get_formatter(format);

    let client = DaemonClient::new(&config);
    let daemon_running = client.is_running();

    let (daemon_status, embedding_model, idle_secs, metrics) = if daemon_running {
        match client.status().await {
            Ok(status) => (
                true,
                Some(status.embedding_model),
                Some(status.idle_secs),
                status.metrics,
            ),
            Err(_) => (false, None, None, None),
        }
    } else {
        (false, None, None, None)
    };

    let (vector_store_connected, vector_store_points) =
        if let Ok(store) = create_backend(&config.vector_store).await {
            let connected = store.health_check().await.unwrap_or(false);
            let points = if connected {
                store
                    .get_collection_info()
                    .await
                    .ok()
                    .flatten()
                    .map_or(0, |info| info.points_count)
            } else {
                0
            };
            (connected, points)
        } else {
            (false, 0)
        };

    let status = StatusInfo {
        daemon_running: daemon_status,
        daemon_idle_secs: idle_secs,
        embedding_model,
        vector_store_driver: config.vector_store.driver.to_string(),
        vector_store_url: config.vector_store.url.clone(),
        vector_store_connected,
        vector_store_points,
        collection: config.vector_store.collection.clone(),
        metrics,
    };

    print!("{}", formatter.format_status(&status));

    if !daemon_status || !vector_store_connected {
        eprintln!();
        if !daemon_status {
            eprintln!(
                "Hint: ML daemon not running. It will start automatically on first search/index."
            );
            eprintln!("      Or start manually with: ssearch serve");
        }
        if !vector_store_connected {
            match config.vector_store.driver {
                VectorDriver::Qdrant => {
                    eprintln!(
                        "Warning: Qdrant not running. Start with: docker-compose up -d qdrant"
                    );
                }
                VectorDriver::PostgreSQL => {
                    eprintln!("Warning: PostgreSQL not accessible. Check connection settings.");
                }
            }
        }
    }

    Ok(())
}
