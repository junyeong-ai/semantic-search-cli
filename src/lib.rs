//! Semantic Search CLI - A tool for semantic search across local files and external data sources.
//!
//! This library provides the core functionality for the `ssearch` command-line tool,
//! including:
//!
//! - Document indexing with text embeddings
//! - Vector search with Qdrant
//! - Tag-based filtering
//! - Multiple output formats
//! - External data source integration

pub mod cli;
pub mod error;
pub mod models;
pub mod services;
pub mod sources;
pub mod utils;

pub use cli::{Cli, Commands};
pub use error::AppError;
pub use models::{Config, OutputFormat};
