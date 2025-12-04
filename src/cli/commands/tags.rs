//! Tags command implementation.

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli::output::get_formatter;
use crate::models::{Config, OutputFormat, Tag};
use crate::services::VectorStoreClient;

/// Tags subcommands.
#[derive(Debug, Subcommand)]
pub enum TagsCommand {
    /// List all tags with counts
    List,

    /// Delete documents by tag
    Delete {
        /// Tag to delete (format: key:value)
        #[arg(required = true)]
        tag: String,

        /// Show what would be deleted without actually deleting
        #[arg(long)]
        dry_run: bool,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        force: bool,
    },
}

/// Handle the tags command.
pub async fn handle_tags(cmd: TagsCommand, format: OutputFormat, verbose: bool) -> Result<()> {
    let formatter = get_formatter(format);
    let config = Config::load()?;

    match cmd {
        TagsCommand::List => handle_list(formatter.as_ref(), &config, verbose).await,
        TagsCommand::Delete {
            tag,
            dry_run,
            force,
        } => handle_delete(formatter.as_ref(), &config, &tag, dry_run, force, verbose).await,
    }
}

async fn handle_list(
    formatter: &dyn crate::cli::output::Formatter,
    config: &Config,
    _verbose: bool,
) -> Result<()> {
    let vector_client = VectorStoreClient::new(&config.vector_store).await?;

    // Note: Qdrant doesn't have a direct way to aggregate tags.
    // This would require scrolling through all points or using a separate index.
    // For now, we'll show a message about the limitation.

    // In a real implementation, you might want to:
    // 1. Maintain a separate tags index
    // 2. Use scroll API to aggregate tags
    // 3. Store tag counts in a separate collection

    let info = vector_client.get_collection_info().await?;

    if info.is_none() {
        println!(
            "{}",
            formatter.format_message("Collection not found. Run 'ssearch index' first.")
        );
        return Ok(());
    }

    // For now, return empty list with a note
    println!(
        "{}",
        formatter.format_message(
            "Tag aggregation requires scrolling through all points.\n\
         This feature will be implemented with pagination support."
        )
    );

    // Placeholder: would return actual tags
    let tags: Vec<(String, u64)> = vec![];
    print!("{}", formatter.format_tags(&tags));

    Ok(())
}

async fn handle_delete(
    formatter: &dyn crate::cli::output::Formatter,
    config: &Config,
    tag_str: &str,
    dry_run: bool,
    force: bool,
    verbose: bool,
) -> Result<()> {
    // Parse tag
    let tag: Tag = tag_str.parse().context("invalid tag format")?;

    if verbose {
        println!("Deleting documents with tag: {}", tag);
    }

    if dry_run {
        println!(
            "{}",
            formatter.format_message(&format!(
                "Dry run: Would delete documents with tag '{}'",
                tag
            ))
        );
        return Ok(());
    }

    // Confirmation prompt
    if !force {
        println!(
            "This will delete all documents with tag '{}'. Continue? [y/N]",
            tag
        );
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("{}", formatter.format_message("Cancelled."));
            return Ok(());
        }
    }

    // Delete
    let vector_client = VectorStoreClient::new(&config.vector_store).await?;
    vector_client
        .delete_by_tags(std::slice::from_ref(&tag))
        .await
        .context("failed to delete documents")?;

    println!(
        "{}",
        formatter.format_message(&format!("Deleted documents with tag '{}'", tag))
    );

    Ok(())
}
