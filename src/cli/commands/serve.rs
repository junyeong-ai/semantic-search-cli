use anyhow::Result;
use clap::{Args, Subcommand};

use crate::client::{DaemonClient, stop_daemon};
use crate::models::Config;
use crate::server::run_daemon;

#[derive(Debug, Args)]
pub struct ServeArgs {
    #[command(subcommand)]
    pub command: Option<ServeCommand>,

    #[arg(long, hide = true)]
    pub daemon: bool,

    #[arg(long, hide = true)]
    pub foreground: bool,
}

#[derive(Debug, Subcommand)]
pub enum ServeCommand {
    Stop,
    Restart,
}

pub async fn handle_serve(args: ServeArgs) -> Result<()> {
    let config = Config::load()?.config;

    if args.daemon {
        return run_daemon_mode(config).await;
    }

    if args.foreground {
        eprintln!("Starting daemon in foreground mode...");
        return run_daemon_mode(config).await;
    }

    match args.command {
        Some(ServeCommand::Stop) => handle_stop(&config),
        Some(ServeCommand::Restart) => handle_restart(&config).await,
        None => handle_start(&config),
    }
}

fn handle_start(config: &Config) -> Result<()> {
    let client = DaemonClient::new(config);

    if client.is_running() {
        println!("Daemon is already running");
        return Ok(());
    }

    let exe = std::env::current_exe()?;

    std::process::Command::new(&exe)
        .args(["serve", "--daemon"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;

    println!("Daemon started");
    println!("Socket: {}", config.socket_path().display());
    Ok(())
}

fn handle_stop(config: &Config) -> Result<()> {
    match stop_daemon(config) {
        Ok(_) => {
            println!("Daemon stopped");
            Ok(())
        }
        Err(crate::error::DaemonError::NotRunning) => {
            println!("Daemon is not running");
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

async fn handle_restart(config: &Config) -> Result<()> {
    let _ = stop_daemon(config);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    handle_start(config)
}

async fn run_daemon_mode(config: Config) -> Result<()> {
    run_daemon(config)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))
}
