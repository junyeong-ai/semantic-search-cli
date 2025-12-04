use serde::{Deserialize, Serialize};

use super::source::Source;
use super::tag::Tag;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub content: String,
    pub source: Source,
    pub tags: Vec<Tag>,
    pub checksum: String,
    pub metadata: DocumentMetadata,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DocumentMetadata {
    pub filename: Option<String>,
    pub extension: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentChunk {
    pub id: String,
    pub document_id: String,
    pub content: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub start_offset: u64,
    pub end_offset: u64,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dense_vector: Vec<f32>,
    pub source: Source,
    pub tags: Vec<Tag>,
    pub checksum: String,
    pub created_at: String,
}

impl Document {
    pub fn generate_id(source: &Source) -> String {
        use sha2::{Digest, Sha256};
        let input = format!("{}:{}", source.source_type, source.location);
        let hash = Sha256::digest(input.as_bytes());
        hex::encode(&hash[..16])
    }

    pub fn new(
        content: String,
        source: Source,
        tags: Vec<Tag>,
        checksum: String,
        metadata: DocumentMetadata,
    ) -> Self {
        let id = Self::generate_id(&source);
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            id,
            content,
            source,
            tags,
            checksum,
            metadata,
            created_at: now.clone(),
            updated_at: now,
        }
    }
}

impl DocumentChunk {
    pub fn generate_id(document_id: &str, chunk_index: u32) -> String {
        use uuid::Uuid;
        let name = format!("{}:{}", document_id, chunk_index);
        Uuid::new_v5(&Uuid::NAMESPACE_OID, name.as_bytes()).to_string()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_document(
        document: &Document,
        content: String,
        chunk_index: u32,
        total_chunks: u32,
        start_offset: u64,
        end_offset: u64,
        line_start: Option<u32>,
        line_end: Option<u32>,
    ) -> Self {
        let id = Self::generate_id(&document.id, chunk_index);
        Self {
            id,
            document_id: document.id.clone(),
            content,
            chunk_index,
            total_chunks,
            start_offset,
            end_offset,
            line_start,
            line_end,
            dense_vector: Vec::new(),
            source: document.source.clone(),
            tags: document.tags.clone(),
            checksum: document.checksum.clone(),
            created_at: document.created_at.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_generate_id() {
        let source = Source::local("/path/to/file.rs");
        let id = Document::generate_id(&source);
        assert_eq!(id.len(), 32);
    }

    #[test]
    fn test_chunk_generate_id() {
        let id = DocumentChunk::generate_id("abc123", 5);
        assert_eq!(id.len(), 36);
        assert!(id.chars().filter(|c| *c == '-').count() == 4);
        let id2 = DocumentChunk::generate_id("abc123", 5);
        assert_eq!(id, id2);
        let id3 = DocumentChunk::generate_id("abc123", 6);
        assert_ne!(id, id3);
    }

    #[test]
    fn test_document_new() {
        let source = Source::local("/test.rs");
        let doc = Document::new(
            "content".to_string(),
            source,
            vec![],
            "checksum".to_string(),
            DocumentMetadata::default(),
        );
        assert!(!doc.id.is_empty());
        assert!(!doc.created_at.is_empty());
    }
}
