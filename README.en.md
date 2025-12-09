# Semantic Search CLI

[![Rust](https://img.shields.io/badge/rust-1.91.1%2B%20(2024%20edition)-orange?style=flat-square&logo=rust)](https://www.rust-lang.org)
[![CI](https://img.shields.io/github/actions/workflow/status/junyeong-ai/semantic-search-cli/ci.yml?branch=main&style=flat-square&logo=github&label=CI)](https://github.com/junyeong-ai/semantic-search-cli/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/junyeong-ai/semantic-search-cli?style=flat-square&logo=github)](https://github.com/junyeong-ai/semantic-search-cli/releases/latest)

> **🌐 [한국어](README.md)** | **English**

---

> **🔍 AI-Powered Semantic Search CLI**
>
> - 🧠 **Semantic search** (Qwen3 embeddings 1024-dim + Qdrant vector DB)
> - 📁 **Local file indexing** (code, docs, config files)
> - 🔗 **External source integration** (Jira, Confluence, Figma)
> - 🏷️ **Tag filtering** (by source, language, project)

---

## ⚡ Quick Start (5 minutes)

```bash
# 1. Install
git clone https://github.com/junyeong-ai/semantic-search-cli
cd semantic-search-cli
./scripts/install.sh

# 2. Start infrastructure
docker-compose up -d qdrant
cd embedding-server && python server.py &

# 3. Check status
ssearch status

# 4. Index files
ssearch index add ./src

# 5. Search! 🎉
ssearch search "user authentication logic"
```

---

## 🎯 Key Features

### Semantic Search
```bash
# Basic search
ssearch search "API endpoint design"

# Tag filtering
ssearch search "payment processing" --tags "source:jira"

# Source type filtering
ssearch search "error handling" --source jira,confluence

# JSON output
ssearch search "error handling" --format json --limit 5

# Minimum score filtering
ssearch search "auth logic" --min-score 0.7
```

### File Indexing
```bash
# Index directory
ssearch index add ./src --tags "project:myapp"

# Exclude patterns
ssearch index add . -e "node_modules" -e "target" -e ".git"
```

### External Source Sync
```bash
# Jira full project sync (streaming)
ssearch source sync jira --project MYPROJ --all
ssearch source sync jira --project MYPROJ --limit 100

# Jira issues (JQL or issue key)
ssearch source sync jira --query "status=Done" --limit 50
ssearch source sync jira --query "PROJ-1234"

# Confluence full space sync (streaming)
ssearch source sync confluence --project DOCS --all
ssearch source sync confluence --project DOCS --limit 100

# Confluence pages (CQL or page ID/URL)
ssearch source sync confluence --query "text~keyword" --limit 50

# Figma designs (URL)
ssearch source sync figma --query "https://figma.com/design/xxx?node-id=123"
```

### Management
```bash
ssearch status            # Check infrastructure status
ssearch tags list         # List tags
ssearch index clear -y    # Delete all data
```

---

## 📦 Installation

### Prerequisites

| Component | Purpose |
|-----------|---------|
| **Docker** | Qdrant vector DB |
| **Python 3.10+** | Embedding server |

### Method 1: Download Release (Recommended)

```bash
# macOS (Apple Silicon)
curl -L https://github.com/junyeong-ai/semantic-search-cli/releases/latest/download/ssearch-$(curl -s https://api.github.com/repos/user/semantic-search-cli/releases/latest | grep tag_name | cut -d '"' -f 4)-aarch64-apple-darwin.tar.gz | tar xz
sudo mv ssearch /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/junyeong-ai/semantic-search-cli/releases/latest/download/ssearch-$(curl -s https://api.github.com/repos/user/semantic-search-cli/releases/latest | grep tag_name | cut -d '"' -f 4)-x86_64-apple-darwin.tar.gz | tar xz
sudo mv ssearch /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/junyeong-ai/semantic-search-cli/releases/latest/download/ssearch-$(curl -s https://api.github.com/repos/user/semantic-search-cli/releases/latest | grep tag_name | cut -d '"' -f 4)-x86_64-unknown-linux-gnu.tar.gz | tar xz
sudo mv ssearch /usr/local/bin/
```

### Method 2: Build from Source

```bash
git clone https://github.com/junyeong-ai/semantic-search-cli
cd semantic-search-cli
./scripts/install.sh
```

> **Requires**: Rust 1.91.1+

### Method 3: Manual Build

```bash
cargo build --release
cp target/release/ssearch ~/.local/bin/
```

### 🤖 Claude Code Skill

Installation prompts for Claude Code skill:
- **User-level**: Available in all projects
- **Project-level**: Auto-distributed to team via git

---

## ⚙️ Configuration

### Start Infrastructure

```bash
# Qdrant (vector DB)
docker-compose up -d qdrant

# Embedding server (Qwen3)
cd embedding-server && python server.py
```

### Config File

**Location**: `~/.config/semantic-search-cli/config.toml`

```toml
[embedding]
url = "http://localhost:11411"
timeout_secs = 120
batch_size = 8

[vector_store]
url = "http://localhost:16334"
collection = "semantic_search"
# api_key = "optional-api-key"

[indexing]
max_file_size = 10485760  # 10MB
chunk_size = 6000
chunk_overlap = 500
exclude_patterns = [
  "**/node_modules/**",
  "**/target/**",
  "**/.git/**",
]

[search]
default_limit = 10
default_format = "text"  # text, json, markdown
# default_min_score = 0.5
```

---

## 📚 Command Reference

| Command | Description |
|---------|-------------|
| `search <query>` | Semantic search |
| `index add <path>` | Index files |
| `index delete <path>` | Delete from index |
| `index clear` | Clear all index |
| `source sync <type>` | Sync external source |
| `source list` | List sources |
| `source delete <type>` | Delete by source |
| `status` | Check infrastructure status |
| `tags list` | List tags |
| `tags delete <tag>` | Delete by tag |
| `import <file>` | Import JSON/JSONL |
| `config init` | Initialize config |
| `config show` | Show current config |
| `config edit` | Edit config file |

### Search Options

| Option | Description |
|--------|-------------|
| `-n, --limit N` | Limit results (default: 10) |
| `-t, --tags "k:v"` | Filter by tags |
| `-s, --source type` | Filter by source type (local, jira, confluence, figma) |
| `--min-score` | Minimum similarity score |
| `--format` | Output format (text/json/markdown) |

---

## 🔧 Troubleshooting

### Connection Error
```bash
ssearch status  # Check infrastructure status
docker ps       # Verify Qdrant is running
```

### No Search Results
- Check indexing status: `ssearch status`
- Verify infrastructure: Qdrant + embedding server

### Indexing Failure
- Check embedding server: `curl localhost:11411/health`

---

## 💬 Support

- **GitHub Issues**: Report issues
- **Developer Docs**: [CLAUDE.md](CLAUDE.md)

---

<div align="center">

**🌐 [한국어](README.md)** | **English**

**Version 0.1.0** • Rust 2024 Edition

</div>
