use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

use crate::error::SourceError;
use crate::models::{Document, DocumentMetadata, Source, SourceType, Tag};
use crate::sources::SyncOptions;
use crate::utils::file::{calculate_checksum, sanitize_filename};
use crate::utils::has_meaningful_content;

#[derive(Debug, Deserialize)]
struct ConfluencePage {
    id: String,
    title: String,
    body: Option<Body>,
    ancestors: Option<Vec<Ancestor>>,
    #[serde(rename = "_links")]
    links: Option<Links>,
}

#[derive(Debug, Deserialize)]
struct Body {
    storage: Option<StorageBody>,
}

#[derive(Debug, Deserialize)]
struct StorageBody {
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Ancestor {
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Links {
    webui: Option<String>,
    base: Option<String>,
}

#[derive(Debug)]
pub struct ConfluenceSource;

impl ConfluenceSource {
    pub fn new() -> Self {
        Self
    }

    pub fn source_type(&self) -> SourceType {
        SourceType::Confluence
    }

    pub fn name(&self) -> &str {
        "Confluence"
    }

    pub fn check_available(&self) -> Result<bool, SourceError> {
        Command::new("which")
            .arg("atlassian-cli")
            .output()
            .map(|o| o.status.success())
            .map_err(|e| SourceError::ExecutionError(e.to_string()))
    }

    pub fn install_instructions(&self) -> &str {
        "Install atlassian-cli: cargo install atlassian-cli"
    }

    pub fn sync(&self, options: SyncOptions) -> Result<Vec<Document>, SourceError> {
        if !self.check_available()? {
            return Err(SourceError::CliNotFound(
                "atlassian-cli not found. Install with: cargo install atlassian-cli".to_string(),
            ));
        }

        if let Some(ref space) = options.space {
            return self.sync_space(space, &options);
        }

        let query = options.query.as_deref().unwrap_or("type=page");

        if let Some(page_id) = extract_page_id(query) {
            return self
                .fetch_page(&page_id, &options.tags)
                .map(|doc| vec![doc]);
        }

        self.sync_by_query(query, &options)
    }

    fn sync_space(
        &self,
        space: &str,
        options: &SyncOptions,
    ) -> Result<Vec<Document>, SourceError> {
        let cql = format!("space=\"{}\" AND type=page", space);
        self.fetch_pages_streaming(&cql, options)
    }

    fn sync_by_query(
        &self,
        query: &str,
        options: &SyncOptions,
    ) -> Result<Vec<Document>, SourceError> {
        self.fetch_pages_streaming(query, options)
    }

    fn fetch_pages_streaming(
        &self,
        cql: &str,
        options: &SyncOptions,
    ) -> Result<Vec<Document>, SourceError> {
        let excluded_ids = self.get_excluded_ids(&options.exclude_ancestors)?;

        if options.limit.is_some() {
            return self.fetch_pages_batch(cql, options, &excluded_ids);
        }

        let args = [
            "confluence",
            "search",
            cql,
            "--format",
            "markdown",
            "--expand",
            "body.storage,ancestors",
            "--all",
            "--stream",
        ];

        eprintln!("Running: atlassian-cli {}", args.join(" "));

        let mut child = Command::new("atlassian-cli")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SourceError::ExecutionError("failed to capture stdout".to_string()))?;

        let reader = BufReader::new(stdout);
        let mut documents = Vec::new();
        let mut skipped = 0;

        for line in reader.lines() {
            let line = match line {
                Ok(l) if !l.trim().is_empty() => l,
                _ => continue,
            };

            let page: ConfluencePage = match serde_json::from_str(&line) {
                Ok(p) => p,
                Err(_) => continue,
            };

            if excluded_ids.contains(&page.id) {
                skipped += 1;
                continue;
            }

            match self.page_to_document(page, &options.tags) {
                Ok(doc) => {
                    documents.push(doc);
                    if documents.len() % 50 == 0 {
                        eprintln!("  Processed {} pages...", documents.len());
                    }
                }
                Err(_) => skipped += 1,
            }
        }

        let status = child.wait().map_err(|e| SourceError::ExecutionError(e.to_string()))?;
        if !status.success() {
            let stderr = child
                .stderr
                .map(|mut s| {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut s, &mut buf).ok();
                    buf
                })
                .unwrap_or_default();
            if !stderr.is_empty() {
                eprintln!("Warning: {}", stderr.trim());
            }
        }

        if skipped > 0 {
            eprintln!("  Skipped {} pages (excluded or empty)", skipped);
        }

        Ok(documents)
    }

