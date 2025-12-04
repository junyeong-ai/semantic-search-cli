# Embedding Server

Local embedding server for semantic-search-cli using sentence-transformers.

## Requirements

- Python 3.14+
- Apple Silicon Mac (MPS acceleration) or CPU

## Quick Start

```bash
# Install dependencies
cd embedding-server
pip install -e .

# Run server (default: BAAI/bge-small-en-v1.5)
python server.py

# Or with a different model
python server.py --model sentence-transformers/all-MiniLM-L6-v2
```

## API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/embed` | POST | Generate embeddings |
| `/info` | GET | Model information |

## Usage with ssearch

```bash
# Start embedding server (default port: 11411)
python server.py

# In another terminal, use ssearch
ssearch index ./my-documents
```

Default configuration (`~/.config/ssearch/config.toml`) already points to `http://localhost:11411`.

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `MODEL_ID` | `BAAI/bge-small-en-v1.5` | Model to use |

### Command Line Options

```
--model, -m     Model ID (default: BAAI/bge-small-en-v1.5)
--host, -H      Host to bind (default: 127.0.0.1)
--port, -p      Port to bind (default: 11411)
--workers, -w   Worker processes (default: 1)
--reload        Enable auto-reload for development
```

## Recommended Models

| Model | Dimension | Max Length | Use Case |
|-------|-----------|------------|----------|
| `Qwen/Qwen3-Embedding-0.6B` | 1024 | 8192 | **Default**, instruction-aware |
| `BAAI/bge-small-en-v1.5` | 384 | 512 | Lightweight alternative |
| `nomic-ai/nomic-embed-text-v1.5` | 768 | 8192 | Code-optimized |

## Instruction-Aware Embeddings

Qwen3-Embedding supports instruction-aware embeddings for 1-5% better retrieval:

- **Documents**: No instruction needed (raw text)
- **Queries**: Automatically prefixed with retrieval instruction

The server handles this automatically via the `instruction_type` parameter.

## Performance

On Apple Silicon (M1/M2/M3):
- MPS acceleration automatically enabled
- ~100-200 embeddings/second for short texts
- Caching for repeated queries

## Development

```bash
# Install dev dependencies
pip install -e ".[dev]"

# Run linter
ruff check .

# Run with auto-reload
python server.py --reload
```
