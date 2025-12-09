# Semantic Search CLI - AI Agent Developer Guide

Essential knowledge for implementing features and debugging this Rust CLI tool.

---

## Architecture Overview

```
src/
├── cli/commands/     # Command handlers (index, search, source, import)
├── models/           # Document, Config, Source, Tag, Search
├── services/
│   ├── batch.rs      # Shared batch processing (embed + store)
│   ├── chunker.rs    # Text chunking with line tracking
│   ├── embedding.rs  # Qwen3 embedding client (1024 dim)
│   └── vector_store.rs    # Qdrant client
├── sources/          # Data sources (local, jira, confluence, figma)
└── utils/            # File utilities, retry logic
```

---

## Core Patterns

### Dense Vector Search

**Implementation** (`services/vector_store.rs`):
- Qwen3 embeddings (1024 dimensions)
- Cosine similarity scoring
- Tag and source type filtering

```rust
pub async fn search(
    &self,
    query_vector: Vec<f32>,
    limit: u64,
    tags: &[Tag],
    source_types: &[SourceType],
    min_score: Option<f32>,
) -> Result<Vec<SearchResult>, VectorStoreError>
```

---

### Batch Processing

**Shared function** (`services/batch.rs`):
```rust
pub async fn process_batch(
    embedding_client: &EmbeddingClient,
    vector_client: &VectorStoreClient,
    chunks: &mut Vec<DocumentChunk>,
    texts: &mut Vec<String>,
) -> Result<()>
```

**Used by**: `index`, `source sync`, `import` commands.

---

### Document Chunking

**Implementation** (`services/chunker.rs`):
- Character-based chunking (6000 chars default, 500 overlap)
- Line number tracking for source location
- Preserves context across chunk boundaries

---

### External Source Sync

**Pattern** (`sources/*.rs`):
```
sync()
├── --project → full project/space sync
├── single ID → fetch_issue/fetch_page
└── query → fetch_issues/fetch_pages
    ├── limit → batch mode
    └── no limit → streaming mode (--all --stream)
```

**CLI Options**:
```bash
ssearch source sync jira --project AKIT --all        # Full project (streaming)
ssearch source sync jira --project AKIT --limit 100  # Batch mode
ssearch source sync jira --query "AKIT-123"          # Single issue
ssearch source sync confluence --project DOCS --all  # Full space (streaming)
```

**URL/ID Parsing**:
```rust
// Jira: PROJ-1234, https://...atlassian.net/browse/PROJ-1234
fn extract_issue_key(query: &str) -> Option<String>

// Confluence: 12345, https://...atlassian.net/wiki/.../pages/12345
fn extract_page_id(query: &str) -> Option<String>

// Figma: https://figma.com/design/xxx?node-id=123
fn extract_file_key(query: &str) -> Option<String>
```

---

## Development Tasks

### Add New Data Source

1. Create `sources/newsource.rs`
2. Implement struct:
   ```rust
   pub fn new() -> Self
   pub fn source_type(&self) -> SourceType
   pub fn check_available(&self) -> Result<bool, SourceError>
   pub fn sync(&self, options: SyncOptions) -> Result<Vec<Document>, SourceError>
   ```
3. Add variant to `models/source.rs`: `SourceType::NewSource`
4. Register in `sources/mod.rs`: `get_data_source()`

### Add Search Filter

1. Update `SearchQuery` in `models/search.rs`
2. Add CLI arg in `cli/commands/search.rs`
3. Implement filter in `vector_store.rs`

### Add Document Field

1. Update `DocumentMetadata` in `models/document.rs`
2. Add to Qdrant payload in `vector_store.rs`

---

## Key Constants

| Location | Constant | Value |
|----------|----------|-------|
| `services/vector_store.rs` | `EMBEDDING_DIM` | 1024 |
| `models/config.rs` | `DEFAULT_EMBEDDING_URL` | `http://localhost:11411` |
| `models/config.rs` | `DEFAULT_QDRANT_URL` | `http://localhost:16334` |
| `models/config.rs` | `DEFAULT_COLLECTION` | `semantic_search` |

---

## Config Defaults

| Section | Key | Default |
|---------|-----|---------|
| embedding | url | `http://localhost:11411` |
| embedding | timeout_secs | 120 |
| embedding | batch_size | 8 |
| vector_store | url | `http://localhost:16334` |
| vector_store | collection | `semantic_search` |
| indexing | chunk_size | 6000 |
| indexing | chunk_overlap | 500 |
| indexing | max_file_size | 10MB |
| search | default_limit | 10 |
| search | default_format | text |

---

## Commands

```bash
cargo test --release           # All tests
cargo clippy -- -D warnings    # Lint
cargo fmt                      # Format
```

---

## Common Issues

| Issue | Check | Fix |
|-------|-------|-----|
| Embedding server down | `curl localhost:11411/health` | Restart Python server |
| Qdrant not running | `docker ps` | `docker-compose up -d qdrant` |
| No search results | `ssearch status` | Index documents first |

---

This guide contains implementation-critical knowledge only. For user documentation, see [README.md](README.md).
