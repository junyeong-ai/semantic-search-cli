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
ssearch search <query> [--limit N] [--tags "key:value"] [--format plain|json|markdown]

# Index local files
ssearch index <path> [--tags "key:value"] [--exclude "pattern"]

# Sync external sources
ssearch source sync <type> --query "<query>" [--limit N] [--tags "key:value"]

# Management
ssearch status                    # Check infrastructure health
ssearch tags list                 # List all indexed tags
ssearch clear --confirm           # Clear all indexed data
```

## Search Examples

```bash
ssearch search "authentication flow"
ssearch search "payment API" --tags "source:confluence" --limit 5
ssearch search "user login" --format json
```

## External Source Types

| Type | Query Format | Examples |
|------|--------------|----------|
| `jira` | JQL, issue key, or URL | `project=ABC`, `PROJ-1234`, `https://...atlassian.net/browse/PROJ-1234` |
| `confluence` | CQL, page ID, or URL | `space=DOCS`, `12345678`, `https://...atlassian.net/wiki/.../pages/123` |
| `figma` | File URL with node-id | `https://figma.com/design/xxx?node-id=123-456` |

## Sync Examples

```bash
# Jira
ssearch source sync jira --query "project=MYPROJ ORDER BY updated DESC" --limit 20
ssearch source sync jira --query "PROJ-1234"

# Confluence
ssearch source sync confluence --query "space=DOCS type=page"
ssearch source sync confluence --query "https://example.atlassian.net/wiki/spaces/X/pages/12345"

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
