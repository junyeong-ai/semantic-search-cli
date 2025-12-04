#!/usr/bin/env python3
"""
Embedding server for semantic-search-cli.

A local embedding server using sentence-transformers.

API Endpoints:
    GET  /health    - Health check
    POST /embed     - Generate embeddings
    GET  /info      - Model information
"""

from __future__ import annotations

import argparse
import logging
import os
import sys
import time
from contextlib import asynccontextmanager
from functools import lru_cache
from typing import TYPE_CHECKING

import torch
import uvicorn
from fastapi import FastAPI, HTTPException
from fastapi.middleware.cors import CORSMiddleware
from pydantic import BaseModel, Field
from sentence_transformers import SentenceTransformer

if TYPE_CHECKING:
    from collections.abc import AsyncIterator

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
)
logger = logging.getLogger(__name__)

# Default model - Qwen3-Embedding with instruction support
DEFAULT_MODEL = "Qwen/Qwen3-Embedding-0.6B"
EMBEDDING_DIMENSION = 1024  # Qwen3-Embedding-0.6B dimension
MAX_LENGTH = 8192  # Max tokens for Qwen3-Embedding

# Query instruction template (1-5% improvement)
# Documents don't need instruction, only queries do
QUERY_INSTRUCTION = "Instruct: Given a search query, retrieve relevant passages that answer the query\nQuery: "


class EmbedRequest(BaseModel):
    """Request body for embedding generation."""

    inputs: list[str] = Field(..., description="List of texts to embed")
    truncate: bool = Field(default=True, description="Whether to truncate long inputs")
    instruction_type: str = Field(
        default="document",
        description="Type of instruction: 'document' for indexing, 'query' for search",
    )


class HealthResponse(BaseModel):
    """Health check response."""

    status: str = "healthy"
    model_id: str | None = None


class InfoResponse(BaseModel):
    """Model information response."""

    model_id: str
    model_type: str = "embedding"
    max_input_length: int
    embedding_dimension: int
    device: str
    instruction_aware: bool = True


class EmbeddingModel:
    """Singleton wrapper for the embedding model."""

    _instance: EmbeddingModel | None = None
    _model: SentenceTransformer | None = None
    _model_id: str = DEFAULT_MODEL
    _device: str = "cpu"

    def __new__(cls) -> EmbeddingModel:
        if cls._instance is None:
            cls._instance = super().__new__(cls)
        return cls._instance

    def load(self, model_id: str | None = None) -> None:
        """Load the embedding model."""
        if model_id:
            self._model_id = model_id

        # Detect best available device
        if torch.backends.mps.is_available():
            self._device = "mps"
            logger.info("Using Apple Silicon MPS acceleration")
        elif torch.cuda.is_available():
            self._device = "cuda"
            logger.info("Using CUDA GPU acceleration")
        else:
            self._device = "cpu"
            logger.info("Using CPU")

        logger.info(f"Loading model: {self._model_id}")
        start_time = time.time()

        # Set environment variable for MPS fallback on unsupported ops
        os.environ["PYTORCH_ENABLE_MPS_FALLBACK"] = "1"

        self._model = SentenceTransformer(
            self._model_id,
            device=self._device,
            trust_remote_code=True,
        )

        load_time = time.time() - start_time
        logger.info(f"Model loaded in {load_time:.2f}s on {self._device}")

    @property
    def model(self) -> SentenceTransformer:
        """Get the loaded model."""
        if self._model is None:
            raise RuntimeError("Model not loaded. Call load() first.")
        return self._model

    @property
    def model_id(self) -> str:
        """Get the model ID."""
        return self._model_id

    @property
    def device(self) -> str:
        """Get the device being used."""
        return self._device

    @property
    def max_seq_length(self) -> int:
        """Get the maximum sequence length."""
        if self._model is None:
            return 512
        return self._model.max_seq_length

    @property
    def embedding_dimension(self) -> int:
        """Get the embedding dimension."""
        if self._model is None:
            return EMBEDDING_DIMENSION
        return self._model.get_sentence_embedding_dimension()


