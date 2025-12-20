//! Index command implementation.

use anyhow::{Context, Result};
use clap::Subcommand;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use std::time::Instant;
use walkdir::WalkDir;

use crate::cli::output::{IndexStats, get_formatter};
use crate::models::{
    Config, Document, DocumentMetadata, OutputFormat, Source, SourceType, Tag, parse_tags,
};
use crate::services::{EmbeddingClient, TextChunker, create_backend, process_batch};
use crate::utils::file::{calculate_checksum, is_text_file, read_file_content};

#[derive(Debug, Subcommand)]
pub enum IndexCommand {
    /// Add files or directories to the search index
    Add {
        /// Path to directory or file to index
        #[arg(required = true)]
        path: PathBuf,

        /// Tags to apply to indexed documents (comma-separated, format: key:value)
        #[arg(long, short = 't')]
        tags: Option<String>,

        /// File patterns to exclude (can be specified multiple times)
        #[arg(long, short = 'e')]
        exclude: Vec<String>,

        /// Show what would be indexed without actually indexing
        #[arg(long)]
        dry_run: bool,
    },

    /// Delete indexed documents by path
    Delete {
        /// Path to file or directory to remove from index
        #[arg(required = true)]
        path: PathBuf,

        /// Show what would be deleted without actually deleting
        #[arg(long)]
        dry_run: bool,

        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        force: bool,
    },

    /// Clear all indexed documents
    Clear {
        /// Skip confirmation prompt
        #[arg(long, short = 'y')]
        force: bool,
    },
}

pub async fn handle_index(cmd: IndexCommand, format: OutputFormat, verbose: bool) -> Result<()> {
    match cmd {
        IndexCommand::Add {
            path,
            tags,
            exclude,
            dry_run,
        } => handle_add(path, tags, exclude, dry_run, format, verbose).await,
        IndexCommand::Delete {
            path,
            dry_run,
            force,
        } => handle_delete(path, dry_run, force, format, verbose).await,
        IndexCommand::Clear { force } => handle_clear(force, format, verbose).await,
    }
}

async fn handle_add(
    path: PathBuf,
    tags: Option<String>,
    exclude: Vec<String>,
    dry_run: bool,
    format: OutputFormat,
    verbose: bool,
) -> Result<()> {
    let config = Config::load()?;
    let formatter = get_formatter(format);
    let start_time = Instant::now();

    let tags: Vec<Tag> = if let Some(ref tag_str) = tags {
        parse_tags(tag_str).context("failed to parse tags")?
    } else {
        Vec::new()
    };

    let path = path.canonicalize().context("invalid path")?;
    if !path.exists() {
        anyhow::bail!("path does not exist: {}", path.display());
    }

    let files = collect_files(&path, &exclude, &config.indexing.exclude_patterns)?;

    if files.is_empty() {
        println!("{}", formatter.format_message("No files found to index."));
        return Ok(());
    }

    if verbose {
        println!("Found {} files to process", files.len());
    }

    if dry_run {
        println!(
            "{}",
            formatter.format_message(&format!("Dry run: Would index {} files", files.len()))
        );
        for file in &files {
            println!("  {}", file.display());
        }
        return Ok(());
    }

    let embedding_client = EmbeddingClient::new(&config);
    let vector_store = create_backend(&config.vector_store).await?;
    vector_store.create_collection().await?;

    let chunker = TextChunker::new(&config.indexing);

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

    let batch_size = config.embedding.batch_size as usize;
    let mut pending_chunks = Vec::new();
    let mut pending_texts = Vec::new();

    for file_path in &files {
        pb.inc(1);

        if !is_text_file(file_path) {
            stats.files_skipped += 1;
            continue;
        }

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

        let checksum = calculate_checksum(&content);
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
            path: Some(file_path.to_string_lossy().to_string()),
            size_bytes: content.len() as u64,
        };

        let document = Document::new(content, source, tags.clone(), checksum, metadata);
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

    pb.finish_and_clear();
    stats.duration_ms = start_time.elapsed().as_millis() as u64;
    print!("{}", formatter.format_index_stats(&stats));

    Ok(())
}

async fn handle_delete(
    path: PathBuf,
    dry_run: bool,
    force: bool,
    format: OutputFormat,
    verbose: bool,
) -> Result<()> {
    let config = Config::load()?;
    let formatter = get_formatter(format);

    let path = path.canonicalize().context("invalid path")?;
    let path_str = path.to_string_lossy().to_string();

    if verbose {
        println!("Deleting indexed documents for: {}", path_str);
    }

    if dry_run {
        println!(
            "{}",
            formatter.format_message(&format!(
                "Dry run: Would delete documents matching '{}'",
                path_str
            ))
        );
        return Ok(());
    }

    if !force {
        println!(
            "This will delete all indexed documents matching '{}'. Continue? [y/N]",
            path_str
        );
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("{}", formatter.format_message("Cancelled."));
            return Ok(());
        }
    }

    let vector_store = create_backend(&config.vector_store).await?;

    let files = if path.is_file() {
        vec![path.clone()]
    } else {
        collect_files(&path, &[], &[])?
    };

    if files.is_empty() {
        println!("{}", formatter.format_message("No documents to delete."));
        return Ok(());
    }

    let document_ids: Vec<String> = files
        .iter()
        .map(|p| {
            let source = Source {
                source_type: SourceType::Local,
                location: p.to_string_lossy().to_string(),
                url: None,
            };
            Document::generate_id(&source)
        })
        .collect();

    vector_store.delete_by_document_ids(&document_ids).await?;

    println!(
        "{}",
        formatter.format_message(&format!("Deleted {} document(s) from index", files.len()))
    );

    Ok(())
}

async fn handle_clear(force: bool, format: OutputFormat, verbose: bool) -> Result<()> {
    let config = Config::load()?;
    let formatter = get_formatter(format);

    if verbose {
        println!("Clearing all indexed documents...");
    }

    if !force {
        println!("This will delete ALL indexed documents. Continue? [y/N]");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("{}", formatter.format_message("Cancelled."));
            return Ok(());
        }
    }

    let vector_store = create_backend(&config.vector_store).await?;
    vector_store.clear_collection().await?;

    println!(
        "{}",
        formatter.format_message("All indexed documents have been cleared.")
    );

    Ok(())
}

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

fn detect_language(path: &Path) -> Option<String> {
    path.extension().and_then(|ext| {
        let ext = ext.to_string_lossy().to_lowercase();
        match ext.as_str() {
            "rs" => Some("rust"),
            "py" => Some("python"),
            "js" | "jsx" => Some("javascript"),
            "ts" | "tsx" => Some("typescript"),
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
