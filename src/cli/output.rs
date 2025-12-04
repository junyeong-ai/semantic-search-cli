//! Output formatters for different output formats.

use std::fmt::Write as FmtWrite;

use crate::models::{OutputFormat, SearchResults};

/// Trait for formatting output.
pub trait Formatter {
    /// Format search results.
    fn format_search_results(&self, results: &SearchResults) -> String;

    /// Format a status message.
    fn format_status(&self, status: &StatusInfo) -> String;

    /// Format indexing statistics.
    fn format_index_stats(&self, stats: &IndexStats) -> String;

    /// Format tags list.
    fn format_tags(&self, tags: &[(String, u64)]) -> String;

    /// Format a simple message.
    fn format_message(&self, message: &str) -> String;

    /// Format an error message.
    fn format_error(&self, error: &str) -> String;
}

/// Infrastructure status information.
#[derive(Debug, Clone)]
pub struct StatusInfo {
    pub embedding_url: String,
    pub embedding_connected: bool,
    pub embedding_model: Option<String>,
    pub qdrant_url: String,
    pub qdrant_connected: bool,
    pub qdrant_points: u64,
    pub collection: String,
}

/// Indexing statistics.
#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    pub files_scanned: u64,
    pub files_indexed: u64,
    pub files_skipped: u64,
    pub chunks_created: u64,
    pub duration_ms: u64,
}

/// Text formatter for human-readable output.
pub struct TextFormatter;

impl Formatter for TextFormatter {
    fn format_search_results(&self, results: &SearchResults) -> String {
        if results.is_empty() {
            return format!("No results found for: {}\n", results.query);
        }

        let mut output = String::new();
        writeln!(output, "Search results for: \"{}\"", results.query).unwrap();
        writeln!(
            output,
            "Found {} results in {}ms\n",
            results.total, results.duration_ms
        )
        .unwrap();

        for (i, result) in results.results.iter().enumerate() {
            writeln!(output, "{}. [Score: {:.3}]", i + 1, result.score).unwrap();
            writeln!(output, "   Location: {}", result.location).unwrap();
            if !result.tags.is_empty() {
                let tags: Vec<String> = result.tags.iter().map(|t| t.to_string()).collect();
                writeln!(output, "   Tags: {}", tags.join(", ")).unwrap();
            }
            writeln!(output, "   ---").unwrap();

            // Show content preview (first 200 chars, UTF-8 safe)
            let preview: String = result.content.chars().take(200).collect();
            let preview = if result.content.chars().count() > 200 {
                format!("{}...", preview)
            } else {
                preview
            };
            for line in preview.lines() {
                writeln!(output, "   {}", line).unwrap();
            }
            writeln!(output).unwrap();
        }

        output
    }

    fn format_status(&self, status: &StatusInfo) -> String {
        let mut output = String::new();
        writeln!(output, "Infrastructure Status").unwrap();
        writeln!(output, "---------------------").unwrap();

        let embedding_status = if status.embedding_connected {
            "[CONNECTED]"
        } else {
            "[DISCONNECTED]"
        };
        writeln!(
            output,
            "Embedding:   {}  {}",
            status.embedding_url, embedding_status
        )
        .unwrap();
        if let Some(ref model) = status.embedding_model {
            writeln!(output, "  Model:     {}", model).unwrap();
        }
        if status.embedding_connected {
            writeln!(output, "  Status:    healthy").unwrap();
        }
        writeln!(output).unwrap();

        let qdrant_status = if status.qdrant_connected {
            "[CONNECTED]"
        } else {
            "[DISCONNECTED]"
        };
        writeln!(
            output,
            "Qdrant:      {}  {}",
            status.qdrant_url, qdrant_status
        )
        .unwrap();
        if status.qdrant_connected {
            writeln!(output, "  Collection: {}", status.collection).unwrap();
            writeln!(output, "  Points:    {}", status.qdrant_points).unwrap();
        }

        output
    }

    fn format_index_stats(&self, stats: &IndexStats) -> String {
        let mut output = String::new();
        writeln!(output, "Indexing Complete").unwrap();
        writeln!(output, "-----------------").unwrap();
        writeln!(output, "Files scanned: {}", stats.files_scanned).unwrap();
        writeln!(output, "Files indexed: {}", stats.files_indexed).unwrap();
        writeln!(output, "Files skipped: {}", stats.files_skipped).unwrap();
        writeln!(output, "Chunks created: {}", stats.chunks_created).unwrap();
        writeln!(output, "Duration: {}ms", stats.duration_ms).unwrap();
        output
    }

    fn format_tags(&self, tags: &[(String, u64)]) -> String {
        if tags.is_empty() {
            return "No tags found.\n".to_string();
        }

        let mut output = String::new();
        writeln!(output, "Tags").unwrap();
        writeln!(output, "----").unwrap();
        for (tag, count) in tags {
            writeln!(output, "  {} ({})", tag, count).unwrap();
        }
        output
    }

    fn format_message(&self, message: &str) -> String {
        format!("{}\n", message)
    }

    fn format_error(&self, error: &str) -> String {
        format!("Error: {}\n", error)
    }
}

/// JSON formatter for machine-readable output.
pub struct JsonFormatter {
    pub pretty: bool,
}

impl JsonFormatter {
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }
}

impl Formatter for JsonFormatter {
    fn format_search_results(&self, results: &SearchResults) -> String {
        if self.pretty {
            serde_json::to_string_pretty(results)
                .unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
        } else {
            serde_json::to_string(results).unwrap_or_else(|e| format!("{{\"error\": \"{}\"}}", e))
        }
    }

