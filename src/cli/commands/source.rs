//! Source command implementation for external data sources.

use anyhow::{Context, Result};
use clap::Subcommand;
use indicatif::{ProgressBar, ProgressStyle};
use std::process::Command;
use std::time::Instant;

use crate::cli::output::{CliInfo, IndexStats, SourceInfo, get_formatter};
use crate::models::{Config, OutputFormat, SourceType, Tag, parse_tags};
use crate::services::{EmbeddingClient, TextChunker, create_backend, process_batch};
use crate::sources::{SyncOptions, get_data_source};

#[derive(Debug, Subcommand)]
pub enum SourceCommand {
    /// List available data sources and their status
    List,

    /// Sync data from an external source
    Sync {
        /// Source type (jira, confluence, figma)
        #[arg(required = true)]
        source: String,

        /// Source-specific query (e.g., JQL for Jira, CQL for Confluence)
        #[arg(long, short = 'q')]
        query: Option<String>,

        /// Project key (Jira) or space key (Confluence) - syncs all items
        #[arg(long, short = 'p')]
        project: Option<String>,

        /// Tags to apply to synced documents
        #[arg(long, short = 't')]
        tags: Option<String>,

        /// Maximum items to sync (ignored with --all)
        #[arg(long, default_value = "100")]
        limit: u32,

        /// Fetch all items without limit (streaming mode)
        #[arg(long)]
        all: bool,

        /// Exclude pages under these ancestor IDs (Confluence only, comma-separated)
        #[arg(long)]
        exclude_ancestor: Option<String>,
    },

    /// Delete all indexed documents from a source type
    Delete {
        /// Source type to delete (jira, confluence, figma)
        #[arg(required = true)]
        source: String,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        force: bool,
    },

    /// Check if external CLI tools are installed
    Status,
}

pub async fn handle_source(cmd: SourceCommand, format: OutputFormat, verbose: bool) -> Result<()> {
    let formatter = get_formatter(format);
    let config = Config::load()?;

    match cmd {
        SourceCommand::List => handle_list(formatter.as_ref(), verbose),
        SourceCommand::Sync {
            source,
            query,
            project,
            tags,
            limit,
            all,
            exclude_ancestor,
        } => {
            handle_sync(
                formatter.as_ref(),
                &config,
                &source,
                query,
                project,
                tags,
                limit,
                all,
                exclude_ancestor,
                verbose,
            )
            .await
        }
        SourceCommand::Delete { source, force } => {
            handle_delete(formatter.as_ref(), &config, &source, force, verbose).await
        }
        SourceCommand::Status => handle_status(formatter.as_ref(), verbose),
    }
}

fn handle_list(formatter: &dyn crate::cli::output::Formatter, _verbose: bool) -> Result<()> {
    let source_defs: &[(&str, &str, &str)] = &[
        ("jira", "Jira issues via atlassian-cli", "atlassian-cli"),
        (
            "confluence",
            "Confluence pages via atlassian-cli",
            "atlassian-cli",
        ),
        ("figma", "Figma designs via figma-cli", "figma-cli"),
    ];

    let sources: Vec<SourceInfo> = source_defs
        .iter()
        .map(|&(name, desc, cli)| SourceInfo {
            name: name.to_owned(),
            description: desc.to_owned(),
            available: check_cli_available(cli),
        })
        .collect();

    print!("{}", formatter.format_sources(&sources));
    Ok(())
}