# Global model instance
embedding_model = EmbeddingModel()


@asynccontextmanager
async def lifespan(app: FastAPI) -> AsyncIterator[None]:
    """Application lifespan handler for model loading."""
    model_id = os.environ.get("MODEL_ID", DEFAULT_MODEL)
    embedding_model.load(model_id)
    yield
    logger.info("Shutting down embedding server")


# Create FastAPI app
app = FastAPI(
    title="Embedding Server",
    description="Embedding server for semantic-search-cli",
    version="0.1.0",
    lifespan=lifespan,
)

# Add CORS middleware
app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@app.get("/health", response_model=HealthResponse)
async def health_check() -> HealthResponse:
    """Health check endpoint."""
    return HealthResponse(
        status="healthy",
        model_id=embedding_model.model_id,
    )


@app.get("/info", response_model=InfoResponse)
async def get_info() -> InfoResponse:
    """Get model information."""
    return InfoResponse(
        model_id=embedding_model.model_id,
        model_type="embedding",
        max_input_length=embedding_model.max_seq_length,
        embedding_dimension=embedding_model.embedding_dimension,
        device=embedding_model.device,
    )


def _get_instruction(instruction_type: str) -> str:
    """Get the appropriate instruction prefix based on type.

    Documents don't need instruction prefix, only queries do.
    """
    if instruction_type == "query":
        return QUERY_INSTRUCTION
    return ""  # No instruction for documents


@lru_cache(maxsize=1024)
def _cached_embed(text: str, instruction_type: str) -> tuple[float, ...]:
    """Cache individual embeddings for repeated queries."""
    instruction = _get_instruction(instruction_type)
    embedding = embedding_model.model.encode(
        instruction + text,
        convert_to_numpy=True,
        normalize_embeddings=True,
    )
    return tuple(embedding.tolist())


@app.post("/embed")
async def embed(request: EmbedRequest) -> list[list[float]]:
    """Generate embeddings for the given texts."""
    if not request.inputs:
        return []

    try:
        instruction = _get_instruction(request.instruction_type)

        # Check cache for single inputs
        if len(request.inputs) == 1:
            cached = _cached_embed(request.inputs[0], request.instruction_type)
            return [list(cached)]

        # Prepend instruction to all inputs
        instructed_inputs = [instruction + text for text in request.inputs]

        # Batch encode for multiple inputs
        embeddings = embedding_model.model.encode(
            instructed_inputs,
            convert_to_numpy=True,
            normalize_embeddings=True,
            show_progress_bar=False,
        )

        # Convert to list of lists
        return embeddings.tolist()

    except Exception as e:
        logger.exception("Error generating embeddings")
        raise HTTPException(status_code=500, detail=str(e)) from e


def main() -> None:
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Embedding server for semantic-search-cli")
    parser.add_argument(
        "--model",
        "-m",
        default=os.environ.get("MODEL_ID", DEFAULT_MODEL),
        help=f"Model ID to use (default: {DEFAULT_MODEL})",
    )
    parser.add_argument(
        "--host",
        "-H",
        default="127.0.0.1",
        help="Host to bind to (default: 127.0.0.1)",
    )
    parser.add_argument(
        "--port",
        "-p",
        type=int,
        default=11411,
        help="Port to bind to (default: 11411)",
    )
    parser.add_argument(
        "--workers",
        "-w",
        type=int,
        default=1,
        help="Number of worker processes (default: 1)",
    )
    parser.add_argument(
        "--reload",
        action="store_true",
        help="Enable auto-reload for development",
    )

    args = parser.parse_args()

    # Set model ID in environment for lifespan handler
    os.environ["MODEL_ID"] = args.model

    logger.info(f"Starting embedding server with model: {args.model}")
    logger.info(f"Listening on http://{args.host}:{args.port}")

    uvicorn.run(
        "server:app",
        host=args.host,
        port=args.port,
        workers=args.workers,
        reload=args.reload,
        log_level="info",
    )


if __name__ == "__main__":
    main()
