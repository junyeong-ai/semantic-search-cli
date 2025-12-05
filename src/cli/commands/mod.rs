//! CLI command implementations.

mod config;
mod import;
mod index;
mod search;
mod source;
mod status;
mod tags;

pub use config::ConfigCommand;
pub use import::ImportArgs;
pub use index::IndexCommand;
pub use search::SearchArgs;
pub use source::SourceCommand;
pub use tags::TagsCommand;

// Re-export command handlers
pub use config::handle_config;
pub use import::handle_import;
pub use index::handle_index;
pub use search::handle_search;
pub use source::handle_source;
pub use status::handle_status;
pub use tags::handle_tags;
