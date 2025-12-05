//! Jira data source via atlassian-cli integration.

use std::process::Command;

use serde::Deserialize;
use serde_json::Value;

use crate::error::SourceError;
use crate::models::{Document, DocumentMetadata, Source, SourceType, Tag};
use crate::sources::SyncOptions;
use crate::utils::file::calculate_checksum;

/// Search result item.
#[derive(Debug, Deserialize)]
struct SearchResultItem {
    key: String,
}

/// Search results wrapper.
#[derive(Debug, Deserialize)]
struct SearchResults {
    items: Vec<SearchResultItem>,
}

/// Full issue from jira get command.
#[derive(Debug, Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraFields,
    #[serde(rename = "self")]
    self_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JiraFields {
    summary: Option<String>,
    description: Option<Value>,
    issuetype: Option<IssueType>,
    status: Option<Status>,
    project: Option<Project>,
}

#[derive(Debug, Deserialize)]
struct IssueType {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Status {
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Project {
    key: Option<String>,
}

/// Jira data source implementation.
#[derive(Debug)]
pub struct JiraSource;

impl JiraSource {
    pub fn new() -> Self {
        Self
    }

    pub fn source_type(&self) -> SourceType {
        SourceType::Jira
    }

    pub fn name(&self) -> &str {
        "Jira"
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

        let query = options.query.as_deref().unwrap_or("ORDER BY updated DESC");

        // Check if query is a Jira URL or direct issue key → fetch directly
        if let Some(issue_key) = extract_issue_key(query) {
            return match self.fetch_issue(&issue_key, &options.tags) {
                Ok(doc) => Ok(vec![doc]),
                Err(e) => Err(e),
            };
        }

        // JQL query → search then fetch each issue
        let limit = options.limit.unwrap_or(10);

        let search_output = Command::new("atlassian-cli")
            .args(["jira", "search", query, "--limit", &limit.to_string()])
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !search_output.status.success() {
            let stderr = String::from_utf8_lossy(&search_output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "jira search failed: {}",
                stderr
            )));
        }

        let search_json = String::from_utf8_lossy(&search_output.stdout);
        let search_results: SearchResults = serde_json::from_str(&search_json).map_err(|e| {
            SourceError::ParseError(format!("failed to parse search results: {}", e))
        })?;

        let issue_keys: Vec<_> = search_results.items.iter().map(|i| i.key.clone()).collect();

        if issue_keys.is_empty() {
            return Ok(Vec::new());
        }

        // Step 2: Fetch each issue's full content
        let mut documents = Vec::new();
        for key in issue_keys {
            match self.fetch_issue(&key, &options.tags) {
                Ok(doc) => documents.push(doc),
                Err(e) => eprintln!("Warning: failed to fetch issue {}: {}", key, e),
            }
        }

        Ok(documents)
    }

    fn fetch_issue(&self, key: &str, tags: &[Tag]) -> Result<Document, SourceError> {
        let output = Command::new("atlassian-cli")
            .args(["jira", "get", key])
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "jira get failed: {}",
                stderr
            )));
        }

        let json = String::from_utf8_lossy(&output.stdout);
        let issue: JiraIssue = serde_json::from_str(&json)
            .map_err(|e| SourceError::ParseError(format!("failed to parse issue: {}", e)))?;

        self.issue_to_document(issue, tags)
    }

    fn issue_to_document(&self, issue: JiraIssue, tags: &[Tag]) -> Result<Document, SourceError> {
        let key = &issue.key;
        let summary = issue.fields.summary.as_deref().unwrap_or("");

        // Extract text from ADF description
        let description = issue
            .fields
            .description
            .as_ref()
            .map(extract_text_from_adf)
            .unwrap_or_default();

        // Build content
        let mut content_parts = Vec::new();
        if !summary.is_empty() {
            content_parts.push(format!("# {}\n", summary));
        }
        if !description.is_empty() {
            content_parts.push(format!("\n{}\n", description));
        }

        let content = content_parts.join("");
        if content.trim().is_empty() {
            return Err(SourceError::ParseError(format!(
                "issue {} has no content",
                key
            )));
        }

        // Build URL
        let url = issue.self_url.as_ref().map_or_else(
            || key.clone(),
            |u| {
                u.split("/rest/api/")
                    .next()
                    .map_or_else(|| key.clone(), |base| format!("{base}/browse/{key}"))
            },
        );

        let source = Source::external(SourceType::Jira, key.clone(), url);
        let checksum = calculate_checksum(&content);

        let metadata = DocumentMetadata {
            filename: Some(format!("{}.md", key)),
            extension: Some("md".to_string()),
            language: Some("markdown".to_string()),
            title: Some(summary.to_string()),
            size_bytes: content.len() as u64,
        };

        // Build tags
        let mut all_tags = tags.to_vec();
        if let Ok(tag) = "source:jira".parse() {
            all_tags.push(tag);
        }
        if let Some(ref project) = issue.fields.project
            && let Some(ref project_key) = project.key
            && let Ok(tag) = format!("jira-project:{}", project_key.to_lowercase()).parse()
        {
            all_tags.push(tag);
        }
        if let Some(ref issue_type) = issue.fields.issuetype
            && let Some(ref name) = issue_type.name
            && let Ok(tag) = format!("jira-type:{}", name.to_lowercase().replace(' ', "-")).parse()
        {
            all_tags.push(tag);
        }
        if let Some(ref status) = issue.fields.status
            && let Some(ref name) = status.name
            && let Ok(tag) =
                format!("jira-status:{}", name.to_lowercase().replace(' ', "-")).parse()
        {
            all_tags.push(tag);
        }

        Ok(Document::new(content, source, all_tags, checksum, metadata))
    }
}

