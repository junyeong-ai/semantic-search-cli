use std::fmt::Write as FmtWrite;

use crate::models::{OutputFormat, SearchResults};
use crate::services::MetricsSummary;

pub trait Formatter {
    fn format_search_results(&self, results: &SearchResults) -> String;
    fn format_status(&self, status: &StatusInfo) -> String;
    fn format_index_stats(&self, stats: &IndexStats) -> String;
    fn format_tags(&self, tags: &[(String, u64)]) -> String;
    fn format_sources(&self, sources: &[SourceInfo]) -> String;
    fn format_cli_status(&self, clis: &[CliInfo]) -> String;
    fn format_message(&self, message: &str) -> String;
    fn format_error(&self, error: &str) -> String;
}

#[derive(Debug, Clone)]
pub struct StatusInfo {
    pub daemon_running: bool,
    pub daemon_idle_secs: Option<u64>,
    pub embedding_model: Option<String>,
    pub vector_store_driver: String,
    pub vector_store_url: String,
    pub vector_store_connected: bool,
    pub vector_store_points: u64,
    pub collection: String,
    pub metrics: Option<MetricsSummary>,
}

#[derive(Debug, Clone, Default)]
pub struct IndexStats {
    pub files_scanned: u64,
    pub files_indexed: u64,
    pub files_skipped: u64,
    pub chunks_created: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone)]
pub struct SourceInfo {
    pub name: String,
    pub description: String,
    pub available: bool,
}

