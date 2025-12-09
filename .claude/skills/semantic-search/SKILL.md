---
name: semantic-search
version: 1.0.0
description: |
  Execute semantic searches via ssearch CLI. Search indexed local files, Jira issues,
  Confluence pages, Figma designs. Use when user asks to find, search, or query documents.
allowed-tools: Bash
---

# ssearch Command Reference

```bash
# Search indexed documents
ssearch search <query> [--limit N] [--tags "key:value"] [--source type] [--format text|json|markdown]

# Index local files
ssearch index add <path> [--tags "key:value"] [-e "pattern"]

# Sync external sources (full project/space)
ssearch source sync jira --project <KEY> --all
ssearch source sync confluence --project <SPACE> --all

# Sync external sources (query-based)
ssearch source sync <type> --query "<query>" [--limit N] [--tags "key:value"]

# Management
ssearch status                    # Check infrastructure health
ssearch tags list                 # List all indexed tags
ssearch index clear -y            # Clear all indexed data
ssearch source delete <type> -y   # Delete by source type
ssearch tags delete <tag> -y      # Delete by tag
```

## Search Examples

```bash
ssearch search "authentication flow"
ssearch search "payment API" --tags "source:confluence" --limit 5
ssearch search "user login" --source jira,confluence --format json
```

## External Source Types

| Type | Query Format | Examples |
|------|--------------|----------|
| `jira` | `--project KEY`, JQL, issue key, or URL | `--project MYPROJ --all`, `PROJ-1234` |
| `confluence` | `--project SPACE`, CQL, page ID, or URL | `--project DOCS --all`, `12345678` |
| `figma` | File URL with node-id | `https://figma.com/design/xxx?node-id=123-456` |

## Sync Examples

```bash
# Jira - Full project sync (streaming)
ssearch source sync jira --project MYPROJ --all
ssearch source sync jira --project MYPROJ --limit 100

# Jira - Query-based
ssearch source sync jira --query "status=Done" --limit 20
ssearch source sync jira --query "PROJ-1234"

# Confluence - Full space sync (streaming)
ssearch source sync confluence --project DOCS --all
ssearch source sync confluence --project DOCS --limit 100

# Confluence - Query-based
ssearch source sync confluence --query "text~keyword" --limit 50

# Figma (requires node-id for specific frame)
ssearch source sync figma --query "https://figma.com/design/abc123?node-id=123-456"
```

## Tag Format

Tags use `key:value` format. Multiple tags: `--tags "source:jira,project:payments"`

Common auto-generated tags:
- `source:local`, `source:jira`, `source:confluence`, `source:figma`
- `lang:rust`, `lang:python`, `lang:typescript`
- `jira-project:myproj`, `jira-status:in-progress`

## Search Result Fields

| Field | Description |
|-------|-------------|
| `score` | Relevance score (0.0-1.0, higher = better match) |
| `location` | File path with line range or external URL |
| `tags` | Associated metadata tags |
| `content` | Matched text snippet |

## Prerequisites

Before searching, infrastructure must be running:

1. **Check status**: `ssearch status`
2. **Qdrant**: `docker-compose up -d qdrant`
3. **Embedding server**: `cd embedding-server && python server.py`

**Critical**: If `ssearch status` shows disconnected services, search will fail.
