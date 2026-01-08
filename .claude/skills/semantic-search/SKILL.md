---
name: semantic-search
version: 1.0.0
description: |
  Search local files, Jira issues, Confluence pages, and Figma designs using semantic similarity.
  Use when user asks to find documents, search code, look up information, query indexed content,
  or needs context from external sources like Jira tickets or Confluence documentation.
allowed-tools: Bash
---

# ssearch CLI

Semantic search CLI for indexed content. Use `--format json` for parsing.

## Quick Reference

```bash
# Search
ssearch search <query> [--limit N] [--tags "key:value"] [--source TYPE] [--format json]

# Index local files
ssearch index add <path> [--tags "key:value"]

# Import custom data (JSON/JSONL)
ssearch import <file> [--tags "key:value"]

# Sync external sources
ssearch source sync jira --project <KEY> --all
ssearch source sync confluence --project <SPACE> --all
ssearch source sync figma --query "<URL>"

# Status
ssearch status
```

## Search Examples

```bash
# Basic search
ssearch search "user authentication"

# Filter by source (built-in: local, jira, confluence, figma)
ssearch search "payment API" --source jira

# Filter by custom source type
ssearch search "meeting notes" --source notion

# Filter by tag
ssearch search "deployment" --tags "project:myapp"

# JSON output for parsing
ssearch search "error handling" --format json | jq '.results[0].location'
```

## Import Custom Data

Import JSON/JSONL documents with optional URL and custom source types:

```bash
# Import from file
ssearch import data.json

# Import from stdin
echo '{"content": "Document text", "title": "My Doc"}' | ssearch import -

# With custom source type
echo '{"content": "...", "source_type": "notion", "title": "Page"}' | ssearch import -
```

### Import Format

```json
{
  "content": "Document content (required)",
  "url": "https://... (optional)",
  "title": "Document title (optional)",
  "path": "logical/path (optional)",
  "source_type": "notion (optional, default: custom)",
  "tags": ["tag1", "tag2"]
}
```

## External Source Sync

| Source | Full Sync | Single Item |
|--------|-----------|-------------|
| Jira | `--project KEY --all` | `--query "PROJ-1234"` |
| Confluence | `--project SPACE --all` | `--query "12345678"` |
| Figma | - | `--query "https://figma.com/..."` |

## Search Options

| Option | Description |
|--------|-------------|
| `-n, --limit` | Result count (default: 10) |
| `-t, --tags` | Filter by tags (`source:jira`, `project:main`) |
| `-s, --source` | Filter by type (any string: `local`, `jira`, `notion`, etc.) |
| `--min-score` | Minimum similarity (0.0-1.0) |
| `--format` | Output format (`text`, `json`, `markdown`) |

## Result Fields

```json
{
  "results": [
    {
      "score": 0.85,
      "location": "/path/file.rs:10-25",
      "source": {
        "source_type": "local",
        "location": "/path/file.rs",
        "url": null
      },
      "tags": ["lang:rust"],
      "content": "matched text..."
    }
  ]
}
```

## Prerequisites

ML daemon auto-starts. Verify with:

```bash
ssearch status
# ML Daemon:     [RUNNING]
# Vector Store:  [CONNECTED]
```

If disconnected:
```bash
docker-compose up -d qdrant
ssearch serve restart
```
