//! Import command implementation.

use anyhow::{Context, Result};
use clap::Args;
use serde::Deserialize;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::cli::output::{IndexStats, get_formatter};
use crate::models::{
    Config, Document, DocumentMetadata, OutputFormat, Source, SourceType, Tag, parse_tags,
};
use crate::services::{EmbeddingClient, TextChunker, VectorStoreClient, process_batch};

/// Arguments for the import command.
#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Path to JSON or JSONL file (use - for stdin)
    #[arg()]
    pub file: Option<PathBuf>,

    /// Tags to apply to imported documents (comma-separated, format: key:value)
    #[arg(long, short = 't')]
    pub tags: Option<String>,

    /// Source name for imported documents
    #[arg(long, default_value = "custom")]
    pub source: String,

    /// Only validate the import file without indexing
    #[arg(long)]
    pub validate_only: bool,
}

/// JSON import document format.
#[derive(Debug, Deserialize)]
pub struct ImportDocument {
    pub content: String,
    pub url: String,
    pub title: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source_type: Option<String>,
}

/// Handle the import command.
pub async fn handle_import(args: ImportArgs, format: OutputFormat, verbose: bool) -> Result<()> {
    let config = Config::load()?;
    let formatter = get_formatter(format);
    let start_time = Instant::now();

    // Parse tags
    let tags: Vec<Tag> = if let Some(ref tag_str) = args.tags {
        parse_tags(tag_str).context("failed to parse tags")?
    } else {
        Vec::new()
    };

    // Read input
    let input = read_input(args.file.as_deref())?;

    // Parse documents
    let import_docs = parse_import_documents(&input)?;

    if import_docs.is_empty() {
        println!(
            "{}",
            formatter.format_message("No documents found in input.")
        );
        return Ok(());
    }

    if verbose || args.validate_only {
        println!("Found {} documents to import", import_docs.len());
    }

    if args.validate_only {
        println!(
            "{}",
            formatter.format_message(&format!(
                "Validation successful: {} documents ready for import",
                import_docs.len()
            ))
        );
        return Ok(());
    }

    // Initialize clients
    let embedding_client = EmbeddingClient::new(&config.embedding)?;
    let vector_client = VectorStoreClient::new(&config.vector_store)?;

    // Ensure collection exists
    vector_client.create_collection().await?;

    // Create chunker
    let chunker = TextChunker::new(&config.indexing);

    let mut stats = IndexStats {
        files_scanned: import_docs.len() as u64,
        ..Default::default()
    };

    // Process documents
    let batch_size = config.embedding.batch_size as usize;
    let mut pending_chunks = Vec::new();
    let mut pending_texts = Vec::new();

    for import_doc in import_docs {
        // Validate required fields
        if import_doc.content.is_empty() {
            stats.files_skipped += 1;
            continue;
        }
        if import_doc.url.is_empty() {
            stats.files_skipped += 1;
            continue;
        }

        // Create document
        let source = import_doc
            .source_type
            .as_deref()
            .and_then(|st| st.parse::<SourceType>().ok())
            .filter(SourceType::is_external)
            .map_or_else(
                || Source::custom(&import_doc.url),
                |source_type| Source::external(source_type, &import_doc.url, &import_doc.url),
            );
        let metadata = DocumentMetadata {
            filename: None,
            extension: None,
            language: None,
            title: import_doc.title.clone(),
            path: import_doc.path.clone(),
            size_bytes: import_doc.content.len() as u64,
        };

        let checksum = {
            use sha2::{Digest, Sha256};
            let hash = Sha256::digest(import_doc.content.as_bytes());
            hex::encode(hash)
        };

        // Merge CLI tags with JSON file tags
        let mut doc_tags = tags.clone();
        for tag_str in &import_doc.tags {
            if let Ok(tag) = tag_str.parse::<Tag>() {
                // Avoid duplicates
                if !doc_tags.iter().any(|t| t.to_string() == tag.to_string()) {
                    doc_tags.push(tag);
                }
            }
        }

        let document = Document::new(import_doc.content, source, doc_tags, checksum, metadata);

        // Chunk document
        let chunks = chunker.chunk(&document);
        stats.chunks_created += chunks.len() as u64;
        stats.files_indexed += 1;

        for chunk in chunks {
            pending_texts.push(chunk.content.clone());
            pending_chunks.push(chunk);
        }

        // Process batch if full
        if pending_texts.len() >= batch_size {
            process_batch(
                &embedding_client,
                &vector_client,
                &mut pending_chunks,
                &mut pending_texts,
            )
            .await?;
        }
    }

    // Process remaining chunks
    if !pending_texts.is_empty() {
        process_batch(
            &embedding_client,
            &vector_client,
            &mut pending_chunks,
            &mut pending_texts,
        )
        .await?;
    }

    stats.duration_ms = start_time.elapsed().as_millis() as u64;

    print!("{}", formatter.format_index_stats(&stats));

    Ok(())
}

/// Read input from file or stdin.
fn read_input(file: Option<&Path>) -> Result<String> {
    match file {
        Some(path) if path.to_string_lossy() != "-" => {
            std::fs::read_to_string(path).context("failed to read file")
        }
        _ => {
            let mut input = String::new();
            io::stdin()
                .read_to_string(&mut input)
                .context("failed to read stdin")?;
            Ok(input)
        }
    }
}

/// Parse import documents from JSON or JSONL.
fn parse_import_documents(input: &str) -> Result<Vec<ImportDocument>> {
    let input = input.trim();

    if input.is_empty() {
        return Ok(Vec::new());
    }

    // Try parsing as JSON array first
    if input.starts_with('[') {
        return serde_json::from_str(input).context("failed to parse JSON array");
    }

    // Try parsing as JSONL (one JSON object per line)
    let mut documents = Vec::new();
    for (i, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let doc: ImportDocument = serde_json::from_str(line)
            .context(format!("failed to parse JSON at line {}", i + 1))?;
        documents.push(doc);
    }

    Ok(documents)
}
