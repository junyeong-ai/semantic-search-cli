//! Text chunking with overlap for optimal embedding.

use crate::models::{Document, DocumentChunk, IndexingConfig};
use crate::utils::has_meaningful_content;

/// Text chunker that splits documents into overlapping chunks.
#[derive(Debug, Clone)]
pub struct TextChunker {
    /// Target chunk size in characters (approximate tokens * 4)
    chunk_size: usize,
    /// Overlap size in characters
    overlap: usize,
}

impl TextChunker {
    /// Create a new text chunker with the given configuration.
    pub fn new(config: &IndexingConfig) -> Self {
        // Convert tokens to approximate characters (1 token ≈ 4 characters)
        let chunk_size = (config.chunk_size as usize) * 4;
        let overlap = (config.chunk_overlap as usize) * 4;
        Self {
            chunk_size,
            overlap,
        }
    }

    /// Create a chunker with default settings.
    pub fn with_defaults() -> Self {
        Self::new(&IndexingConfig::default())
    }

    /// Chunk a document into overlapping segments.
    pub fn chunk(&self, document: &Document) -> Vec<DocumentChunk> {
        let content = &document.content;

        if content.is_empty() {
            return Vec::new();
        }

        // If content is smaller than chunk size, return as single chunk
        if content.len() <= self.chunk_size {
            return vec![DocumentChunk::from_document(
                document,
                content.clone(),
                0,
                1,
                0,
                content.len() as u64,
                Some(1),
                Some(content.lines().count() as u32),
            )];
        }

        let chunks: Vec<_> = self
            .split_with_overlap(content)
            .into_iter()
            .filter(|(chunk_content, _, _, _, _)| has_meaningful_content(chunk_content))
            .collect();

        let total_chunks = chunks.len() as u32;

        chunks
            .into_iter()
            .enumerate()
            .map(
                |(idx, (chunk_content, start_offset, end_offset, line_start, line_end))| {
                    DocumentChunk::from_document(
                        document,
                        chunk_content,
                        idx as u32,
                        total_chunks,
                        start_offset,
                        end_offset,
                        Some(line_start),
                        Some(line_end),
                    )
                },
            )
            .collect()
    }

    /// Split content into overlapping chunks with position information.
    fn split_with_overlap(&self, content: &str) -> Vec<(String, u64, u64, u32, u32)> {
        let mut chunks = Vec::new();
        let chars: Vec<char> = content.chars().collect();
        let total_chars = chars.len();

        if total_chars == 0 {
            return chunks;
        }

        let step = if self.chunk_size > self.overlap {
            self.chunk_size - self.overlap
        } else {
            self.chunk_size
        };

        let mut start = 0;
        let mut line_count = 1u32;
        let mut char_to_line: Vec<u32> = Vec::with_capacity(total_chars);

        // Build character-to-line mapping
        for c in &chars {
            char_to_line.push(line_count);
            if *c == '\n' {
                line_count += 1;
            }
        }

        while start < total_chars {
            let end = (start + self.chunk_size).min(total_chars);

            // Try to find a natural break point (newline, period, space)
            let adjusted_end = self.find_break_point(&chars, start, end, total_chars);

            let chunk_content: String = chars[start..adjusted_end].iter().collect();
            let line_start = char_to_line.get(start).copied().unwrap_or(1);
            let line_end = char_to_line
                .get(adjusted_end.saturating_sub(1))
                .copied()
                .unwrap_or(line_start);

            chunks.push((
                chunk_content,
                start as u64,
                adjusted_end as u64,
                line_start,
                line_end,
            ));

            if adjusted_end >= total_chars {
                break;
            }

            start += step;
            if start >= total_chars {
                break;
            }
        }

        chunks
    }

    /// Find a natural break point near the target end position.
    fn find_break_point(
        &self,
        chars: &[char],
        _start: usize,
        target_end: usize,
        total: usize,
    ) -> usize {
        if target_end >= total {
            return total;
        }

        // Look for a natural break point within the last 20% of the chunk
        let search_start = target_end.saturating_sub(self.chunk_size / 5);
        let search_range = &chars[search_start..target_end];

        // Priority: double newline > single newline > period+space > space
        let mut best_break = None;
        let mut last_newline = None;
        let mut last_sentence = None;
        let mut last_space = None;

        for (i, c) in search_range.iter().enumerate() {
            let pos = search_start + i;
            match c {
                '\n' => {
                    // Check for double newline (paragraph break)
                    if i > 0 && search_range.get(i.saturating_sub(1)) == Some(&'\n') {
                        best_break = Some(pos + 1);
                    }
                    last_newline = Some(pos + 1);
                }
                '.' | '!' | '?' => {
                    // Sentence end followed by space or newline
                    if search_range.get(i + 1).is_some_and(|c| c.is_whitespace()) {
                        last_sentence = Some(pos + 1);
                    }
                }
                ' ' | '\t' => {
                    last_space = Some(pos + 1);
                }
                _ => {}
            }
        }

        best_break
            .or(last_newline)
            .or(last_sentence)
            .or(last_space)
            .unwrap_or(target_end)
    }
}

/// Estimate the number of tokens in a text.
/// Uses a simple heuristic: ~4 characters per token on average.
pub fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DocumentMetadata, Source};

    fn create_test_document(content: &str) -> Document {
        Document::new(
            content.to_string(),
            Source::local("/test.txt"),
            vec![],
            "test_checksum".to_string(),
            DocumentMetadata::default(),
        )
    }

    #[test]
    fn test_small_document_single_chunk() {
        let chunker = TextChunker::with_defaults();
        let doc = create_test_document("Hello, world!");
        let chunks = chunker.chunk(&doc);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].content, "Hello, world!");
        assert_eq!(chunks[0].chunk_index, 0);
        assert_eq!(chunks[0].total_chunks, 1);
    }

    #[test]
    fn test_empty_document() {
        let chunker = TextChunker::with_defaults();
        let doc = create_test_document("");
        let chunks = chunker.chunk(&doc);

        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunking_preserves_overlap() {
        let config = IndexingConfig {
            chunk_size: 50,    // 200 chars
            chunk_overlap: 10, // 40 chars
            ..Default::default()
        };
        let chunker = TextChunker::new(&config);

        let content = "a".repeat(500); // Large enough to create multiple chunks
        let doc = create_test_document(&content);
        let chunks = chunker.chunk(&doc);

        assert!(chunks.len() > 1);
        // Each chunk should have the correct indices
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.chunk_index, i as u32);
            assert_eq!(chunk.total_chunks, chunks.len() as u32);
        }
    }

    #[test]
    fn test_line_tracking() {
        let chunker = TextChunker::with_defaults();
        let content = "Line 1\nLine 2\nLine 3";
        let doc = create_test_document(content);
        let chunks = chunker.chunk(&doc);

        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].line_start, Some(1));
        assert_eq!(chunks[0].line_end, Some(3));
    }

    #[test]
    fn test_estimate_tokens() {
        assert_eq!(estimate_tokens("1234"), 1);
        assert_eq!(estimate_tokens("12345678"), 2);
        assert_eq!(estimate_tokens(""), 0);
    }
}