    fn fetch_pages_batch(
        &self,
        cql: &str,
        options: &SyncOptions,
        excluded_ids: &HashSet<String>,
    ) -> Result<Vec<Document>, SourceError> {
        let limit = options.limit.unwrap_or(100);
        let limit_str = limit.to_string();

        let args = [
            "confluence",
            "search",
            cql,
            "--format",
            "markdown",
            "--expand",
            "body.storage,ancestors",
            "--limit",
            &limit_str,
        ];

        eprintln!("Running: atlassian-cli {}", args.join(" "));

        let output = Command::new("atlassian-cli")
            .args(&args)
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "confluence search failed: {}",
                stderr
            )));
        }

        #[derive(Deserialize)]
        struct SearchResponse {
            items: Vec<ConfluencePage>,
        }

        let response: SearchResponse = serde_json::from_slice(&output.stdout)
            .map_err(|e| SourceError::ParseError(format!("failed to parse response: {}", e)))?;

        let mut documents = Vec::new();
        let mut skipped = 0;

        for page in response.items {
            if excluded_ids.contains(&page.id) {
                skipped += 1;
                continue;
            }

            match self.page_to_document(page, &options.tags) {
                Ok(doc) => documents.push(doc),
                Err(_) => skipped += 1,
            }
        }

        if skipped > 0 {
            eprintln!("  Skipped {} pages (excluded or empty)", skipped);
        }

        Ok(documents)
    }

    fn fetch_page(&self, page_id: &str, tags: &[Tag]) -> Result<Document, SourceError> {
        let output = Command::new("atlassian-cli")
            .args(["confluence", "get", page_id, "--format", "markdown"])
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "confluence get failed: {}",
                stderr
            )));
        }

        let page: ConfluencePage = serde_json::from_slice(&output.stdout)
            .map_err(|e| SourceError::ParseError(format!("failed to parse page: {}", e)))?;

        self.page_to_document(page, tags)
    }

    fn get_excluded_ids(&self, exclude_ancestors: &[String]) -> Result<HashSet<String>, SourceError> {
        let mut excluded = HashSet::new();

        for ancestor_id in exclude_ancestors {
            excluded.insert(ancestor_id.clone());

            let output = Command::new("atlassian-cli")
                .args([
                    "confluence",
                    "search",
                    &format!("ancestor={}", ancestor_id),
                    "--all",
                ])
                .output()
                .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

            if output.status.success() {
                #[derive(Deserialize)]
                struct SearchItem {
                    content: ContentRef,
                }
                #[derive(Deserialize)]
                struct ContentRef {
                    id: String,
                }
                #[derive(Deserialize)]
                struct SearchResults {
                    items: Vec<SearchItem>,
                }

                if let Ok(results) = serde_json::from_slice::<SearchResults>(&output.stdout) {
                    for item in results.items {
                        excluded.insert(item.content.id);
                    }
                }
            }
        }

        if !excluded.is_empty() {
            eprintln!("  Excluding {} pages (ancestor filter)", excluded.len());
        }

        Ok(excluded)
    }

    fn page_to_document(&self, page: ConfluencePage, tags: &[Tag]) -> Result<Document, SourceError> {
        let raw_content = page
            .body
            .as_ref()
            .and_then(|b| b.storage.as_ref())
            .and_then(|s| s.value.clone())
            .unwrap_or_default();

        let cleaned_content = clean_markdown(&raw_content);
        if !has_meaningful_content(&cleaned_content) {
            return Err(SourceError::ParseError(format!(
                "page {} has no meaningful content",
                page.id
            )));
        }

        let path = build_page_path(&page);
        let full_content = format!("# {}\n\n{}", page.title, cleaned_content);

        let url = page.links.as_ref().map_or_else(
            || page.id.clone(),
            |l| {
                let base = l.base.as_deref().unwrap_or("");
                let webui = l.webui.as_deref().unwrap_or("");
                format!("{}{}", base, webui)
            },
        );

        let source = Source::external(SourceType::Confluence, page.id.clone(), url);
        let checksum = calculate_checksum(&full_content);

        let metadata = DocumentMetadata {
            filename: Some(format!("{}.md", sanitize_filename(&page.title))),
            extension: Some("md".to_string()),
            language: Some("markdown".to_string()),
            title: Some(page.title.clone()),
            path: Some(path),
            size_bytes: full_content.len() as u64,
        };

        let mut all_tags = tags.to_vec();
        if let Ok(tag) = "source:confluence".parse() {
            all_tags.push(tag);
        }

        Ok(Document::new(
            full_content,
            source,
            all_tags,
            checksum,
            metadata,
        ))
    }
}

