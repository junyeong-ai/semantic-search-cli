//! Index command implementation.

use anyhow::{Context, Result};
use clap::Args;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;

use crate::cli::output::{IndexStats, get_formatter};
use crate::models::{Config, Document, DocumentMetadata, OutputFormat, Source, Tag, parse_tags};
use crate::services::{EmbeddingClient, TextChunker, VectorStoreClient, process_batch};
use crate::utils::file::{calculate_checksum, is_text_file, read_file_content};

/// Arguments for the index command.
#[derive(Debug, Args)]
pub struct IndexArgs {
    /// Path to directory or file to index
    #[arg(required = true)]
    pub path: PathBuf,

    /// Tags to apply to indexed documents (comma-separated, format: key:value)
    #[arg(long, short = 't')]
    pub tags: Option<String>,

    /// File patterns to exclude (can be specified multiple times)
    #[arg(long, short = 'e')]
    pub exclude: Vec<String>,

    /// Show what would be indexed without actually indexing
    #[arg(long)]
    pub dry_run: bool,

    /// Force re-indexing of all files (ignore checksums)
    #[arg(long)]
    pub force: bool,
}

/// Handle the index command.
pub async fn handle_index(args: IndexArgs, format: OutputFormat, verbose: bool) -> Result<()> {
    let config = Config::load()?;
    let formatter = get_formatter(format);
    let start_time = Instant::now();

    // Parse tags
    let tags: Vec<Tag> = if let Some(ref tag_str) = args.tags {
        parse_tags(tag_str).context("failed to parse tags")?
    } else {
        Vec::new()
    };

    // Validate path
    let path = args.path.canonicalize().context("invalid path")?;
    if !path.exists() {
        anyhow::bail!("path does not exist: {}", path.display());
    }

    // Collect files to index
    let files = collect_files(&path, &args.exclude, &config.indexing.exclude_patterns)?;

    if files.is_empty() {
        println!("{}", formatter.format_message("No files found to index."));
        return Ok(());
    }

    if verbose {
        println!("Found {} files to process", files.len());
    }

    if args.dry_run {
        println!(
            "{}",
            formatter.format_message(&format!("Dry run: Would index {} files", files.len()))
        );
        for file in &files {
            println!("  {}", file.display());
        }
        return Ok(());
    }

    // Initialize clients
    let embedding_client = EmbeddingClient::new(&config.embedding)?;
    let vector_client = VectorStoreClient::new(&config.vector_store).await?;

    // Ensure collection exists
    vector_client.create_collection().await?;

    // Create chunker
    let chunker = TextChunker::new(&config.indexing);

    // Setup progress bar
    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
    );

    let mut stats = IndexStats {
        files_scanned: files.len() as u64,
        ..Default::default()
    };

    // Process files in batches
    let batch_size = config.embedding.batch_size as usize;
    let mut pending_chunks = Vec::new();
    let mut pending_texts = Vec::new();

    for file_path in &files {
        pb.inc(1);

        // Check if file is text
        if !is_text_file(file_path) {
            stats.files_skipped += 1;
            continue;
        }

        // Read file content
        let content = match read_file_content(file_path, config.indexing.max_file_size) {
            Ok(c) => c,
            Err(e) => {
                if verbose {
                    pb.println(format!("Skipping {}: {}", file_path.display(), e));
                }
                stats.files_skipped += 1;
                continue;
            }
        };

        if content.is_empty() {
            stats.files_skipped += 1;
            continue;
        }

        // Calculate checksum
        let checksum = calculate_checksum(&content);

        // Create document
        let source = Source::local(file_path.to_string_lossy().to_string());
        let metadata = DocumentMetadata {
            filename: file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string()),
            extension: file_path
                .extension()
                .map(|e| e.to_string_lossy().to_string()),
            language: detect_language(file_path),
            title: None,
            size_bytes: content.len() as u64,
        };

        let document = Document::new(content, source, tags.clone(), checksum, metadata);

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

    pb.finish_and_clear();

    stats.duration_ms = start_time.elapsed().as_millis() as u64;

    print!("{}", formatter.format_index_stats(&stats));

    Ok(())
}

/// Collect files to index from the given path.
fn collect_files(
    path: &PathBuf,
    exclude: &[String],
    default_exclude: &[String],
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if path.is_file() {
        files.push(path.clone());
        return Ok(files);
    }

    for entry in WalkDir::new(path).follow_links(false) {
        let entry = entry.context("failed to read directory entry")?;
        let entry_path = entry.path();

        if !entry_path.is_file() {
            continue;
        }

        // Check exclusions
        let path_str = entry_path.to_string_lossy();
        let mut excluded = false;

        for pattern in exclude.iter().chain(default_exclude.iter()) {
            if glob::Pattern::new(pattern)
                .map(|p| p.matches(&path_str))
                .unwrap_or(false)
            {
                excluded = true;
                break;
            }
        }

        if !excluded {
            files.push(entry_path.to_path_buf());
        }
    }

    Ok(files)
}

/// Detect programming language from file extension.
fn detect_language(path: &Path) -> Option<String> {
    path.extension().and_then(|ext| {
        let ext = ext.to_string_lossy().to_lowercase();
        match ext.as_str() {
            "rs" => Some("rust"),
            "py" => Some("python"),
            "js" => Some("javascript"),
            "ts" => Some("typescript"),
            "jsx" => Some("javascript"),
            "tsx" => Some("typescript"),
            "go" => Some("go"),
            "java" => Some("java"),
            "kt" | "kts" => Some("kotlin"),
            "c" | "h" => Some("c"),
            "cpp" | "hpp" | "cc" | "cxx" => Some("cpp"),
            "rb" => Some("ruby"),
            "php" => Some("php"),
            "swift" => Some("swift"),
            "scala" => Some("scala"),
            "sh" | "bash" => Some("shell"),
            "sql" => Some("sql"),
            "html" | "htm" => Some("html"),
            "css" | "scss" | "sass" => Some("css"),
            "json" => Some("json"),
            "yaml" | "yml" => Some("yaml"),
            "toml" => Some("toml"),
            "xml" => Some("xml"),
            "md" | "markdown" => Some("markdown"),
            _ => None,
        }
        .map(String::from)
    })
}
