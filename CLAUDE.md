# Semantic Search CLI - AI Agent Developer Guide

Rust CLI for semantic search. ONNX embeddings, Qdrant/PostgreSQL backends, async/await.

---

## Architecture

```
src/
├── main.rs              # CLI entry, command dispatch
├── cli/commands/        # Command handlers (search, index, source, import)
├── models/              # Data models (Config, Document, Tag, Search)
├── services/
│   ├── batch.rs         # Batch processing (embed + store)
│   ├── chunker.rs       # Text chunking with line tracking
│   ├── embedding.rs     # ONNX daemon client
│   ├── metrics.rs       # SQLite metrics
│   └── vector_store/    # Qdrant/PostgreSQL backends
├── server/              # ML daemon (ONNX inference via Unix socket)
├── client/              # Daemon IPC client
├── sources/             # External sources (jira, confluence, figma)
└── utils/               # File utils, retry logic
```

---

## Key Patterns

### ML Daemon
```rust
// server/mod.rs - Auto-starts on first request
DaemonServer::new(config)
  → loads ONNX model (~/.cache/semantic-search-cli/models/)
  → listens on Unix socket (/tmp/ssearch.sock)
  → idle timeout: 600s (configurable)
```

### Vector Store
```rust
// services/vector_store/mod.rs - Factory pattern
create_backend(&config) → Box<dyn VectorStore>
// Trait: upsert, search, delete, count, collection_info
```

### Batch Processing
```rust
// services/batch.rs - Used by index, source sync, import
process_batch(embedding_client, vector_store, chunks, texts)
  → embed texts in batches (8 per batch)
  → upsert to vector store
```

### External Sources
```rust
// sources/*.rs - Pattern for all sources
sync()
├── --project KEY --all → full sync (streaming)
├── --project KEY --limit N → batch mode
├── --query "ID" → single item
└── --query "JQL/CQL" → query-based

// Uses atlassian-cli (jira, confluence) and figma-cli
```

---

## Adding Features

### New Data Source
1. `sources/newsource.rs`: Implement `new()`, `source_type()`, `check_available()`, `sync()`
2. `models/source.rs`: Add `SourceType::NewSource`
3. `sources/mod.rs`: Register in `get_data_source()`

### New Search Filter
1. `models/search.rs`: Add field to `SearchQuery`
2. `cli/commands/search.rs`: Add CLI arg
3. `services/vector_store/*.rs`: Implement filter

### New Config Option
1. `models/config.rs`: Add to struct with `#[serde(default)]`

---

## Constants

| Location | Constant | Value |
|----------|----------|-------|
| `services/vector_store/mod.rs` | EMBEDDING_DIM | 1024 |
| `models/config.rs` | DEFAULT_QDRANT_URL | `http://localhost:16334` |
| `models/config.rs` | DEFAULT_COLLECTION | `semantic_search` |
| `models/config.rs` | DEFAULT_EMBEDDING_MODEL | `JunyeongAI/qwen3-embedding-0.6b-onnx` |
| `services/chunker.rs` | chunk_size | 6000 chars |
| `services/chunker.rs` | chunk_overlap | 500 chars |

---

## Common Tasks

### Debug
```bash
RUST_LOG=debug cargo run -- search "query"
```

### Inspect Database
```bash
# Qdrant
curl http://localhost:16334/collections/semantic_search

# Metrics SQLite
sqlite3 ~/.cache/semantic-search-cli/metrics.db ".schema"
```

---

## Test Commands

```bash
cargo test --release        # 47 tests
cargo clippy -- -D warnings # Lint
cargo fmt --check           # Format check
```

---

## Config Paths

- Global: `~/.config/ssearch/config.toml` (XDG_CONFIG_HOME respected)
- Project: `.ssearch/config.toml` (overrides global)
- Environment: `SSEARCH_*` variables (highest priority)
