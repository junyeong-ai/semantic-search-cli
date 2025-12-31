use anyhow::{Context, Result};
use clap::Subcommand;
use std::process::Command;

use crate::cli::output::get_formatter;
use crate::models::{Config, ConfigSource, OutputFormat, ResolvedConfig};

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    #[command(about = "Initialize configuration file")]
    Init {
        #[arg(
            long,
            short = 'g',
            help = "Create global config instead of project config"
        )]
        global: bool,
        #[arg(long, short = 'f', help = "Force overwrite existing config")]
        force: bool,
    },
    #[command(about = "Show current configuration")]
    Show {
        #[arg(long, help = "Show source of each configuration value")]
        source: bool,
    },
    #[command(about = "Show configuration file paths")]
    Path {
        #[arg(long, help = "Show all possible config paths")]
        all: bool,
    },
    #[command(about = "Edit configuration file")]
    Edit {
        #[arg(
            long,
            short = 'g',
            help = "Edit global config instead of project config"
        )]
        global: bool,
    },
}

pub async fn handle_config(cmd: ConfigCommand, format: OutputFormat, _verbose: bool) -> Result<()> {
    let formatter = get_formatter(format);

    match cmd {
        ConfigCommand::Init { global, force } => handle_init(global, force, formatter.as_ref()),
        ConfigCommand::Show { source } => handle_show(source, format),
        ConfigCommand::Path { all } => handle_path(all),
        ConfigCommand::Edit { global } => handle_edit(global, formatter.as_ref()),
    }
}

fn handle_init(
    global: bool,
    force: bool,
    formatter: &dyn crate::cli::output::Formatter,
) -> Result<()> {
    if global {
        let config_path = Config::global_path()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;

        if config_path.exists() && !force {
            anyhow::bail!(
                "Global config already exists at: {}\nUse --force to overwrite.",
                config_path.display()
            );
        }

        let path = Config::init_global().context("failed to create global config")?;
        println!(
            "{}",
            formatter.format_message(&format!("Created global config at: {}", path.display()))
        );
    } else {
        let config_dir = Config::project_config_dir()
            .ok_or_else(|| anyhow::anyhow!("could not determine project directory"))?;
        let config_path = config_dir.join("config.toml");

        if config_path.exists() && !force {
            anyhow::bail!(
                "Project config already exists at: {}\nUse --force to overwrite.",
                config_path.display()
            );
        }

        let path = Config::init_project().context("failed to create project config")?;
        println!(
            "{}",
            formatter.format_message(&format!("Created project config at: {}", path.display()))
        );
    }

    Ok(())
}

fn handle_show(show_source: bool, format: OutputFormat) -> Result<()> {
    let resolved = Config::load()?;

    if format == OutputFormat::Json {
        if show_source {
            let output = serde_json::json!({
                "config": resolved.config,
                "project_path": resolved.project_path,
                "global_path": resolved.global_path,
            });
            println!("{}", serde_json::to_string_pretty(&output)?);
        } else {
            println!("{}", serde_json::to_string_pretty(&resolved.config)?);
        }
        return Ok(());
    }

    if let Some(ref path) = resolved.project_path {
        println!("# Project config: {}", path.display());
    }
    if let Some(ref path) = resolved.global_path {
        println!("# Global config: {}", path.display());
    }
    println!();

    print_resolved_config(&resolved, show_source);

    Ok(())
}

