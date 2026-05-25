//! Markdown-based learnings store for SmartAssist.
//!
//! This crate provides a human-readable, file-based knowledge store that
//! complements the vector-based RAG system. Learnings are stored as markdown
//! files in `~/.smartassist/md/` with structured metadata in HTML comments.

pub mod context;
pub mod error;
pub mod extractor;
pub mod parser;
pub mod store;

pub use context::LearningContextProvider;
pub use error::LearningsError;
pub use extractor::LearningExtractor;
pub use parser::{CompositeParser, DocumentFormat, DocumentParser, MarkdownParser, ParsedDocument, TextParser};
pub use store::{LearningEntry, LearningSource, LearningStore};

/// Result type for learnings operations.
pub type Result<T> = std::result::Result<T, LearningsError>;