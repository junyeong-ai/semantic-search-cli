//! Tags command implementation.

use anyhow::{Context, Result};
use clap::Subcommand;

use crate::cli::output::get_formatter;
use crate::models::{Config, OutputFormat, Tag};
use crate::services::create_backend;

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
    let vector_store = create_backend(&config.vector_store).await?;

    let info = vector_store.get_collection_info().await?;
    if info.is_none() {
        println!(
            "{}",
            formatter.format_message("Collection not found. Run 'ssearch index' first.")
        );
        return Ok(());
    }

    let tags = vector_store
        .list_all_tags()
        .await
        .context("failed to list tags")?;

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
    let vector_store = create_backend(&config.vector_store).await?;
    vector_store
        .delete_by_tags(std::slice::from_ref(&tag))
        .await
        .context("failed to delete documents")?;

    println!(
        "{}",
        formatter.format_message(&format!("Deleted documents with tag '{}'", tag))
    );

    Ok(())
}
