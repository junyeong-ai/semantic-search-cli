use anyhow::{Context, Result};
use clap::Args;
use std::time::Instant;

use crate::cli::output::get_formatter;
use crate::models::{Config, OutputFormat, SearchResults, SourceType, Tag, parse_tags};
use crate::services::{EmbeddingClient, VectorStoreClient};

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(required = true, help = "Search query text")]
    pub query: String,

    #[arg(long, short = 'n', help = "Maximum number of results to return")]
    pub limit: Option<u32>,

    #[arg(
        long,
        short = 't',
        help = "Filter by tags (e.g., 'source:confluence,space:common')"
    )]
    pub tags: Option<String>,

    #[arg(
        long,
        short = 's',
        help = "Filter by source type (e.g., 'local,confluence,jira')"
    )]
    pub source: Option<String>,

    #[arg(long, help = "Minimum similarity score threshold (0.0-1.0)")]
    pub min_score: Option<f32>,
}

pub async fn handle_search(args: SearchArgs, format: OutputFormat, verbose: bool) -> Result<()> {
    let query = args.query.trim();
    if query.is_empty() {
        anyhow::bail!("search query cannot be empty");
    }

    let config = Config::load()?;
    let formatter = get_formatter(format);
    let start_time = Instant::now();

    let limit = args.limit.unwrap_or(config.search.default_limit);
    if limit == 0 {
        anyhow::bail!("limit must be at least 1");
    }

    let min_score = args.min_score.or(config.search.default_min_score);
    if let Some(score) = min_score
        && !(0.0..=1.0).contains(&score)
    {
        anyhow::bail!("min_score must be between 0.0 and 1.0");
    }

    let tags: Vec<Tag> = args
        .tags
        .as_ref()
        .map(|s| parse_tags(s))
        .transpose()
        .context("failed to parse tags")?
        .unwrap_or_default();

    let source_types: Vec<SourceType> = if let Some(ref source_str) = args.source {
        source_str
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<SourceType>().map_err(|e| anyhow::anyhow!("{e}")))
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };

    if verbose {
        eprintln!("Query: \"{query}\"");
        eprintln!("  Limit: {limit}");
        if !tags.is_empty() {
            let tag_strs: Vec<String> = tags.iter().map(ToString::to_string).collect();
            eprintln!("  Tags: {}", tag_strs.join(", "));
        }
        if !source_types.is_empty() {
            let source_strs: Vec<String> = source_types.iter().map(ToString::to_string).collect();
            eprintln!("  Sources: {}", source_strs.join(", "));
        }
        if let Some(score) = min_score {
            eprintln!("  Min score: {score:.3}");
        }
    }

    let embedding_client = EmbeddingClient::new(&config.embedding)?;
    let vector_client = VectorStoreClient::new(&config.vector_store)?;

    let embed_start = Instant::now();
    let query_embedding = embedding_client
        .embed_query(query)
        .await
        .context("failed to generate query embedding")?;
    let embed_ms = embed_start.elapsed().as_millis();

    let search_start = Instant::now();
    let results = vector_client
        .search(
            query_embedding,
            u64::from(limit),
            &tags,
            &source_types,
            min_score,
        )
        .await
        .context("search failed")?;
    let search_ms = search_start.elapsed().as_millis();

    if verbose {
        let total_ms = start_time.elapsed().as_millis();
        eprintln!("Timing:");
        eprintln!("  Embedding: {embed_ms}ms");
        eprintln!("  Search: {search_ms}ms");
        eprintln!("  Total: {total_ms}ms");
        eprintln!();
    }

    let duration_ms = start_time.elapsed().as_millis() as u64;
    let total = results.len() as u64;
    let search_results = SearchResults::new(query.to_string(), results, total, duration_ms);

    print!("{}", formatter.format_search_results(&search_results));

    Ok(())
}
