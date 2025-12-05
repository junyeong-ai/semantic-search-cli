//! Semantic Search CLI entry point.

use anyhow::Result;
use clap::Parser;
use tokio::signal;

use ssearch::cli::commands::{
    handle_config, handle_import, handle_index, handle_search, handle_source, handle_status,
    handle_tags,
};
use ssearch::cli::{Cli, Commands};
use ssearch::models::Config;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load().unwrap_or_default();
    let format = cli.format.unwrap_or(config.search.default_format);
    let verbose = cli.verbose;

    // Run the command with graceful shutdown support
    tokio::select! {
        result = run_command(cli.command, format, verbose) => {
            result?;
        }
        _ = shutdown_signal() => {
            eprintln!("\nReceived shutdown signal, cleaning up...");
            // Give a moment for cleanup
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    Ok(())
}

/// Run the selected command.
async fn run_command(
    command: Commands,
    format: ssearch::models::OutputFormat,
    verbose: bool,
) -> Result<()> {
    match command {
        Commands::Status => {
            handle_status(format, verbose).await?;
        }
        Commands::Index(cmd) => {
            handle_index(cmd, format, verbose).await?;
        }
        Commands::Search(args) => {
            handle_search(args, format, verbose).await?;
        }
        Commands::Config(cmd) => {
            handle_config(cmd, format, verbose).await?;
        }
        Commands::Tags(cmd) => {
            handle_tags(cmd, format, verbose).await?;
        }
        Commands::Import(args) => {
            handle_import(args, format, verbose).await?;
        }
        Commands::Source(cmd) => {
            handle_source(cmd, format, verbose).await?;
        }
    }

    Ok(())
}

/// Wait for shutdown signal (Ctrl+C).
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