impl Default for ConfluenceSource {
    fn default() -> Self {
        Self::new()
    }
}

fn extract_page_id(query: &str) -> Option<String> {
    let query = query.trim();

    if query.contains("atlassian.net/wiki") || query.contains("/pages/") {
        return query
            .split("/pages/")
            .nth(1)
            .and_then(|rest| rest.split('/').next())
            .filter(|id| is_numeric_id(id))
            .map(String::from);
    }

    if is_numeric_id(query) {
        return Some(query.to_string());
    }

    None
}

fn is_numeric_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_digit())
}

fn build_page_path(page: &ConfluencePage) -> String {
    let ancestors: Vec<&str> = page
        .ancestors
        .as_ref()
        .map(|a| {
            a.iter()
                .filter_map(|anc| anc.title.as_deref())
                .collect()
        })
        .unwrap_or_default();

    let mut parts = ancestors;
    parts.push(&page.title);
    parts.join(" > ")
}

static RE_MACRO_METADATA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\|[^|\n]*[^\s|]{500,}[^|\n]*\|").unwrap());
static RE_EMPTY_TABLE_ROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\|[\s|]*\|[\s|]*$\n?").unwrap());
static RE_MULTI_BLANK_LINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

fn clean_markdown(content: &str) -> String {
    let cleaned = RE_MACRO_METADATA.replace_all(content, "|");
    let cleaned = RE_EMPTY_TABLE_ROW.replace_all(&cleaned, "");
    let cleaned = RE_MULTI_BLANK_LINES.replace_all(&cleaned, "\n\n");
    cleaned.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_confluence_source_creation() {
        let source = ConfluenceSource::new();
        assert_eq!(source.source_type(), SourceType::Confluence);
        assert_eq!(source.name(), "Confluence");
    }

    #[test]
    fn test_clean_markdown_removes_macro_metadata() {
        let content = "## Title\n\nSome text\n\n| Header |\n| ---- |\n| abc123def456ghi789"
            .to_owned()
            + &"x".repeat(600)
            + " |\n\nMore content";
        let cleaned = clean_markdown(&content);
        assert!(cleaned.contains("Title"));
        assert!(cleaned.contains("More content"));
        assert!(!cleaned.contains(&"x".repeat(100)));
    }

    #[test]
    fn test_extract_page_id() {
        assert_eq!(extract_page_id("12345678"), Some("12345678".to_string()));
        assert_eq!(extract_page_id("12345"), Some("12345".to_string()));

        let url = "https://example.atlassian.net/wiki/spaces/DEV/pages/12345678/Page+Title";
        assert_eq!(extract_page_id(url), Some("12345678".to_string()));

        let url2 = "https://example.atlassian.net/wiki/spaces/DEV/pages/12345";
        assert_eq!(extract_page_id(url2), Some("12345".to_string()));

        assert_eq!(extract_page_id("space=COMMON"), None);
        assert_eq!(extract_page_id("type=page"), None);
        assert_eq!(extract_page_id("text~hello"), None);
    }

    #[test]
    fn test_build_page_path() {
        let page = ConfluencePage {
            id: "123".to_string(),
            title: "My Page".to_string(),
            body: None,
            ancestors: Some(vec![
                Ancestor {
                    title: Some("Root".to_string()),
                },
                Ancestor {
                    title: Some("Parent".to_string()),
                },
            ]),
            links: None,
        };
        assert_eq!(build_page_path(&page), "Root > Parent > My Page");
    }
}