fn handle_status(formatter: &dyn crate::cli::output::Formatter, _verbose: bool) -> Result<()> {
    let cli_defs: &[(&str, &str)] = &[
        ("atlassian-cli", "For Jira and Confluence integration"),
        ("figma-cli", "For Figma design integration"),
    ];

    let clis: Vec<CliInfo> = cli_defs
        .iter()
        .map(|&(name, desc)| {
            let available = check_cli_available(name);
            let version = if available {
                Command::new(name)
                    .arg("--version")
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            };
            CliInfo {
                name: name.to_owned(),
                description: desc.to_owned(),
                available,
                version,
            }
        })
        .collect();

    print!("{}", formatter.format_cli_status(&clis));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn handle_sync(
    formatter: &dyn crate::cli::output::Formatter,
    config: &Config,
    source: &str,
    query: Option<String>,
    project: Option<String>,
    tags: Option<String>,
    limit: u32,
    all: bool,
    exclude_ancestor: Option<String>,
    verbose: bool,
) -> Result<()> {
    let start_time = Instant::now();

    let source_type: SourceType = source
        .parse()
        .map_err(|_| anyhow::anyhow!("unknown source type: {}", source))?;

    if !source_type.is_external() {
        anyhow::bail!(
            "source '{}' is not an external source. Use 'ssearch index add' for local files.",
            source
        );
    }

    let data_source = get_data_source(source_type)
        .ok_or_else(|| anyhow::anyhow!("no implementation found for source: {}", source))?;

    if !data_source.check_available()? {
        anyhow::bail!(
            "Required CLI is not installed.\n{}",
            data_source.install_instructions()
        );
    }

    if project.is_some() && !matches!(source_type, SourceType::Jira | SourceType::Confluence) {
        anyhow::bail!("--project option is only available for Jira and Confluence sources");
    }

    let tags: Vec<Tag> = if let Some(ref tag_str) = tags {
        parse_tags(tag_str).context("failed to parse tags")?
    } else {
        Vec::new()
    };

    let exclude_ancestors: Vec<String> = exclude_ancestor
        .map(|s| s.split(',').map(|id| id.trim().to_string()).collect())
        .unwrap_or_default();

    println!("Syncing from {} source...", data_source.name());
    if verbose {
        if let Some(ref p) = project {
            println!("  Project: {}", p);
        }
        if let Some(ref q) = query {
            println!("  Query: {}", q);
        }
        if !all {
            println!("  Limit: {}", limit);
        }
        if !exclude_ancestors.is_empty() {
            println!("  Excluding ancestors: {:?}", exclude_ancestors);
        }
    }

    let sync_options = SyncOptions {
        query,
        project,
        tags,
        limit: if all { None } else { Some(limit) },
        exclude_ancestors,
    };

    let documents = data_source
        .sync(sync_options)
        .context("failed to sync from external source")?;

    if documents.is_empty() {
        println!(
            "{}",
            formatter.format_message("No documents found from source.")
        );
        return Ok(());
    }

    println!("Fetched {} documents, indexing...", documents.len());

    let embedding_client = EmbeddingClient::new(config);
    let vector_store = create_backend(&config.vector_store).await?;
    vector_store.create_collection().await?;

    let chunker = TextChunker::new(&config.indexing);

    let pb = ProgressBar::new(documents.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut stats = IndexStats {
        files_scanned: documents.len() as u64,
        ..Default::default()
    };

    let batch_size = config.embedding.batch_size as usize;
    let mut pending_chunks = Vec::new();
    let mut pending_texts = Vec::new();

    for document in &documents {
        pb.inc(1);

        if document.content.is_empty() {
            stats.files_skipped += 1;
            continue;
        }

        let chunks = chunker.chunk(document);
        stats.chunks_created += chunks.len() as u64;
        stats.files_indexed += 1;

        for chunk in chunks {
            pending_texts.push(chunk.content.clone());
            pending_chunks.push(chunk);
        }

        if pending_texts.len() >= batch_size {
            process_batch(
                &embedding_client,
                vector_store.as_ref(),
                &mut pending_chunks,
                &mut pending_texts,
            )
            .await?;
        }
    }

    if !pending_texts.is_empty() {
        process_batch(
            &embedding_client,
            vector_store.as_ref(),
            &mut pending_chunks,
            &mut pending_texts,
        )
        .await?;
    }

    pb.finish_and_clear();
    stats.duration_ms = start_time.elapsed().as_millis() as u64;
    print!("{}", formatter.format_index_stats(&stats));

    Ok(())
}

async fn handle_delete(
    formatter: &dyn crate::cli::output::Formatter,
    config: &Config,
    source: &str,
    force: bool,
    verbose: bool,
) -> Result<()> {
    let source_type: SourceType = source
        .parse()
        .map_err(|_| anyhow::anyhow!("unknown source type: {}", source))?;

    if !source_type.is_external() {
        anyhow::bail!(
            "source '{}' is not an external source. Use 'ssearch index delete' for local files.",
            source
        );
    }

    if verbose {
        println!("Deleting all {} documents from index...", source);
    }

    if !force {
        println!(
            "This will delete all indexed documents from source '{}'. Continue? [y/N]",
            source
        );
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("{}", formatter.format_message("Cancelled."));
            return Ok(());
        }
    }

    let vector_store = create_backend(&config.vector_store).await?;
    vector_store.delete_by_source_type(source_type).await?;

    println!(
        "{}",
        formatter.format_message(&format!("Deleted all {} documents from index.", source))
    );

    Ok(())
}

fn check_cli_available(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