#[derive(Debug, Clone)]
pub struct CliInfo {
    pub name: String,
    pub description: String,
    pub available: bool,
    pub version: Option<String>,
}

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
                let tags: Vec<String> = result.tags.iter().map(ToString::to_string).collect();
                writeln!(output, "   Tags: {}", tags.join(", ")).unwrap();
            }
            writeln!(output, "   ---").unwrap();

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
        writeln!(output, "Status").unwrap();
        writeln!(output, "------").unwrap();

        let daemon_status = if status.daemon_running {
            "[RUNNING]"
        } else {
            "[STOPPED]"
        };
        writeln!(output, "ML Daemon:     {}", daemon_status).unwrap();

        if status.daemon_running {
            if let Some(ref model) = status.embedding_model {
                writeln!(output, "  Embedding:   {}", model).unwrap();
            }
            if let Some(idle) = status.daemon_idle_secs {
                writeln!(output, "  Idle:        {}s", idle).unwrap();
            }
            if let Some(ref m) = status.metrics {
                writeln!(output, "  Requests:    {}", m.total_requests).unwrap();
                writeln!(output, "  Avg Latency: {}ms", m.avg_latency_ms).unwrap();
                if m.error_rate > 0.0 {
                    writeln!(output, "  Error Rate:  {:.1}%", m.error_rate).unwrap();
                }
            }
        }
        writeln!(output).unwrap();

        let vector_status = if status.vector_store_connected {
            "[CONNECTED]"
        } else {
            "[DISCONNECTED]"
        };
        writeln!(
            output,
            "Vector Store:  {} ({})",
            status.vector_store_driver, vector_status
        )
        .unwrap();
        if status.vector_store_connected {
            writeln!(output, "  URL:         {}", status.vector_store_url).unwrap();
            writeln!(output, "  Collection:  {}", status.collection).unwrap();
            writeln!(output, "  Points:      {}", status.vector_store_points).unwrap();
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

    fn format_sources(&self, sources: &[SourceInfo]) -> String {
        let mut output = String::new();
        writeln!(output, "Available Data Sources").unwrap();
        writeln!(output, "----------------------").unwrap();
        for src in sources {
            let status = if src.available { "✓" } else { "✗" };
            writeln!(output, "  {} {} - {}", status, src.name, src.description).unwrap();
        }
        output
    }

    fn format_cli_status(&self, clis: &[CliInfo]) -> String {
        let mut output = String::new();
        writeln!(output, "External CLI Status").unwrap();
        writeln!(output, "-------------------").unwrap();
        for cli in clis {
            if cli.available {
                writeln!(output, "✓ {} - {}", cli.name, cli.description).unwrap();
                if let Some(ref ver) = cli.version {
                    writeln!(output, "  Version: {}", ver).unwrap();
                }
            } else {
                writeln!(output, "✗ {} - Not installed", cli.name).unwrap();
                writeln!(output, "  {}", cli.description).unwrap();
            }
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
        let metrics = status.metrics.as_ref().map(|m| {
            serde_json::json!({
                "total_requests": m.total_requests,
                "avg_latency_ms": m.avg_latency_ms,
                "error_rate": m.error_rate,
            })
        });

        let json = serde_json::json!({
            "daemon": {
                "running": status.daemon_running,
                "idle_secs": status.daemon_idle_secs,
                "embedding_model": status.embedding_model,
                "metrics": metrics,
            },
            "vector_store": {
                "driver": status.vector_store_driver,
                "url": status.vector_store_url,
                "connected": status.vector_store_connected,
                "collection": status.collection,
                "points": status.vector_store_points,
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
        let tags_array: Vec<serde_json::Value> = tags
            .iter()
            .map(|(tag, count)| serde_json::json!({"tag": tag, "count": count}))
            .collect();

        let json = serde_json::json!({"tags": tags_array});

        if self.pretty {
            serde_json::to_string_pretty(&json).unwrap()
        } else {
            serde_json::to_string(&json).unwrap()
        }
    }

    fn format_sources(&self, sources: &[SourceInfo]) -> String {
        let sources_array: Vec<serde_json::Value> = sources
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "description": s.description,
                    "available": s.available,
                })
            })
            .collect();

        let json = serde_json::json!({"sources": sources_array});

        if self.pretty {
            serde_json::to_string_pretty(&json).unwrap()
        } else {
            serde_json::to_string(&json).unwrap()
        }
    }

    fn format_cli_status(&self, clis: &[CliInfo]) -> String {
        let clis_array: Vec<serde_json::Value> = clis
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "description": c.description,
                    "available": c.available,
                    "version": c.version,
                })
            })
            .collect();

        let json = serde_json::json!({"clis": clis_array});

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
        writeln!(output, "## Status\n").unwrap();

        let daemon_status = if status.daemon_running { "✅" } else { "❌" };
        writeln!(output, "### ML Daemon {}\n", daemon_status).unwrap();

        if status.daemon_running {
            if let Some(ref model) = status.embedding_model {
                writeln!(output, "- **Embedding:** {}", model).unwrap();
            }
            if let Some(ref m) = status.metrics {
                writeln!(output, "- **Requests:** {}", m.total_requests).unwrap();
                writeln!(output, "- **Avg Latency:** {}ms", m.avg_latency_ms).unwrap();
                if m.error_rate > 0.0 {
                    writeln!(output, "- **Error Rate:** {:.1}%", m.error_rate).unwrap();
                }
            }
        }
        writeln!(output).unwrap();

        let vector_status = if status.vector_store_connected {
            "✅"
        } else {
            "❌"
        };
        writeln!(
            output,
            "### Vector Store ({}) {}\n",
            status.vector_store_driver, vector_status
        )
        .unwrap();
        writeln!(output, "- **URL:** `{}`", status.vector_store_url).unwrap();
        writeln!(output, "- **Collection:** {}", status.collection).unwrap();
        writeln!(output, "- **Points:** {}", status.vector_store_points).unwrap();

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

    fn format_sources(&self, sources: &[SourceInfo]) -> String {
        let mut output = String::new();
        writeln!(output, "## Available Data Sources\n").unwrap();
        writeln!(output, "| Source | Description | Status |").unwrap();
        writeln!(output, "|--------|-------------|--------|").unwrap();
        for src in sources {
            let status = if src.available { "✅" } else { "❌" };
            writeln!(
                output,
                "| `{}` | {} | {} |",
                src.name, src.description, status
            )
            .unwrap();
        }
        output
    }

    fn format_cli_status(&self, clis: &[CliInfo]) -> String {
        let mut output = String::new();
        writeln!(output, "## External CLI Status\n").unwrap();
        for cli in clis {
            let status = if cli.available { "✅" } else { "❌" };
            writeln!(output, "### {} {}\n", cli.name, status).unwrap();
            writeln!(output, "- **Description:** {}", cli.description).unwrap();
            if let Some(ref ver) = cli.version {
                writeln!(output, "- **Version:** {}", ver).unwrap();
            }
            writeln!(output).unwrap();
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

pub fn get_formatter(format: OutputFormat) -> Box<dyn Formatter> {
    match format {
        OutputFormat::Text => Box::new(TextFormatter),
        OutputFormat::Json => Box::new(JsonFormatter::new(true)),
        OutputFormat::Markdown => Box::new(MarkdownFormatter),
    }
}
