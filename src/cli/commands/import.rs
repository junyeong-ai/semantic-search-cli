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
use crate::services::{EmbeddingClient, TextChunker, create_backend, process_batch};

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

#[derive(Debug, Deserialize)]
pub struct ImportDocument {
    pub content: String,
    #[serde(default)]
    pub url: Option<String>,
    pub title: Option<String>,
    pub path: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub source_type: Option<String>,
}

pub async fn handle_import(args: ImportArgs, format: OutputFormat, verbose: bool) -> Result<()> {
    let config = Config::load()?.config;
    let formatter = get_formatter(format);
    let start_time = Instant::now();

    let tags: Vec<Tag> = if let Some(ref tag_str) = args.tags {
        parse_tags(tag_str).context("failed to parse tags")?
    } else {
        Vec::new()
    };

    let input = read_input(args.file.as_deref())?;
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

    let embedding_client = EmbeddingClient::new(&config);
    let vector_store = create_backend(&config.vector_store).await?;
    vector_store.create_collection().await?;

    let chunker = TextChunker::new(&config.indexing);

    let mut stats = IndexStats {
        files_scanned: import_docs.len() as u64,
        ..Default::default()
    };

    let batch_size = config.embedding.batch_size as usize;
    let mut pending_chunks = Vec::new();
    let mut pending_texts = Vec::new();

    for import_doc in import_docs {
        if import_doc.content.is_empty() {
            stats.files_skipped += 1;
            continue;
        }

        let checksum = {
            use sha2::{Digest, Sha256};
            let hash = Sha256::digest(import_doc.content.as_bytes());
            hex::encode(hash)
        };

        // Parse source_type - never fails, defaults to Other("custom")
        let source_type: SourceType = import_doc
            .source_type
            .as_deref()
            .map(|s| s.parse().unwrap())
            .unwrap_or_else(|| SourceType::Other("custom".to_string()));

        // Location: url > path > checksum
        let location = import_doc
            .url
            .clone()
            .or_else(|| import_doc.path.clone())
            .unwrap_or_else(|| checksum.clone());

        let source = Source::new(source_type, location, import_doc.url.clone());

        let metadata = DocumentMetadata {
            filename: None,
            extension: None,
            language: None,
            title: import_doc.title.clone(),
            path: import_doc.path.clone(),
            size_bytes: import_doc.content.len() as u64,
        };

        let mut doc_tags = tags.clone();
        for tag_str in &import_doc.tags {
            if let Ok(tag) = tag_str.parse::<Tag>()
                && !doc_tags.iter().any(|t| t.to_string() == tag.to_string())
            {
                doc_tags.push(tag);
            }
        }

        let document = Document::new(import_doc.content, source, doc_tags, checksum, metadata);

        let chunks = chunker.chunk(&document);
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

    stats.duration_ms = start_time.elapsed().as_millis() as u64;
    print!("{}", formatter.format_index_stats(&stats));

    Ok(())
}

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

fn parse_import_documents(input: &str) -> Result<Vec<ImportDocument>> {
    let input = input.trim();

    if input.is_empty() {
        return Ok(Vec::new());
    }

    if input.starts_with('[') {
        return serde_json::from_str(input).context("failed to parse JSON array");
    }

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
