use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use serde::Deserialize;

use crate::error::SourceError;
use crate::models::{Document, DocumentMetadata, Source, SourceType, Tag};
use crate::sources::SyncOptions;
use crate::utils::file::calculate_checksum;
use crate::utils::has_meaningful_content;

#[derive(Debug, Deserialize)]
struct JiraIssue {
    key: String,
    fields: JiraFields,
}

#[derive(Debug, Deserialize)]
struct JiraFields {
    summary: Option<String>,
    description: Option<String>,
    issuetype: Option<IssueType>,
    status: Option<Status>,
    project: Option<Project>,
    parent: Option<Parent>,
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
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Parent {
    key: Option<String>,
    fields: Option<ParentFields>,
}

#[derive(Debug, Deserialize)]
struct ParentFields {
    summary: Option<String>,
}

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
        let mut documents = Vec::new();
        self.sync_streaming(options, |doc| {
            documents.push(doc);
            Ok(())
        })?;
        Ok(documents)
    }

    pub fn sync_streaming<F>(
        &self,
        options: SyncOptions,
        mut on_document: F,
    ) -> Result<u64, SourceError>
    where
        F: FnMut(Document) -> Result<(), SourceError>,
    {
        if !self.check_available()? {
            return Err(SourceError::CliNotFound(
                "atlassian-cli not found. Install with: cargo install atlassian-cli".to_string(),
            ));
        }

        if let Some(ref project) = options.project {
            let jql = format!("project={}", project);
            return self.fetch_issues_streaming(&jql, &options, on_document);
        }

        let query = options.query.as_deref().unwrap_or("ORDER BY updated DESC");

        if let Some(issue_key) = extract_issue_key(query) {
            let doc = self.fetch_issue(&issue_key, &options.tags)?;
            on_document(doc)?;
            return Ok(1);
        }

        self.fetch_issues_streaming(query, &options, on_document)
    }

    fn fetch_issues_streaming<F>(
        &self,
        jql: &str,
        options: &SyncOptions,
        mut on_document: F,
    ) -> Result<u64, SourceError>
    where
        F: FnMut(Document) -> Result<(), SourceError>,
    {
        if let Some(limit) = options.limit {
            return self.fetch_issues_batch(jql, options, on_document, limit);
        }

        let args = [
            "jira", "search", jql, "--format", "markdown", "--all", "--stream",
        ];

        eprintln!("Running: atlassian-cli {}", args.join(" "));

        let mut child = Command::new("atlassian-cli")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SourceError::ExecutionError("failed to capture stdout".to_string()))?;

        let reader = BufReader::new(stdout);
        let mut count = 0u64;
        let mut skipped = 0u64;

        for line in reader.lines() {
            let line = match line {
                Ok(l) if !l.trim().is_empty() => l,
                _ => continue,
            };

            let issue: JiraIssue = match serde_json::from_str(&line) {
                Ok(i) => i,
                Err(_) => continue,
            };

            match self.issue_to_document(issue, &options.tags) {
                Ok(doc) => {
                    on_document(doc)?;
                    count += 1;
                    if count.is_multiple_of(50) {
                        eprintln!("  Processed {} issues...", count);
                    }
                }
                Err(_) => skipped += 1,
            }
        }

        let status = child
            .wait()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;
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
            eprintln!("  Skipped {} issues (empty content)", skipped);
        }

        Ok(count)
    }

    fn fetch_issues_batch<F>(
        &self,
        jql: &str,
        options: &SyncOptions,
        mut on_document: F,
        limit: u32,
    ) -> Result<u64, SourceError>
    where
        F: FnMut(Document) -> Result<(), SourceError>,
    {
        let limit_str = limit.to_string();
        let args = [
            "jira", "search", jql, "--format", "markdown", "--limit", &limit_str,
        ];

        eprintln!("Running: atlassian-cli {}", args.join(" "));

        let output = Command::new("atlassian-cli")
            .args(args)
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "jira search failed: {}",
                stderr
            )));
        }

        #[derive(Deserialize)]
        struct SearchResponse {
            items: Vec<JiraIssue>,
        }

        let response: SearchResponse = serde_json::from_slice(&output.stdout)
            .map_err(|e| SourceError::ParseError(format!("failed to parse response: {}", e)))?;

        let mut count = 0u64;
        let mut skipped = 0u64;

        for issue in response.items {
            match self.issue_to_document(issue, &options.tags) {
                Ok(doc) => {
                    on_document(doc)?;
                    count += 1;
                }
                Err(_) => skipped += 1,
            }
        }

        if skipped > 0 {
            eprintln!("  Skipped {} issues (empty content)", skipped);
        }

        Ok(count)
    }

    fn fetch_issue(&self, key: &str, tags: &[Tag]) -> Result<Document, SourceError> {
        let output = Command::new("atlassian-cli")
            .args(["jira", "get", key, "--format", "markdown"])
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "jira get failed: {}",
                stderr
            )));
        }

        let issue: JiraIssue = serde_json::from_slice(&output.stdout)
            .map_err(|e| SourceError::ParseError(format!("failed to parse issue: {}", e)))?;

        self.issue_to_document(issue, tags)
    }

    fn issue_to_document(&self, issue: JiraIssue, tags: &[Tag]) -> Result<Document, SourceError> {
        let key = &issue.key;
        let summary = issue.fields.summary.as_deref().unwrap_or("");
        let description = issue.fields.description.as_deref().unwrap_or("");

        let content = if description.is_empty() {
            format!("# {}\n\n{}", key, summary)
        } else {
            format!("# {}\n\n{}\n\n{}", key, summary, description)
        };

        if !has_meaningful_content(&content) {
            return Err(SourceError::ParseError(format!(
                "issue {} has no meaningful content",
                key
            )));
        }

        let path = build_issue_path(&issue);
        let url = format!("https://42dot.atlassian.net/browse/{}", key);

        let source = Source::with_url(SourceType::Jira, key.clone(), url);
        let checksum = calculate_checksum(&content);

        let metadata = DocumentMetadata {
            filename: Some(format!("{}.md", key)),
            extension: Some("md".to_string()),
            language: Some("markdown".to_string()),
            title: Some(summary.to_string()),
            path: Some(path),
            size_bytes: content.len() as u64,
        };

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

fn extract_issue_key(query: &str) -> Option<String> {
    let query = query.trim();

    if query.contains("atlassian.net/browse/") {
        return query
            .split("/browse/")
            .nth(1)
            .and_then(|rest| rest.split(['/', '?', '#']).next())
            .filter(|key| is_valid_issue_key(key))
            .map(String::from);
    }

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

fn build_issue_path(issue: &JiraIssue) -> String {
    let mut parts = Vec::new();

    if let Some(ref project) = issue.fields.project {
        if let Some(ref name) = project.name {
            parts.push(name.as_str());
        } else if let Some(ref key) = project.key {
            parts.push(key.as_str());
        }
    }

    if let Some(ref parent) = issue.fields.parent {
        if let Some(ref fields) = parent.fields {
            if let Some(ref summary) = fields.summary {
                parts.push(summary.as_str());
            }
        } else if let Some(ref key) = parent.key {
            parts.push(key.as_str());
        }
    }

    let summary = issue.fields.summary.as_deref().unwrap_or(&issue.key);
    parts.push(summary);

    parts.join(" > ")
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
    fn test_extract_issue_key() {
        assert_eq!(
            extract_issue_key("PROJ-1234"),
            Some("PROJ-1234".to_string())
        );
        assert_eq!(extract_issue_key("PROJ-123"), Some("PROJ-123".to_string()));

        let url = "https://example.atlassian.net/browse/PROJ-1234";
        assert_eq!(extract_issue_key(url), Some("PROJ-1234".to_string()));

        let url2 = "https://example.atlassian.net/browse/PROJ-123?param=1";
        assert_eq!(extract_issue_key(url2), Some("PROJ-123".to_string()));

        assert_eq!(extract_issue_key("key=PROJ-123"), None);
        assert_eq!(extract_issue_key("ORDER BY updated"), None);
        assert_eq!(extract_issue_key("project=PROJ"), None);
    }

    #[test]
    fn test_build_issue_path() {
        let issue = JiraIssue {
            key: "AKIT-123".to_string(),
            fields: JiraFields {
                summary: Some("Test Issue".to_string()),
                description: None,
                issuetype: None,
                status: None,
                project: Some(Project {
                    key: Some("AKIT".to_string()),
                    name: Some("AKit".to_string()),
                }),
                parent: Some(Parent {
                    key: Some("AKIT-100".to_string()),
                    fields: Some(ParentFields {
                        summary: Some("Parent Epic".to_string()),
                    }),
                }),
            },
        };

        assert_eq!(build_issue_path(&issue), "AKit > Parent Epic > Test Issue");
    }
}
