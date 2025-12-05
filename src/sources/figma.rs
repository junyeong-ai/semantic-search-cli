//! Figma data source via figma-cli integration.

use std::process::Command;

use serde::Deserialize;

use crate::error::SourceError;
use crate::models::{Document, DocumentMetadata, Source, SourceType, Tag};
use crate::sources::SyncOptions;
use crate::utils::file::{calculate_checksum, sanitize_filename};

/// figma-cli extract output format
#[derive(Debug, Deserialize)]
struct ExtractOutput {
    metadata: ExtractMetadata,
    structure: ExtractStructure,
}

#[derive(Debug, Deserialize)]
struct ExtractMetadata {
    #[serde(rename = "fileKey")]
    file_key: String,
    #[serde(rename = "fileName")]
    file_name: String,
}

#[derive(Debug, Deserialize)]
struct ExtractStructure {
    pages: Vec<PageInfo>,
}

#[derive(Debug, Deserialize)]
struct PageInfo {
    id: String,
    name: String,
}

/// figma-cli inspect output format
#[derive(Debug, Deserialize)]
struct InspectOutput {
    file: InspectFile,
    nodes: std::collections::HashMap<String, NodeWrapper>,
}

#[derive(Debug, Deserialize)]
struct InspectFile {
    key: String,
    name: String,
}

#[derive(Debug, Deserialize)]
struct NodeWrapper {
    document: FigmaNode,
}

#[derive(Debug, Deserialize)]
struct FigmaNode {
    #[serde(rename = "type")]
    node_type: String,
    id: String,
    name: String,
    #[serde(default)]
    children: Vec<FigmaNode>,
    #[serde(default)]
    characters: Option<String>,
}

/// Figma data source implementation.
#[derive(Debug)]
pub struct FigmaSource;

impl FigmaSource {
    /// Create a new Figma source.
    pub fn new() -> Self {
        Self
    }

    /// Get the source type.
    pub fn source_type(&self) -> SourceType {
        SourceType::Figma
    }

    /// Human-readable name.
    pub fn name(&self) -> &str {
        "Figma"
    }

    /// Check if figma-cli is available.
    pub fn check_available(&self) -> Result<bool, SourceError> {
        let output = Command::new("which")
            .arg("figma-cli")
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        Ok(output.status.success())
    }

    /// Get installation instructions.
    pub fn install_instructions(&self) -> &str {
        "Install figma-cli: cargo install figma-cli"
    }

    /// Sync designs from Figma using figma-cli.
    /// Creates separate documents for each significant node (page/frame).
    pub fn sync(&self, options: SyncOptions) -> Result<Vec<Document>, SourceError> {
        if !self.check_available()? {
            return Err(SourceError::CliNotFound(
                "figma-cli not found. Install with: cargo install figma-cli".to_string(),
            ));
        }

        let query = options.query.as_ref().ok_or_else(|| {
            SourceError::SyncError("Figma sync requires a --query with file key or URL".to_string())
        })?;

        // Check if URL has node-id → inspect that specific node
        if let Some(node_id) = extract_node_id(query) {
            return self.sync_single_node(query, &node_id, &options.tags);
        }

        // Extract file structure to get pages
        let file_key = extract_file_key(query).unwrap_or_else(|| query.to_owned());
        self.sync_all_pages(&file_key, &options.tags, options.limit)
    }

    /// Sync a single node by its ID.
    fn sync_single_node(
        &self,
        query: &str,
        _node_id: &str,
        tags: &[Tag],
    ) -> Result<Vec<Document>, SourceError> {
        let output = Command::new("figma-cli")
            .args(["inspect", query, "--depth", "10"])
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "figma-cli inspect failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let inspect: InspectOutput = serde_json::from_str(&stdout)
            .map_err(|e| SourceError::ParseError(format!("failed to parse inspect: {}", e)))?;

        let mut documents = Vec::new();
        for (id, wrapper) in &inspect.nodes {
            if let Some(doc) = self.node_to_document(
                &wrapper.document,
                &inspect.file.key,
                &inspect.file.name,
                id,
                tags,
            ) {
                documents.push(doc);
            }
        }

        Ok(documents)
    }

