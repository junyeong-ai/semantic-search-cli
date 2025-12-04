use std::process::Command;

use serde::Deserialize;

use crate::error::SourceError;
use crate::models::{Document, DocumentMetadata, Source, SourceType, Tag};
use crate::sources::SyncOptions;
use crate::utils::file::{calculate_checksum, sanitize_filename};

#[derive(Debug, Deserialize)]
struct SearchResultItem {
    content: SearchContent,
}

#[derive(Debug, Deserialize)]
struct SearchContent {
    id: String,
    #[serde(rename = "type")]
    content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SearchResults {
    items: Vec<SearchResultItem>,
}

#[derive(Debug, Deserialize)]
struct ConfluencePage {
    id: String,
    title: String,
    body: Option<Body>,
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
        let output = Command::new("which")
            .arg("atlassian-cli")
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        Ok(output.status.success())
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

        let query = options.query.as_deref().unwrap_or("type=page");

        if let Some(page_id) = extract_page_id(query) {
            return match self.fetch_page(&page_id, &options.tags) {
                Ok(doc) => Ok(vec![doc]),
                Err(e) => Err(e),
            };
        }

        let limit = options.limit.unwrap_or(10);

        let search_output = Command::new("atlassian-cli")
            .args(["confluence", "search", query, "--limit", &limit.to_string()])
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !search_output.status.success() {
            let stderr = String::from_utf8_lossy(&search_output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "confluence search failed: {}",
                stderr
            )));
        }

        let search_json = String::from_utf8_lossy(&search_output.stdout);
        let search_results: SearchResults = serde_json::from_str(&search_json).map_err(|e| {
            SourceError::ParseError(format!("failed to parse search results: {}", e))
        })?;

        let excluded_ids = self.get_excluded_page_ids(&options.exclude_ancestors)?;

        let page_ids: Vec<_> = search_results
            .items
            .iter()
            .filter(|item| {
                item.content
                    .content_type
                    .as_deref()
                    .map(|t| t == "page")
                    .unwrap_or(true)
            })
            .map(|item| item.content.id.clone())
            .filter(|id| !excluded_ids.contains(id))
            .collect();

        if page_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut documents = Vec::new();
        for page_id in page_ids {
            match self.fetch_page(&page_id, &options.tags) {
                Ok(doc) => documents.push(doc),
                Err(e) => {
                    eprintln!("Warning: failed to fetch page {}: {}", page_id, e);
                }
            }
        }

        Ok(documents)
    }

    fn fetch_page(&self, page_id: &str, tags: &[Tag]) -> Result<Document, SourceError> {
        let output = Command::new("atlassian-cli")
            .args(["confluence", "get", page_id])
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "confluence get failed: {}",
                stderr
            )));
        }

        let json = String::from_utf8_lossy(&output.stdout);
        let page: ConfluencePage = serde_json::from_str(&json)
            .map_err(|e| SourceError::ParseError(format!("failed to parse page: {}", e)))?;

        self.page_to_document(page, tags)
    }

    fn get_excluded_page_ids(
        &self,
        exclude_ancestors: &[String],
    ) -> Result<std::collections::HashSet<String>, SourceError> {
        let mut excluded = std::collections::HashSet::new();

        for ancestor_id in exclude_ancestors {
            excluded.insert(ancestor_id.clone());

            let query = format!("ancestor={}", ancestor_id);
            let output = Command::new("atlassian-cli")
                .args(["confluence", "search", &query, "--limit", "1000"])
                .output()
                .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

            if output.status.success() {
                let json = String::from_utf8_lossy(&output.stdout);
                if let Ok(results) = serde_json::from_str::<SearchResults>(&json) {
                    for item in results.items {
                        excluded.insert(item.content.id);
                    }
                }
            }
        }

        Ok(excluded)
    }

    fn page_to_document(
        &self,
        page: ConfluencePage,
        tags: &[Tag],
    ) -> Result<Document, SourceError> {
        let html_content = page
            .body
            .as_ref()
            .and_then(|b| b.storage.as_ref())
            .and_then(|s| s.value.clone())
            .unwrap_or_default();

        let cleaned_content = strip_html_tags(&html_content);
        if cleaned_content.is_empty() {
            return Err(SourceError::ParseError(format!(
                "page {} has no content",
                page.id
            )));
        }

        let full_content = format!("# {}\n\n{}", page.title, cleaned_content);

        let url = page
            .links
            .as_ref()
            .map(|l| {
                let base = l.base.as_deref().unwrap_or("");
                let webui = l.webui.as_deref().unwrap_or("");
                format!("{}{}", base, webui)
            })
            .unwrap_or_else(|| page.id.clone());

        let source = Source::external(SourceType::Confluence, page.id.clone(), url);
        let checksum = calculate_checksum(&full_content);

        let metadata = DocumentMetadata {
            filename: Some(format!("{}.md", sanitize_filename(&page.title))),
            extension: Some("md".to_string()),
            language: Some("markdown".to_string()),
            title: Some(page.title.clone()),
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
            .filter(|id| is_valid_page_id(id))
            .map(String::from);
    }

    if is_valid_page_id(query) {
        return Some(query.to_string());
    }

    None
}

fn is_valid_page_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_digit())
}

fn strip_html_tags(html: &str) -> String {
    let mut result = String::with_capacity(html.len());
    let mut in_tag = false;
    let mut in_script_or_style = false;
    let mut last_was_space = false;

    let lower_html = html.to_lowercase();
    let chars: Vec<char> = html.chars().collect();
    let lower_chars: Vec<char> = lower_html.chars().collect();

    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];

        if c == '<' {
            let remaining: String = lower_chars[i..].iter().collect();
            if remaining.starts_with("<script") || remaining.starts_with("<style") {
                in_script_or_style = true;
            } else if remaining.starts_with("</script") || remaining.starts_with("</style") {
                in_script_or_style = false;
            }
            in_tag = true;
        } else if c == '>' {
            in_tag = false;
            if i > 0 {
                let tag_chars: String = lower_chars[..i]
                    .iter()
                    .rev()
                    .take(10)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                if (tag_chars.contains("/p>")
                    || tag_chars.contains("/div>")
                    || tag_chars.contains("/h")
                    || tag_chars.contains("/li>")
                    || tag_chars.contains("/br>")
                    || tag_chars.contains("br/>"))
                    && !last_was_space
                    && !result.is_empty()
                {
                    result.push('\n');
                    last_was_space = true;
                }
            }
        } else if !in_tag && !in_script_or_style {
            if c.is_whitespace() {
                if !last_was_space && !result.is_empty() {
                    result.push(' ');
                    last_was_space = true;
                }
            } else {
                result.push(c);
                last_was_space = false;
            }
        }

        i += 1;
    }

    result
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&middot;", "·")
        .trim()
        .to_string()
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
    fn test_strip_html_tags() {
        let html = "<p>Hello <strong>world</strong>!</p><script>alert('test');</script>";
        let text = strip_html_tags(html);
        assert!(text.contains("Hello"));
        assert!(text.contains("world"));
        assert!(!text.contains("script"));
        assert!(!text.contains("alert"));
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
}