impl Default for JiraSource {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract issue key from Jira URL or direct key.
/// Supports:
///   - Direct key: PROJECT-123, PROJ-1234
///   - URL: https://domain.atlassian.net/browse/PROJECT-123
fn extract_issue_key(query: &str) -> Option<String> {
    let query = query.trim();

    // Check if it's a URL
    if query.contains("atlassian.net/browse/") {
        return query
            .split("/browse/")
            .nth(1)
            .and_then(|rest| rest.split(['/', '?', '#']).next())
            .filter(|key| is_valid_issue_key(key))
            .map(String::from);
    }

    // Check if it's a direct issue key (e.g., PROJ-1234)
    if is_valid_issue_key(query) {
        return Some(query.to_string());
    }

    None
}

fn is_valid_issue_key(key: &str) -> bool {
    key.contains('-')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && key.split('-').next().is_some_and(|p| !p.is_empty())
        && key
            .split('-')
            .nth(1)
            .is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()))
}

/// Extract plain text from Atlassian Document Format (ADF).
fn extract_text_from_adf(value: &Value) -> String {
    let mut result = String::new();
    extract_text_recursive(value, &mut result);
    result.trim().to_string()
}

fn extract_text_recursive(value: &Value, result: &mut String) {
    match value {
        Value::Object(obj) => {
            if let Some(Value::String(text)) = obj.get("text") {
                result.push_str(text);
            }
            if let Some(content) = obj.get("content") {
                extract_text_recursive(content, result);
            }
            // Handle paragraph/listItem boundaries
            if let Some(Value::String(node_type)) = obj.get("type")
                && matches!(node_type.as_str(), "paragraph" | "listItem" | "heading")
            {
                result.push('\n');
            }
        }
        Value::Array(arr) => {
            for item in arr {
                extract_text_recursive(item, result);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jira_source_creation() {
        let source = JiraSource::new();
        assert_eq!(source.source_type(), SourceType::Jira);
        assert_eq!(source.name(), "Jira");
    }

    #[test]
    fn test_extract_text_from_adf() {
        let adf = serde_json::json!({
            "type": "doc",
            "content": [
                {
                    "type": "paragraph",
                    "content": [
                        {"type": "text", "text": "Hello "},
                        {"type": "text", "text": "World"}
                    ]
                }
            ]
        });
        let text = extract_text_from_adf(&adf);
        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn test_extract_issue_key() {
        // Direct key
        assert_eq!(
            extract_issue_key("PROJ-1234"),
            Some("PROJ-1234".to_string())
        );
        assert_eq!(extract_issue_key("PROJ-123"), Some("PROJ-123".to_string()));

        // URL
        let url = "https://example.atlassian.net/browse/PROJ-1234";
        assert_eq!(extract_issue_key(url), Some("PROJ-1234".to_string()));

        let url2 = "https://example.atlassian.net/browse/PROJ-123?param=1";
        assert_eq!(extract_issue_key(url2), Some("PROJ-123".to_string()));

        // JQL queries (should not match)
        assert_eq!(extract_issue_key("key=PROJ-123"), None);
        assert_eq!(extract_issue_key("ORDER BY updated"), None);
        assert_eq!(extract_issue_key("project=PROJ"), None);
    }
}