    /// Sync all pages from a Figma file.
    fn sync_all_pages(
        &self,
        file_key: &str,
        tags: &[Tag],
        limit: Option<u32>,
    ) -> Result<Vec<Document>, SourceError> {
        // Step 1: Extract to get page list
        let output = Command::new("figma-cli")
            .args(["extract", file_key, "--format", "json"])
            .output()
            .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SourceError::ExecutionError(format!(
                "figma-cli extract failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json_start = stdout
            .find('{')
            .ok_or_else(|| SourceError::ParseError("no JSON in extract output".to_string()))?;

        let extract: ExtractOutput = serde_json::from_str(&stdout[json_start..])
            .map_err(|e| SourceError::ParseError(format!("failed to parse extract: {}", e)))?;

        let file_name = extract.metadata.file_name.clone();
        let file_key = if extract.metadata.file_key.is_empty() {
            file_key.to_string()
        } else {
            extract.metadata.file_key.clone()
        };

        // Filter out separator pages
        let pages: Vec<_> = extract
            .structure
            .pages
            .iter()
            .filter(|p| !p.name.starts_with('-'))
            .collect();

        let page_limit = limit.unwrap_or(100) as usize;
        let mut documents = Vec::new();

        // Step 2: Inspect each page to get frames
        for page in pages.iter().take(page_limit) {
            let node_id = page.id.replace(':', "-");
            let inspect_output = Command::new("figma-cli")
                .args(["inspect", &file_key, "--nodes", &node_id, "--depth", "5"])
                .output()
                .map_err(|e| SourceError::ExecutionError(e.to_string()))?;

            if !inspect_output.status.success() {
                eprintln!("Warning: failed to inspect page {}", page.name);
                continue;
            }

            let inspect_stdout = String::from_utf8_lossy(&inspect_output.stdout);
            if let Ok(inspect) = serde_json::from_str::<InspectOutput>(&inspect_stdout) {
                for (id, wrapper) in &inspect.nodes {
                    // Create documents for top-level frames in this page
                    self.collect_frame_documents(
                        &wrapper.document,
                        &file_key,
                        &file_name,
                        &page.name,
                        id,
                        tags,
                        &mut documents,
                    );
                }
            }
        }

        // If no frames found, create at least a file-level document
        if documents.is_empty() {
            let content = format!(
                "# {}\n\n## Pages\n{}",
                file_name,
                pages
                    .iter()
                    .map(|p| format!("- {}\n", p.name))
                    .collect::<String>()
            );

            if content.len() >= 30 {
                let url = format!("https://www.figma.com/design/{}", file_key);
                let source = Source::external(SourceType::Figma, file_key.clone(), url);
                let checksum = calculate_checksum(&content);
                let metadata = DocumentMetadata {
                    filename: Some(format!("{}.md", sanitize_filename(&file_name))),
                    extension: Some("md".to_string()),
                    language: Some("markdown".to_string()),
                    title: Some(file_name.clone()),
                    size_bytes: content.len() as u64,
                };

                let mut all_tags = tags.to_vec();
                if let Ok(tag) = "source:figma".parse() {
                    all_tags.push(tag);
                }

                documents.push(Document::new(content, source, all_tags, checksum, metadata));
            }
        }

        Ok(documents)
    }

    /// Recursively collect frame documents from node tree.
    #[allow(clippy::too_many_arguments)]
    fn collect_frame_documents(
        &self,
        node: &FigmaNode,
        file_key: &str,
        file_name: &str,
        page_name: &str,
        node_id: &str,
        tags: &[Tag],
        documents: &mut Vec<Document>,
    ) {
        // Create document for FRAME, COMPONENT, COMPONENT_SET at top level
        match node.node_type.as_str() {
            "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE" => {
                if let Some(doc) =
                    self.frame_to_document(node, file_key, file_name, page_name, node_id, tags)
                {
                    documents.push(doc);
                }
            }
            "CANVAS" => {
                // Page node - recurse into children
                for child in &node.children {
                    let child_id = &child.id;
                    self.collect_frame_documents(
                        child, file_key, file_name, page_name, child_id, tags, documents,
                    );
                }
            }
            _ => {}
        }
    }

    /// Convert a frame node to a Document.
    fn frame_to_document(
        &self,
        node: &FigmaNode,
        file_key: &str,
        file_name: &str,
        page_name: &str,
        node_id: &str,
        tags: &[Tag],
    ) -> Option<Document> {
        let mut content_parts = Vec::new();

        // Title with context
        content_parts.push(format!("# {} / {} / {}\n", file_name, page_name, node.name));
        content_parts.push(format!("\nType: {}\n", node.node_type));

        // Collect text content from tree
        let mut texts = Vec::new();
        Self::collect_texts(node, &mut texts);

        if !texts.is_empty() {
            content_parts.push("\n## Content\n".to_string());
            for text in texts {
                content_parts.push(format!("- {}\n", text));
            }
        }

        // Collect structure info
        let mut frames = Vec::new();
        Self::collect_child_names(node, &mut frames, 0);

        if !frames.is_empty() && frames.len() <= 50 {
            content_parts.push("\n## Structure\n".to_string());
            for (name, depth) in frames {
                let indent = "  ".repeat(depth);
                content_parts.push(format!("{}- {}\n", indent, name));
            }
        }

        let content = content_parts.join("");
        if content.len() < 50 {
            return None;
        }

        let figma_node_id = node_id.replace('-', ":");
        let url = format!(
            "https://www.figma.com/design/{}?node-id={}",
            file_key, figma_node_id
        );
        let source = Source::external(SourceType::Figma, figma_node_id.clone(), url);
        let checksum = calculate_checksum(&content);

        let title = format!("{} - {}", page_name, node.name);
        let metadata = DocumentMetadata {
            filename: Some(format!("{}.md", sanitize_filename(&title))),
            extension: Some("md".to_string()),
            language: Some("markdown".to_string()),
            title: Some(title),
            size_bytes: content.len() as u64,
        };

        let mut all_tags = tags.to_vec();
        if let Ok(tag) = "source:figma".parse() {
            all_tags.push(tag);
        }
        if let Ok(tag) = format!("figma-type:{}", node.node_type.to_lowercase()).parse() {
            all_tags.push(tag);
        }

        Some(Document::new(content, source, all_tags, checksum, metadata))
    }

    /// Recursively collect text content from nodes.
    fn collect_texts(node: &FigmaNode, texts: &mut Vec<String>) {
        if node.node_type == "TEXT" {
            let text = node.characters.as_deref().unwrap_or(&node.name).trim();

            if is_meaningful_text(text) && !texts.contains(&text.to_string()) {
                texts.push(text.to_string());
            }
        }

        for child in &node.children {
            Self::collect_texts(child, texts);
        }
    }

    /// Collect child frame names for structure info.
    fn collect_child_names(node: &FigmaNode, names: &mut Vec<(String, usize)>, depth: usize) {
        if depth > 3 {
            return;
        }

        for child in &node.children {
            if matches!(
                child.node_type.as_str(),
                "FRAME" | "GROUP" | "COMPONENT" | "INSTANCE"
            ) {
                // Only add if name is meaningful
                if is_meaningful_frame_name(&child.name) {
                    names.push((child.name.clone(), depth));
                }
                Self::collect_child_names(child, names, depth + 1);
            }
        }
    }

    /// Convert a single inspected node to Document.
    fn node_to_document(
        &self,
        node: &FigmaNode,
        file_key: &str,
        file_name: &str,
        node_id: &str,
        tags: &[Tag],
    ) -> Option<Document> {
        self.frame_to_document(node, file_key, file_name, "Selected", node_id, tags)
    }
}

impl Default for FigmaSource {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract file key from Figma URL or direct key.
/// Supports:
///   - Direct key: AbcXyz123DefGhi456
///   - URL: https://www.figma.com/design/{key}/{name}?...
///   - URL: https://www.figma.com/file/{key}/{name}
fn extract_file_key(query: &str) -> Option<String> {
    let query = query.trim();

    // Check if it's a Figma URL
    if query.contains("figma.com/") {
        // Try /design/ or /file/ patterns
        for pattern in &["/design/", "/file/"] {
            if let Some(rest) = query.split(pattern).nth(1)
                && let Some(key) = rest.split('/').next()
            {
                let key = key.split('?').next().unwrap_or(key);
                if is_valid_file_key(key) {
                    return Some(key.to_string());
                }
            }
        }
        return None;
    }

    // Check if it's a direct file key (alphanumeric, 20+ chars)
    if is_valid_file_key(query) {
        return Some(query.to_string());
    }

    None
}

fn is_valid_file_key(key: &str) -> bool {
    key.len() >= 10 && key.chars().all(|c| c.is_ascii_alphanumeric())
}

/// Check if frame name is meaningful (not auto-generated).
fn is_meaningful_frame_name(name: &str) -> bool {
    let lower = name.to_lowercase();

    // Filter auto-generated names like "Frame 1234567"
    for prefix in ["frame ", "group ", "instance ", "component "] {
        if let Some(rest) = lower.strip_prefix(prefix)
            && rest.trim().chars().all(|c| c.is_ascii_digit())
        {
            return false;
        }
    }

    // Filter out generic names
    const GENERIC_NAMES: &[&str] = &[
        "frame",
        "group",
        "container",
        "wrapper",
        "row",
        "column",
        "item",
        "cell",
        "box",
        "layer",
        "shape",
        "card",
        "download",
        "title",
        "contents",
        "scrollable area",
        "scroll area",
    ];

    if GENERIC_NAMES.contains(&lower.as_str()) {
        return false;
    }

    true
}

/// Check if text is meaningful (not a placeholder).
fn is_meaningful_text(text: &str) -> bool {
    if text.len() < 3 {
        return false;
    }

    let lower = text.to_lowercase();

    // Filter out common Figma placeholder names
    const PLACEHOLDERS: &[&str] = &[
        "title",
        "label",
        "text",
        "button",
        "icon",
        "image",
        "frame",
        "group",
        "component",
        "instance",
        "rectangle",
        "ellipse",
        "line",
        "vector",
        "container",
        "wrapper",
        "header",
        "footer",
        "content",
        "body",
        "main",
        "section",
        "row",
        "column",
        "item",
        "card",
        "placeholder",
        "untitled",
        "layer",
        "shape",
        "sub title",
        "download",
        "down_small",
        "up_small",
        "left_small",
        "right_small",
    ];

    if PLACEHOLDERS.contains(&lower.as_str()) {
        return false;
    }

    // Filter out auto-generated Figma names like "Frame 1234567"
    if lower.starts_with("frame ") || lower.starts_with("group ") {
        let rest = lower.split_whitespace().nth(1).unwrap_or("");
        if rest.chars().all(|c| c.is_ascii_digit()) {
            return false;
        }
    }

    // Filter out single repeated characters
    if text.chars().all(|c| c == text.chars().next().unwrap()) {
        return false;
    }

    true
}

/// Extract node-id from Figma URL query parameter.
fn extract_node_id(query: &str) -> Option<String> {
    if !query.contains("node-id=") {
        return None;
    }

    query
        .split("node-id=")
        .nth(1)
        .and_then(|rest| rest.split('&').next())
        .filter(|id| !id.is_empty())
        .map(|id| id.replace("%3A", ":").replace("-", ":"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_figma_source_creation() {
        let source = FigmaSource::new();
        assert_eq!(source.source_type(), SourceType::Figma);
        assert_eq!(source.name(), "Figma");
    }

    #[test]
    fn test_extract_file_key() {
        // Direct key
        assert_eq!(
            extract_file_key("AbcXyz123DefGhi456"),
            Some("AbcXyz123DefGhi456".to_string())
        );

        // Design URL
        let url =
            "https://www.figma.com/design/AbcXyz123DefGhi456/Sample-Project?node-id=123-456&m=dev";
        assert_eq!(
            extract_file_key(url),
            Some("AbcXyz123DefGhi456".to_string())
        );

        // File URL (old format)
        let url2 = "https://www.figma.com/file/abc123xyz789/-Name";
        assert_eq!(extract_file_key(url2), Some("abc123xyz789".to_string()));

        // Invalid
        assert_eq!(extract_file_key("short"), None);
        assert_eq!(extract_file_key("project/path"), None);
    }

    #[test]
    fn test_extract_node_id() {
        let url = "https://www.figma.com/design/abc123?node-id=123-456&m=dev";
        assert_eq!(extract_node_id(url), Some("123:456".to_string()));

        let url2 = "https://www.figma.com/design/abc123";
        assert_eq!(extract_node_id(url2), None);
    }

    #[test]
    fn test_collect_texts() {
        let node = FigmaNode {
            node_type: "FRAME".to_string(),
            id: "1:1".to_string(),
            name: "Frame".to_string(),
            children: vec![FigmaNode {
                node_type: "TEXT".to_string(),
                id: "1:2".to_string(),
                name: "Hello World".to_string(),
                children: vec![],
                characters: Some("Hello World".to_string()),
            }],
            characters: None,
        };

        let mut texts = Vec::new();
        FigmaSource::collect_texts(&node, &mut texts);
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0], "Hello World");
    }
}
