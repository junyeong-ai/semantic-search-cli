# Semantic Search CLI

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![Release](https://img.shields.io/github/v/release/junyeong-ai/semantic-search-cli?style=flat-square&logo=github)](https://github.com/junyeong-ai/semantic-search-cli/releases/latest)

> **English** | **[한국어](README.md)**

**Semantic search from the terminal.** Search local code, Jira, Confluence, and Figma with a single command.

---

## Why Semantic Search CLI?

- **Semantic Search** — Search by meaning, not keywords (Qwen3 1024-dim embeddings)
- **Unified Search** — Local files + Jira + Confluence + Figma
- **Automation** — ML daemon auto-starts, Claude Code integration

---

## Quick Start

```bash
# Install
git clone https://github.com/junyeong-ai/semantic-search-cli && cd semantic-search-cli
./scripts/install.sh

# Start Qdrant
docker-compose up -d qdrant

# Index & Search
ssearch index add ./src
ssearch search "user authentication logic"
```

---

## Features

### Search
```bash
ssearch search "API design"                    # Basic search
ssearch search "payment" --source jira         # Jira only
ssearch search "error" --tags "project:main"   # Tag filter
ssearch search "auth" --min-score 0.7          # Similarity filter
ssearch search "design" --format json          # JSON output
```

### Indexing
```bash
ssearch index add ./src                        # Directory
ssearch index add . --tags "project:myapp"     # With tags
ssearch index add . -e "node_modules" -e ".git" # Exclude patterns
ssearch index delete ./old                     # Delete
ssearch index clear -y                         # Clear all
```

### External Source Sync
```bash
# Jira
ssearch source sync jira --project MYPROJ --all        # Full project (streaming)
ssearch source sync jira --project MYPROJ --limit 100  # Batch mode
ssearch source sync jira --query "PROJ-1234"           # Single issue

# Confluence
ssearch source sync confluence --project DOCS --all    # Full space
ssearch source sync confluence --query "12345678"      # Single page

# Figma
ssearch source sync figma --query "https://figma.com/design/xxx?node-id=123"
```

### Management
```bash
ssearch status              # Infrastructure status
ssearch tags list           # Tag list
ssearch source list         # Source list
ssearch serve restart       # Restart ML daemon
```

---

## Installation

### Auto Install (Recommended)
```bash
git clone https://github.com/junyeong-ai/semantic-search-cli && cd semantic-search-cli
./scripts/install.sh
```

### Manual Build
```bash
cargo build --release
cp target/release/ssearch ~/.local/bin/
```

**Requirements**: Docker (for Qdrant)

---

## Configuration

### Config Files (Priority Order)
1. Environment variables (`SSEARCH_*`)
2. Project config (`.ssearch/config.toml`)
3. Global config (`~/.config/ssearch/config.toml`)

Global config example:

```toml
[embedding]
model_id = "JunyeongAI/qwen3-embedding-0.6b-onnx"
dimension = 1024
batch_size = 8

[vector_store]
driver = "qdrant"           # qdrant | postgresql
url = "http://localhost:16334"
collection = "semantic_search"

[indexing]
chunk_size = 6000
chunk_overlap = 500
max_file_size = 10485760    # 10MB

[search]
default_limit = 10
default_format = "text"     # text | json | markdown

[daemon]
idle_timeout_secs = 600     # Auto-stop after 10 min
auto_start = true

[metrics]
enabled = true
retention_days = 30
```

---

## Command Reference

| Command | Description |
|---------|-------------|
| `search <query>` | Semantic search |
| `index add <path>` | Index files |
| `index delete <path>` | Delete |
| `index clear` | Clear all |
| `source sync <type>` | Sync external source |
| `source list` | Source list |
| `source delete <type>` | Delete by source |
| `tags list` | Tag list |
| `tags delete <tag>` | Delete by tag |
| `import <file>` | Import JSON/JSONL |
| `status` | Check status |
| `serve restart` | Restart daemon |
| `config init/show/edit` | Config management |

### Search Options

| Option | Description |
|--------|-------------|
| `-n, --limit` | Result limit (default: 10) |
| `-t, --tags` | Tag filter (`key:value`) |
| `-s, --source` | Source filter (`local,jira,confluence,figma`) |
| `--min-score` | Minimum similarity (0.0-1.0) |
| `-f, --format` | Output format (`text,json,markdown`) |

---

## Troubleshooting

### Check Status
```bash
ssearch status
docker ps  # Check Qdrant
```

### Restart Daemon
```bash
ssearch serve restart
```

### Debug
```bash
RUST_LOG=debug ssearch search "query"
```

---

## Support

- [GitHub Issues](https://github.com/junyeong-ai/semantic-search-cli/issues)
- [Developer Guide](CLAUDE.md)

---

<div align="center">

**English** | **[한국어](README.md)**

Made with Rust

</div>
