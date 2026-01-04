use std::path::Path;

use anyhow::Result;
use clap::Parser;
use tokio::signal;

use ssearch::cli::commands::{
    handle_config, handle_import, handle_index, handle_search, handle_serve, handle_source,
    handle_status, handle_tags,
};
use ssearch::cli::{Cli, Commands};
use ssearch::models::Config;

/// Detect ONNX Runtime library path and set ORT_DYLIB_PATH if not already set.
/// Must be called before any ort code runs.
fn detect_and_set_ort_path() {
    // Skip if user has already set a valid ORT_DYLIB_PATH
    if std::env::var("ORT_DYLIB_PATH")
        .map(|p| Path::new(&p).exists())
        .unwrap_or(false)
    {
        return;
    }

    let home = std::env::var("HOME").unwrap_or_default();

    // Find first existing path
    let found = if cfg!(target_os = "macos") {
        [
            format!("{home}/.local/lib/ssearch/libonnxruntime.dylib"),
            "/opt/homebrew/opt/onnxruntime/lib/libonnxruntime.dylib".into(),
            "/usr/local/opt/onnxruntime/lib/libonnxruntime.dylib".into(),
        ]
        .into_iter()
        .find(|p| Path::new(p).exists())
    } else if cfg!(target_os = "linux") {
        [
            format!("{home}/.local/lib/ssearch/libonnxruntime.so"),
            "/usr/lib/libonnxruntime.so".into(),
            "/usr/local/lib/libonnxruntime.so".into(),
            "/usr/lib/x86_64-linux-gnu/libonnxruntime.so".into(),
            "/usr/lib/aarch64-linux-gnu/libonnxruntime.so".into(),
        ]
        .into_iter()
        .find(|p| Path::new(p).exists())
    } else {
        None
    };

    if let Some(path) = found {
        // SAFETY: Called at program start before any threads are spawned.
        unsafe {
            std::env::set_var("ORT_DYLIB_PATH", path);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    detect_and_set_ort_path();

    let cli = Cli::parse();
    let resolved = Config::load().unwrap_or_default();
    let format = cli.format.unwrap_or(resolved.config.search.default_format);
    let verbose = cli.verbose;

    tokio::select! {
        result = run_command(cli.command, format, verbose) => {
            result?;
        }
        _ = shutdown_signal() => {
            eprintln!("\nReceived shutdown signal, cleaning up...");
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    Ok(())
}

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
        Commands::Serve(args) => {
            handle_serve(args).await?;
        }
    }

    Ok(())
}

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
