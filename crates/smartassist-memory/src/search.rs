//! Semantic search functionality.

use crate::embeddings::EmbeddingProvider;
use crate::store::VectorStore;
use crate::{MemoryEntry, Result};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Search query parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    /// Query text.
    pub text: String,

    /// Maximum results to return.
    #[serde(default = "default_limit")]
    pub limit: usize,

    /// Minimum similarity score (0.0 - 1.0).
    #[serde(default)]
    pub min_score: f32,

    /// Metadata filters.
    #[serde(default)]
    pub filters: std::collections::HashMap<String, serde_json::Value>,
}

fn default_limit() -> usize {
    10
}

impl SearchQuery {
    /// Create a new search query.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            limit: default_limit(),
            min_score: 0.0,
            filters: std::collections::HashMap::new(),
        }
    }

    /// Set the result limit.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set the minimum score threshold.
    pub fn with_min_score(mut self, score: f32) -> Self {
        self.min_score = score;
        self
    }

    /// Add a metadata filter.
    pub fn with_filter(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.filters.insert(key.into(), value);
        self
    }
}

/// Search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// The memory entry.
    pub entry: MemoryEntry,

    /// Similarity score (0.0 - 1.0).
    pub score: f32,
}

/// Semantic search engine.
pub struct SearchEngine {
    /// Embedding provider.
    embeddings: Arc<dyn EmbeddingProvider>,

    /// Vector store.
    store: Arc<dyn VectorStore>,
}

impl SearchEngine {
    /// Create a new search engine.
    pub fn new(embeddings: Arc<dyn EmbeddingProvider>, store: Arc<dyn VectorStore>) -> Self {
        Self { embeddings, store }
    }

    /// Add content to the search index.
    pub async fn index(&self, content: &str) -> Result<String> {
        let embedding = self.embeddings.embed_one(content).await?;
        let entry = MemoryEntry::new(content, embedding);
        let id = entry.id.clone();
        self.store.insert(entry).await?;
        Ok(id)
    }

    /// Add content with metadata.
    pub async fn index_with_metadata(
        &self,
        content: &str,
        metadata: std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        let embedding = self.embeddings.embed_one(content).await?;
        let mut entry = MemoryEntry::new(content, embedding);
        entry.metadata = metadata;
        let id = entry.id.clone();
        self.store.insert(entry).await?;
        Ok(id)
    }

    /// Search for similar content.
    pub async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        // Generate embedding for query
        let query_embedding = self.embeddings.embed_one(&query.text).await?;

        // Search the store
        let results = self.store.search(&query_embedding, query.limit * 2).await?;

        // Filter and transform results
        let results: Vec<SearchResult> = results
            .into_iter()
            .filter(|(entry, score)| {
                // Apply score filter
                if *score < query.min_score {
                    return false;
                }

                // Apply metadata filters
                for (key, value) in &query.filters {
                    if entry.metadata.get(key) != Some(value) {
                        return false;
                    }
                }

                true
            })
            .take(query.limit)
            .map(|(entry, score)| SearchResult { entry, score })
            .collect();

        Ok(results)
    }

    /// Delete an entry from the index.
    pub async fn delete(&self, id: &str) -> Result<()> {
        self.store.delete(id).await
    }

    /// Clear all entries.
    pub async fn clear(&self) -> Result<()> {
        self.store.clear().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_query() {
        let query = SearchQuery::new("test query")
            .with_limit(5)
            .with_min_score(0.5)
            .with_filter("type", serde_json::json!("document"));

        assert_eq!(query.text, "test query");
        assert_eq!(query.limit, 5);
        assert_eq!(query.min_score, 0.5);
        assert!(query.filters.contains_key("type"));
    }
}
