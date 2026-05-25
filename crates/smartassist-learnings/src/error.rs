//! Error types for learnings operations.

use thiserror::Error;

/// Errors that can occur in learnings operations.
#[derive(Debug, Error)]
pub enum LearningsError {
    /// IO error reading or writing learning files.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Parsing error when reading learning entries.
    #[error("Parse error: {0}")]
    Parse(String),

    /// Entry not found.
    #[error("Entry not found: {0}")]
    NotFound(String),

    /// Invalid category name.
    #[error("Invalid category: {0}")]
    InvalidCategory(String),

    /// Document parsing error.
    #[error("Document parsing error: {0}")]
    DocumentParse(String),

    /// LLM summarization error.
    #[error("LLM summarization error: {0}")]
    LlmSummarization(String),
}