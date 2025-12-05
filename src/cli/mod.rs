//! CLI module for the semantic search CLI.

pub mod commands;
pub mod output;

use clap::{Parser, Subcommand};

use crate::models::OutputFormat;

/// Semantic search CLI for local files and external data sources.
#[derive(Debug, Parser)]
#[command(name = "ssearch")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    #[arg(
        long,
        short = 'f',
        global = true,
        help = "Output format: text, json, or markdown"
    )]
    pub format: Option<OutputFormat>,

    #[arg(long, short = 'v', global = true, help = "Enable verbose output")]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Check infrastructure status (embedding server, Qdrant)
    Status,

    /// Manage search index (add, delete, clear)
    #[command(subcommand)]
    Index(commands::IndexCommand),

    /// Search indexed content
    Search(commands::SearchArgs),

    /// Manage configuration
    #[command(subcommand)]
    Config(commands::ConfigCommand),

    /// Manage tags
    #[command(subcommand)]
    Tags(commands::TagsCommand),

    /// Import data from JSON/JSONL files
    Import(commands::ImportArgs),

    /// Manage external data sources
    #[command(subcommand)]
    Source(commands::SourceCommand),
}

// FromStr is implemented in models::search
