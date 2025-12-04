use anyhow::{Context, Result};
use clap::Subcommand;
use std::process::Command;

use crate::cli::output::get_formatter;
use crate::models::{Config, OutputFormat};

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Init,
    Show,
    Path,
    Edit,
}

pub async fn handle_config(cmd: ConfigCommand, format: OutputFormat, _verbose: bool) -> Result<()> {
    let formatter = get_formatter(format);

    match cmd {
        ConfigCommand::Init => handle_init(formatter.as_ref()),
        ConfigCommand::Show => handle_show(format),
        ConfigCommand::Path => handle_path(),
        ConfigCommand::Edit => handle_edit(formatter.as_ref()),
    }
}

fn handle_init(formatter: &dyn crate::cli::output::Formatter) -> Result<()> {
    let config_path = Config::config_path()
        .ok_or_else(|| anyhow::anyhow!("could not determine config directory"))?;

    if config_path.exists() {
        println!(
            "{}",
            formatter.format_message(&format!(
                "Config file already exists at: {}",
                config_path.display()
            ))
        );
        return Ok(());
    }

    let config = Config::default();
    config.save().context("failed to save config")?;

    println!(
        "{}",
        formatter.format_message(&format!(
            "Created default config at: {}",
            config_path.display()
        ))
    );

    Ok(())
}

fn handle_show(format: OutputFormat) -> Result<()> {
    let config = Config::load()?;
    let config_path = Config::config_path();

    match format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&config)?;
            println!("{}", json);
        }
        _ => {
            if let Some(path) = config_path {
                println!("Config file: {}\n", path.display());
            }

            println!("[embedding]");
            println!("url = \"{}\"", config.embedding.url);
            println!("timeout_secs = {}", config.embedding.timeout_secs);
            println!("batch_size = {}", config.embedding.batch_size);
            println!();

            println!("[vector_store]");
            println!("url = \"{}\"", config.vector_store.url);
            println!("collection = \"{}\"", config.vector_store.collection);
            if config.vector_store.api_key.is_some() {
                println!("api_key = \"********\"");
            }
            println!();

            println!("[indexing]");
            println!("max_file_size = {}", config.indexing.max_file_size);
            println!("chunk_size = {}", config.indexing.chunk_size);
            println!("chunk_overlap = {}", config.indexing.chunk_overlap);
            if !config.indexing.exclude_patterns.is_empty() {
                println!("exclude_patterns = [");
                for pattern in &config.indexing.exclude_patterns {
                    println!("  \"{}\",", pattern);
                }
                println!("]");
            }
            println!();

            println!("[search]");
            println!("default_limit = {}", config.search.default_limit);
            println!("default_format = \"{}\"", config.search.default_format);
            if let Some(score) = config.search.default_min_score {
                println!("default_min_score = {}", score);
            }
        }
    }

    Ok(())
}

fn handle_path() -> Result<()> {
    let config_path = Config::config_path()
        .ok_or_else(|| anyhow::anyhow!("could not determine config directory"))?;

    println!("{}", config_path.display());

    Ok(())
}

fn handle_edit(formatter: &dyn crate::cli::output::Formatter) -> Result<()> {
    let config_path = Config::config_path()
        .ok_or_else(|| anyhow::anyhow!("could not determine config directory"))?;

    if !config_path.exists() {
        let config = Config::default();
        config.save().context("failed to create config")?;
        println!(
            "{}",
            formatter.format_message(&format!(
                "Created default config at: {}",
                config_path.display()
            ))
        );
    }

    let editor = std::env::var("EDITOR")
        .unwrap_or_else(|_| std::env::var("VISUAL").unwrap_or_else(|_| "vim".to_string()));

    Command::new(&editor)
        .arg(&config_path)
        .status()
        .context(format!("failed to open editor: {}", editor))?;

    Ok(())
}
