//! Local file system data source.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

use crate::error::SourceError;
use crate::models::{Document, DocumentMetadata, Source, SourceType, Tag};
use crate::utils::file::{calculate_checksum, is_text_file, read_file_content};

/// Local file system data source.
#[derive(Debug)]
pub struct LocalSource {
    /// Root path to scan
    root: PathBuf,

    /// Patterns to exclude
    exclude_patterns: Vec<String>,

    /// Maximum file size
    max_file_size: u64,
}

impl LocalSource {
    /// Create a new local source.
    pub fn new(root: PathBuf, exclude_patterns: Vec<String>, max_file_size: u64) -> Self {
        Self {
            root,
            exclude_patterns,
            max_file_size,
        }
    }

    /// Collect all indexable files from the source.
    pub fn collect_files(&self) -> Result<Vec<PathBuf>, SourceError> {
        let mut files = Vec::new();

        if self.root.is_file() {
            files.push(self.root.clone());
            return Ok(files);
        }

        for entry in WalkDir::new(&self.root).follow_links(false) {
            let entry = entry.map_err(|e| SourceError::SyncError(e.to_string()))?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            // Check exclusions
            let path_str = path.to_string_lossy();
            let mut excluded = false;

            for pattern in &self.exclude_patterns {
                if glob::Pattern::new(pattern)
                    .map(|p| p.matches(&path_str))
                    .unwrap_or(false)
                {
                    excluded = true;
                    break;
                }
            }

            if !excluded && is_text_file(path) {
                files.push(path.to_path_buf());
            }
        }

        Ok(files)
    }

    /// Read a file and create a Document.
    pub fn read_document(&self, path: &Path, tags: Vec<Tag>) -> Result<Document, SourceError> {
        let content = read_file_content(path, self.max_file_size)
            .map_err(|e| SourceError::SyncError(e.to_string()))?;

        let checksum = calculate_checksum(&content);
        let source = Source::local(path.to_string_lossy().to_string());

        let metadata = DocumentMetadata {
            filename: path.file_name().map(|n| n.to_string_lossy().to_string()),
            extension: path.extension().map(|e| e.to_string_lossy().to_string()),
            language: detect_language(path),
            title: None,
            path: Some(path.to_string_lossy().to_string()),
            size_bytes: content.len() as u64,
        };

        Ok(Document::new(content, source, tags, checksum, metadata))
    }

    /// Get the source type.
    pub fn source_type(&self) -> SourceType {
        SourceType::Local
    }
}

/// Detect programming language from file extension.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language() {
        assert_eq!(
            detect_language(Path::new("test.rs")),
            Some("rust".to_string())
        );
        assert_eq!(
            detect_language(Path::new("test.py")),
            Some("python".to_string())
        );
        assert_eq!(detect_language(Path::new("test.unknown")), None);
    }
}
