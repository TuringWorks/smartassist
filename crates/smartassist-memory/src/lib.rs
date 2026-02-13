//! Vector memory and embeddings for SmartAssist.
//!
//! This crate provides:
//! - Embedding generation via OpenAI/Google APIs
//! - Vector storage and retrieval
//! - Semantic search capabilities

pub mod error;
pub mod embeddings;
pub mod store;
pub mod search;

pub use error::MemoryError;
pub use embeddings::{EmbeddingProvider, OpenAIEmbeddings};
pub use store::{VectorStore, MemoryVectorStore, FileVectorStore};
pub use search::{SearchQuery, SearchResult};

/// Result type for memory operations.
pub type Result<T> = std::result::Result<T, MemoryError>;

/// A memory entry with vector embedding.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MemoryEntry {
    /// Unique identifier.
    pub id: String,

    /// Text content.
    pub content: String,

    /// Vector embedding.
    pub embedding: Vec<f32>,

    /// Metadata.
    pub metadata: std::collections::HashMap<String, serde_json::Value>,

    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl MemoryEntry {
    /// Create a new memory entry.
    pub fn new(content: impl Into<String>, embedding: Vec<f32>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            content: content.into(),
            embedding,
            metadata: std::collections::HashMap::new(),
            created_at: chrono::Utc::now(),
        }
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}