fn print_resolved_config(resolved: &ResolvedConfig, show_source: bool) {
    let config = &resolved.config;
    let sources = &resolved.sources;
    let src = |s: &ConfigSource| {
        if show_source {
            format!("  # {}", format_source(s))
        } else {
            String::new()
        }
    };

    println!("[embedding]");
    println!(
        "model_id = \"{}\"{}",
        config.embedding.model_id,
        src(&sources.embedding_model_id)
    );
    if let Some(ref path) = config.embedding.model_path {
        println!("model_path = \"{}\"", path.display());
    }
    println!(
        "dimension = {}{}",
        config.embedding.dimension,
        src(&sources.embedding_dimension)
    );
    println!(
        "batch_size = {}{}",
        config.embedding.batch_size,
        src(&sources.embedding_batch_size)
    );
    println!(
        "max_tokens = {}{}",
        config.embedding.max_tokens,
        src(&sources.embedding_max_tokens)
    );
    println!();

    println!("[vector_store]");
    println!(
        "driver = \"{}\"{}",
        config.vector_store.driver,
        src(&sources.vector_store_driver)
    );
    println!(
        "url = \"{}\"{}",
        config.vector_store.url,
        src(&sources.vector_store_url)
    );
    println!(
        "collection = \"{}\"{}",
        config.vector_store.collection,
        src(&sources.vector_store_collection)
    );
    if config.vector_store.api_key.is_some() {
        println!(
            "api_key = \"********\"{}",
            src(&sources.vector_store_api_key)
        );
    }
    println!();

    println!("[indexing]");
    println!(
        "max_file_size = {}{}",
        config.indexing.max_file_size,
        src(&sources.indexing_max_file_size)
    );
    println!(
        "chunk_size = {}{}",
        config.indexing.chunk_size,
        src(&sources.indexing_chunk_size)
    );
    println!(
        "chunk_overlap = {}{}",
        config.indexing.chunk_overlap,
        src(&sources.indexing_chunk_overlap)
    );
    if !config.indexing.exclude_patterns.is_empty() {
        if show_source {
            println!(
                "exclude_patterns = [...]{}",
                src(&sources.indexing_exclude_patterns)
            );
        } else {
            println!("exclude_patterns = [");
            for pattern in &config.indexing.exclude_patterns {
                println!("  \"{pattern}\",");
            }
            println!("]");
        }
    }
    println!();

    println!("[search]");
    println!(
        "default_limit = {}{}",
        config.search.default_limit,
        src(&sources.search_default_limit)
    );
    println!(
        "default_format = \"{}\"{}",
        config.search.default_format,
        src(&sources.search_default_format)
    );
    if let Some(score) = config.search.default_min_score {
        println!("default_min_score = {score}");
    }
    println!();

    println!("[daemon]");
    println!(
        "idle_timeout_secs = {}{}",
        config.daemon.idle_timeout_secs,
        src(&sources.daemon_idle_timeout)
    );
    println!(
        "auto_start = {}{}",
        config.daemon.auto_start,
        src(&sources.daemon_auto_start)
    );
    if !show_source {
        println!("socket_path = \"{}\"", config.socket_path().display());
    }
    println!();

    println!("[metrics]");
    println!(
        "enabled = {}{}",
        config.metrics.enabled,
        src(&sources.metrics_enabled)
    );
    println!(
        "retention_days = {}{}",
        config.metrics.retention_days,
        src(&sources.metrics_retention_days)
    );
}

fn format_source(source: &ConfigSource) -> &'static str {
    match source {
        ConfigSource::Default => "default",
        ConfigSource::Global => "global",
        ConfigSource::Project => "project",
        ConfigSource::Env => "env",
    }
}

fn handle_path(show_all: bool) -> Result<()> {
    let project_path = Config::find_project_config();
    let global_path = Config::global_path();

    println!("Configuration paths:");
    println!();

    if let Some(ref path) = project_path {
        println!("Project config (active): {}", path.display());
    } else if show_all && let Some(dir) = Config::project_config_dir() {
        println!(
            "Project config (would be): {}",
            dir.join("config.toml").display()
        );
    }

    if let Some(ref path) = global_path {
        if path.exists() {
            println!("Global config (active): {}", path.display());
        } else if show_all {
            println!("Global config (would be): {}", path.display());
        }
    }

    if show_all && let Ok(cwd) = std::env::current_dir() {
        let env_path = cwd.join(".env");
        if env_path.exists() {
            println!(".env file (active): {}", env_path.display());
        } else {
            println!(".env file (would be): {}", env_path.display());
        }
    }

    Ok(())
}

fn handle_edit(global: bool, formatter: &dyn crate::cli::output::Formatter) -> Result<()> {
    let config_path = if global {
        let path = Config::global_path()
            .ok_or_else(|| anyhow::anyhow!("could not determine home directory"))?;

        if !path.exists() {
            Config::init_global().context("failed to create global config")?;
            println!(
                "{}",
                formatter.format_message(&format!("Created global config at: {}", path.display()))
            );
        }
        path
    } else {
        let path = Config::find_project_config()
            .or_else(|| Config::project_config_dir().map(|d| d.join("config.toml")))
            .ok_or_else(|| anyhow::anyhow!("could not determine config path"))?;

        if !path.exists() {
            Config::init_project().context("failed to create project config")?;
            println!(
                "{}",
                formatter.format_message(&format!("Created project config at: {}", path.display()))
            );
        }
        path
    };

    let editor = std::env::var("EDITOR")
        .unwrap_or_else(|_| std::env::var("VISUAL").unwrap_or_else(|_| "vim".into()));

    Command::new(&editor)
        .arg(&config_path)
        .status()
        .context(format!("failed to open editor: {}", editor))?;

    Ok(())
}