    fn format_status(&self, status: &StatusInfo) -> String {
        let json = serde_json::json!({
            "embedding": {
                "url": status.embedding_url,
                "connected": status.embedding_connected,
                "model": status.embedding_model,
            },
            "qdrant": {
                "url": status.qdrant_url,
                "connected": status.qdrant_connected,
                "collection": status.collection,
                "points": status.qdrant_points,
            }
        });

        if self.pretty {
            serde_json::to_string_pretty(&json).unwrap()
        } else {
            serde_json::to_string(&json).unwrap()
        }
    }

    fn format_index_stats(&self, stats: &IndexStats) -> String {
        let json = serde_json::json!({
            "files_scanned": stats.files_scanned,
            "files_indexed": stats.files_indexed,
            "files_skipped": stats.files_skipped,
            "chunks_created": stats.chunks_created,
            "duration_ms": stats.duration_ms,
        });

        if self.pretty {
            serde_json::to_string_pretty(&json).unwrap()
        } else {
            serde_json::to_string(&json).unwrap()
        }
    }

    fn format_tags(&self, tags: &[(String, u64)]) -> String {
        let json: Vec<serde_json::Value> = tags
            .iter()
            .map(|(tag, count)| {
                serde_json::json!({
                    "tag": tag,
                    "count": count,
                })
            })
            .collect();

        if self.pretty {
            serde_json::to_string_pretty(&json).unwrap()
        } else {
            serde_json::to_string(&json).unwrap()
        }
    }

    fn format_message(&self, message: &str) -> String {
        serde_json::json!({"message": message}).to_string()
    }

    fn format_error(&self, error: &str) -> String {
        serde_json::json!({"error": error}).to_string()
    }
}

/// Markdown formatter for documentation-friendly output.
pub struct MarkdownFormatter;

impl Formatter for MarkdownFormatter {
    fn format_search_results(&self, results: &SearchResults) -> String {
        if results.is_empty() {
            return format!("## No results found\n\nQuery: `{}`\n", results.query);
        }

        let mut output = String::new();
        writeln!(output, "## Search Results\n").unwrap();
        writeln!(output, "**Query:** `{}`\n", results.query).unwrap();
        writeln!(
            output,
            "Found {} results in {}ms\n",
            results.total, results.duration_ms
        )
        .unwrap();

        for (i, result) in results.results.iter().enumerate() {
            writeln!(output, "### {}. Score: {:.3}\n", i + 1, result.score).unwrap();
            writeln!(output, "**Location:** `{}`\n", result.location).unwrap();
            if !result.tags.is_empty() {
                let tags: Vec<String> = result.tags.iter().map(|t| format!("`{}`", t)).collect();
                writeln!(output, "**Tags:** {}\n", tags.join(", ")).unwrap();
            }
            writeln!(output, "```").unwrap();
            writeln!(output, "{}", result.content).unwrap();
            writeln!(output, "```\n").unwrap();
        }

        output
    }

    fn format_status(&self, status: &StatusInfo) -> String {
        let mut output = String::new();
        writeln!(output, "## Infrastructure Status\n").unwrap();

        let embedding_status = if status.embedding_connected {
            "✅"
        } else {
            "❌"
        };
        writeln!(output, "### Embedding Server {}\n", embedding_status).unwrap();
        writeln!(output, "- **URL:** `{}`", status.embedding_url).unwrap();
        if let Some(ref model) = status.embedding_model {
            writeln!(output, "- **Model:** {}", model).unwrap();
        }
        writeln!(output).unwrap();

        let qdrant_status = if status.qdrant_connected {
            "✅"
        } else {
            "❌"
        };
        writeln!(output, "### Qdrant {}\n", qdrant_status).unwrap();
        writeln!(output, "- **URL:** `{}`", status.qdrant_url).unwrap();
        writeln!(output, "- **Collection:** {}", status.collection).unwrap();
        writeln!(output, "- **Points:** {}", status.qdrant_points).unwrap();

        output
    }

    fn format_index_stats(&self, stats: &IndexStats) -> String {
        let mut output = String::new();
        writeln!(output, "## Indexing Complete\n").unwrap();
        writeln!(output, "| Metric | Value |").unwrap();
        writeln!(output, "|--------|-------|").unwrap();
        writeln!(output, "| Files scanned | {} |", stats.files_scanned).unwrap();
        writeln!(output, "| Files indexed | {} |", stats.files_indexed).unwrap();
        writeln!(output, "| Files skipped | {} |", stats.files_skipped).unwrap();
        writeln!(output, "| Chunks created | {} |", stats.chunks_created).unwrap();
        writeln!(output, "| Duration | {}ms |", stats.duration_ms).unwrap();
        output
    }

    fn format_tags(&self, tags: &[(String, u64)]) -> String {
        if tags.is_empty() {
            return "## Tags\n\n*No tags found.*\n".to_string();
        }

        let mut output = String::new();
        writeln!(output, "## Tags\n").unwrap();
        writeln!(output, "| Tag | Count |").unwrap();
        writeln!(output, "|-----|-------|").unwrap();
        for (tag, count) in tags {
            writeln!(output, "| `{}` | {} |", tag, count).unwrap();
        }
        output
    }

    fn format_message(&self, message: &str) -> String {
        format!("> {}\n", message)
    }

    fn format_error(&self, error: &str) -> String {
        format!("> ⚠️ **Error:** {}\n", error)
    }
}

/// Get a formatter for the given output format.
pub fn get_formatter(format: OutputFormat) -> Box<dyn Formatter> {
    match format {
        OutputFormat::Text => Box::new(TextFormatter),
        OutputFormat::Json => Box::new(JsonFormatter::new(true)),
        OutputFormat::Markdown => Box::new(MarkdownFormatter),
    }
}
