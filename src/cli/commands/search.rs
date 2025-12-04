use anyhow::{Context, Result};
use clap::Args;
use std::time::Instant;

use crate::cli::output::get_formatter;
use crate::models::{Config, OutputFormat, SearchResults, SourceType, Tag, parse_tags};
use crate::services::{EmbeddingClient, VectorStoreClient};

#[derive(Debug, Args)]
pub struct SearchArgs {
    #[arg(required = true)]
    pub query: String,

    #[arg(long, short = 'n')]
    pub limit: Option<u32>,

    #[arg(long, short = 't')]
    pub tags: Option<String>,

    #[arg(long, short = 's')]
    pub source: Option<String>,

    #[arg(long)]
    pub min_score: Option<f32>,
}

pub async fn handle_search(args: SearchArgs, format: OutputFormat, verbose: bool) -> Result<()> {
    let config = Config::load()?;
    let formatter = get_formatter(format);
    let start_time = Instant::now();

    let limit = args.limit.unwrap_or(config.search.default_limit);
    let min_score = args.min_score.or(config.search.default_min_score);

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
            .map(|s| {
                s.parse::<SourceType>()
                    .map_err(|e| anyhow::anyhow!("{}", e))
            })
            .collect::<Result<Vec<_>>>()?
    } else {
        Vec::new()
    };

    if verbose {
        println!("Searching for: \"{}\"", args.query);
        if !tags.is_empty() {
            let tag_strs: Vec<String> = tags.iter().map(|t| t.to_string()).collect();
            println!("  Tags: {}", tag_strs.join(", "));
        }
        if !source_types.is_empty() {
            let source_strs: Vec<String> = source_types.iter().map(|s| s.to_string()).collect();
            println!("  Sources: {}", source_strs.join(", "));
        }
        if let Some(score) = min_score {
            println!("  Min score: {}", score);
        }
    }

    let embedding_client = EmbeddingClient::new(&config.embedding)?;
    let vector_client = VectorStoreClient::new(&config.vector_store).await?;

    let query_embedding = embedding_client
        .embed_query(&args.query)
        .await
        .context("failed to generate query embedding")?;

    let results = vector_client
        .search(
            query_embedding,
            limit as u64,
            &tags,
            &source_types,
            min_score,
        )
        .await
        .context("search failed")?;

    let duration_ms = start_time.elapsed().as_millis() as u64;
    let total = results.len() as u64;
    let search_results = SearchResults::new(args.query.clone(), results, total, duration_ms);

    print!("{}", formatter.format_search_results(&search_results));

    Ok(())
}
