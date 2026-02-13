//! Memory tools.
//!
//! - [`MemorySearchTool`] - Semantic search of memory
//! - [`MemoryGetTool`] - Read memory files
//! - [`MemoryStoreTool`] - Store content with embeddings
//! - [`MemoryIndexTool`] - Index file content for semantic search

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use smartassist_memory::{EmbeddingProvider, MemoryEntry, VectorStore};
use std::sync::Arc;
use std::time::Instant;
use tracing::debug;

// ---------------------------------------------------------------------------
// MemorySearchTool
// ---------------------------------------------------------------------------

/// Memory search tool - Semantic search of memory.
pub struct MemorySearchTool {
    /// Maximum results to return.
    max_results: usize,

    /// Vector store for searching.
    store: Option<Arc<dyn VectorStore>>,

    /// Optional embedding provider for real query embeddings.
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
}

impl Default for MemorySearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MemorySearchTool {
    /// Create a new memory search tool.
    pub fn new() -> Self {
        Self {
            max_results: 10,
            store: None,
            embedding_provider: None,
        }
    }

    /// Set the maximum results to return.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    /// Set the vector store.
    pub fn with_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Set the embedding provider for real query embeddings.
    pub fn with_embedding_provider(mut self, provider: Arc<dyn EmbeddingProvider>) -> Self {
        self.embedding_provider = Some(provider);
        self
    }
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &str {
        "memory_search"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_search".to_string(),
            description: "Search the memory store for relevant information using semantic similarity".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of results to return (default: 10)"
                    },
                    "threshold": {
                        "type": "number",
                        "description": "Minimum similarity threshold (0.0-1.0, default: 0.7)"
                    },
                    "categories": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Filter by categories (optional)"
                    }
                },
                "required": ["query"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'query' argument"))?;

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.max_results as u64) as usize;

        let threshold = args
            .get("threshold")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.7) as f32;

        let categories: Option<Vec<String>> = args
            .get("categories")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        debug!(
            "Memory search: query='{}', limit={}, threshold={}, categories={:?}",
            query, limit, threshold, categories
        );

        // Check if we have a vector store configured
        let store = match &self.store {
            Some(s) => s.clone(),
            None => {
                // Try to get store from context
                match context.data.get("memory_store") {
                    Some(_store_value) => {
                        debug!("No memory store configured, returning empty results");
                        let duration = start.elapsed();
                        return Ok(
                            ToolResult::success(tool_use_id, serde_json::json!({
                                "query": query,
                                "results": [],
                                "count": 0,
                                "threshold": threshold,
                                "message": "Memory store not configured"
                            }))
                            .with_duration(duration),
                        );
                    }
                    None => {
                        let duration = start.elapsed();
                        return Ok(
                            ToolResult::success(tool_use_id, serde_json::json!({
                                "query": query,
                                "results": [],
                                "count": 0,
                                "threshold": threshold,
                                "message": "Memory store not configured"
                            }))
                            .with_duration(duration),
                        );
                    }
                }
            }
        };

        // Generate query embedding: use real provider if available, else fallback
        let query_embedding = if let Some(ref provider) = self.embedding_provider {
            provider.embed_one(query).await.map_err(|e| {
                AgentError::tool_execution(format!(
                    "Failed to generate query embedding: {}",
                    e
                ))
            })?
        } else {
            generate_simple_embedding(query)
        };

        // Search the store
        let results = store
            .search(&query_embedding, limit)
            .await
            .map_err(|e| AgentError::tool_execution(format!("Memory search failed: {}", e)))?;

        // Filter by threshold and convert to JSON
        let filtered_results: Vec<serde_json::Value> = results
            .into_iter()
            .filter(|(_, score)| *score >= threshold)
            .filter(|(entry, _)| {
                // Filter by categories if specified
                if let Some(ref cats) = categories {
                    if let Some(entry_cat) = entry.metadata.get("category") {
                        if let Some(cat_str) = entry_cat.as_str() {
                            return cats.iter().any(|c| c == cat_str);
                        }
                    }
                    false
                } else {
                    true
                }
            })
            .map(|(entry, score)| {
                serde_json::json!({
                    "id": entry.id,
                    "content": entry.content,
                    "score": score,
                    "metadata": entry.metadata,
                    "created_at": entry.created_at.to_rfc3339(),
                })
            })
            .collect();

        let count = filtered_results.len();

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "query": query,
                "results": filtered_results,
                "count": count,
                "threshold": threshold,
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Memory
    }
}

// ---------------------------------------------------------------------------
// MemoryGetTool
// ---------------------------------------------------------------------------

/// Memory get tool - Read memory files.
pub struct MemoryGetTool {
    /// Vector store for retrieval.
    store: Option<Arc<dyn VectorStore>>,
}

impl Default for MemoryGetTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryGetTool {
    /// Create a new memory get tool.
    pub fn new() -> Self {
        Self { store: None }
    }

    /// Set the vector store.
    pub fn with_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.store = Some(store);
        self
    }
}

#[async_trait]
impl Tool for MemoryGetTool {
    fn name(&self) -> &str {
        "memory_get"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_get".to_string(),
            description: "Get a specific memory entry by ID or path".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {
                        "type": "string",
                        "description": "The memory entry ID"
                    },
                    "path": {
                        "type": "string",
                        "description": "The memory file path (relative to memory store)"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let id = args.get("id").and_then(|v| v.as_str());
        let path = args.get("path").and_then(|v| v.as_str());

        if id.is_none() && path.is_none() {
            return Err(AgentError::tool_execution(
                "Either 'id' or 'path' must be provided",
            ));
        }

        debug!("Memory get: id={:?}, path={:?}", id, path);

        // Get the store
        let store = match &self.store {
            Some(s) => s.clone(),
            None => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, "Memory store not configured")
                        .with_duration(duration),
                );
            }
        };

        // If we have an ID, look it up directly
        if let Some(entry_id) = id {
            match store.get(entry_id).await {
                Ok(Some(entry)) => {
                    let duration = start.elapsed();
                    return Ok(
                        ToolResult::success(tool_use_id, serde_json::json!({
                            "id": entry.id,
                            "content": entry.content,
                            "metadata": entry.metadata,
                            "created_at": entry.created_at.to_rfc3339(),
                        }))
                        .with_duration(duration),
                    );
                }
                Ok(None) => {
                    let duration = start.elapsed();
                    return Ok(
                        ToolResult::error(tool_use_id, format!("Memory entry not found: {}", entry_id))
                            .with_duration(duration),
                    );
                }
                Err(e) => {
                    let duration = start.elapsed();
                    return Ok(
                        ToolResult::error(tool_use_id, format!("Failed to get memory: {}", e))
                            .with_duration(duration),
                    );
                }
            }
        }

        // If we have a path, search for it in metadata.
        // Use a large-limit search and filter entries whose metadata["path"] matches.
        if let Some(memory_path) = path {
            // Use a zero-vector query with a large limit to iterate all entries
            let all_results = store
                .search(&[], usize::MAX)
                .await
                .unwrap_or_default();

            for (entry, _score) in all_results {
                if let Some(entry_path) = entry.metadata.get("path") {
                    if entry_path.as_str() == Some(memory_path) {
                        let duration = start.elapsed();
                        return Ok(
                            ToolResult::success(tool_use_id, serde_json::json!({
                                "id": entry.id,
                                "content": entry.content,
                                "metadata": entry.metadata,
                                "created_at": entry.created_at.to_rfc3339(),
                            }))
                            .with_duration(duration),
                        );
                    }
                }
            }

            let duration = start.elapsed();
            return Ok(
                ToolResult::error(
                    tool_use_id,
                    format!("No memory entry found for path: {}", memory_path),
                )
                .with_duration(duration),
            );
        }

        let duration = start.elapsed();
        Ok(
            ToolResult::error(tool_use_id, "Memory entry not found")
                .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Memory
    }
}

// ---------------------------------------------------------------------------
// MemoryStoreTool
// ---------------------------------------------------------------------------

/// Memory store tool - Store content with embeddings for later retrieval.
pub struct MemoryStoreTool {
    /// Vector store for persistence.
    store: Option<Arc<dyn VectorStore>>,

    /// Embedding provider for generating vector embeddings.
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
}

impl Default for MemoryStoreTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStoreTool {
    /// Create a new memory store tool.
    pub fn new() -> Self {
        Self {
            store: None,
            embedding_provider: None,
        }
    }

    /// Set the vector store.
    pub fn with_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Set the embedding provider.
    pub fn with_embedding_provider(mut self, provider: Arc<dyn EmbeddingProvider>) -> Self {
        self.embedding_provider = Some(provider);
        self
    }
}

#[async_trait]
impl Tool for MemoryStoreTool {
    fn name(&self) -> &str {
        "memory_store"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_store".to_string(),
            description: "Store content in the memory store with embeddings for later retrieval"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to store"
                    },
                    "category": {
                        "type": "string",
                        "description": "Category for the memory entry (optional)"
                    },
                    "path": {
                        "type": "string",
                        "description": "File path associated with this memory (optional)"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Tags for the memory entry (optional)"
                    }
                },
                "required": ["content"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let provider = match &self.embedding_provider {
            Some(p) => p.clone(),
            None => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, "Embedding provider not configured")
                        .with_duration(duration),
                );
            }
        };

        let store = match &self.store {
            Some(s) => s.clone(),
            None => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, "Memory store not configured")
                        .with_duration(duration),
                );
            }
        };

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'content' argument"))?;

        let category = args.get("category").and_then(|v| v.as_str());
        let path = args.get("path").and_then(|v| v.as_str());
        let tags: Option<Vec<String>> = args
            .get("tags")
            .and_then(|v| serde_json::from_value(v.clone()).ok());

        debug!(
            "Memory store: content_len={}, category={:?}, path={:?}, tags={:?}",
            content.len(),
            category,
            path,
            tags
        );

        // Generate embedding via provider
        let embedding = provider.embed_one(content).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to generate embedding: {}", e))
        })?;

        // Build the memory entry with metadata
        let mut entry = MemoryEntry::new(content, embedding);

        if let Some(cat) = category {
            entry = entry.with_metadata("category", serde_json::Value::String(cat.to_string()));
        }
        if let Some(p) = path {
            entry = entry.with_metadata("path", serde_json::Value::String(p.to_string()));
        }
        if let Some(t) = tags {
            entry = entry.with_metadata("tags", serde_json::json!(t));
        }

        let entry_id = entry.id.clone();

        store.insert(entry).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to store memory entry: {}", e))
        })?;

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "id": entry_id,
                "message": "Memory entry stored successfully",
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Memory
    }
}

// ---------------------------------------------------------------------------
// MemoryIndexTool
// ---------------------------------------------------------------------------

/// Memory index tool - Index a file's content for semantic search.
pub struct MemoryIndexTool {
    /// Vector store for persistence.
    store: Option<Arc<dyn VectorStore>>,

    /// Embedding provider for generating vector embeddings.
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
}

impl Default for MemoryIndexTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryIndexTool {
    /// Create a new memory index tool.
    pub fn new() -> Self {
        Self {
            store: None,
            embedding_provider: None,
        }
    }

    /// Set the vector store.
    pub fn with_store(mut self, store: Arc<dyn VectorStore>) -> Self {
        self.store = Some(store);
        self
    }

    /// Set the embedding provider.
    pub fn with_embedding_provider(mut self, provider: Arc<dyn EmbeddingProvider>) -> Self {
        self.embedding_provider = Some(provider);
        self
    }
}

#[async_trait]
impl Tool for MemoryIndexTool {
    fn name(&self) -> &str {
        "memory_index"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_index".to_string(),
            description: "Index a file's content into the memory store for semantic search"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The file path to index"
                    },
                    "chunk_size": {
                        "type": "integer",
                        "description": "Size of each chunk in characters (default: 1000)"
                    }
                },
                "required": ["path"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let provider = match &self.embedding_provider {
            Some(p) => p.clone(),
            None => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, "Embedding provider not configured")
                        .with_duration(duration),
                );
            }
        };

        let store = match &self.store {
            Some(s) => s.clone(),
            None => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, "Memory store not configured")
                        .with_duration(duration),
                );
            }
        };

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'path' argument"))?;

        let chunk_size = args
            .get("chunk_size")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000) as usize;

        debug!("Memory index: path='{}', chunk_size={}", path, chunk_size);

        // Read file content
        let content = tokio::fs::read_to_string(path).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to read file '{}': {}", path, e))
        })?;

        // Split into overlapping chunks (overlap of 200 chars)
        let overlap = 200;
        let chunks = chunk_text(&content, chunk_size, overlap);
        let chunk_count = chunks.len();

        // Generate embeddings for all chunks
        let chunk_strings: Vec<String> = chunks.iter().map(|s| s.to_string()).collect();
        let embeddings = provider.embed(&chunk_strings).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to generate embeddings: {}", e))
        })?;

        // Store each chunk with metadata
        let mut entries = Vec::with_capacity(chunk_count);
        for (i, (chunk, embedding)) in chunks.into_iter().zip(embeddings).enumerate() {
            let entry = MemoryEntry::new(chunk, embedding)
                .with_metadata("path", serde_json::Value::String(path.to_string()))
                .with_metadata("chunk_index", serde_json::json!(i));
            entries.push(entry);
        }

        store.insert_batch(entries).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to store indexed chunks: {}", e))
        })?;

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "path": path,
                "chunks_indexed": chunk_count,
                "chunk_size": chunk_size,
                "message": format!("Indexed {} chunks from {}", chunk_count, path),
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Memory
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Generate a simple embedding for a query (fallback when no provider is configured).
/// In production, use a real embedding model via the EmbeddingProvider trait.
fn generate_simple_embedding(text: &str) -> Vec<f32> {
    // Simple character-based embedding for demonstration
    let mut embedding = vec![0.0f32; 128];

    for (i, ch) in text.chars().enumerate() {
        let idx = (ch as usize + i) % embedding.len();
        embedding[idx] += 1.0;
    }

    // Normalize
    let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if magnitude > 0.0 {
        for x in &mut embedding {
            *x /= magnitude;
        }
    }

    embedding
}

/// Split text into overlapping chunks.
///
/// Each chunk is at most `chunk_size` characters, and consecutive chunks overlap
/// by `overlap` characters so that context at chunk boundaries is preserved.
fn chunk_text(text: &str, chunk_size: usize, overlap: usize) -> Vec<String> {
    if text.is_empty() {
        return vec![];
    }

    let chars: Vec<char> = text.chars().collect();
    let mut chunks = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + chunk_size).min(chars.len());
        let chunk: String = chars[start..end].iter().collect();
        chunks.push(chunk);

        // Advance by (chunk_size - overlap), but ensure we always move forward
        let step = if chunk_size > overlap {
            chunk_size - overlap
        } else {
            chunk_size
        };
        start += step;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;
    use smartassist_memory::MemoryVectorStore;

    #[test]
    fn test_memory_search_tool_creation() {
        let tool = MemorySearchTool::new();
        assert_eq!(tool.name(), "memory_search");
    }

    #[test]
    fn test_memory_get_tool_creation() {
        let tool = MemoryGetTool::new();
        assert_eq!(tool.name(), "memory_get");
    }

    #[test]
    fn test_memory_store_tool_creation() {
        let tool = MemoryStoreTool::new();
        assert_eq!(tool.name(), "memory_store");
        // Verify the definition has the expected required field
        let def = tool.definition();
        assert_eq!(def.name, "memory_store");
    }

    #[test]
    fn test_memory_index_tool_creation() {
        let tool = MemoryIndexTool::new();
        assert_eq!(tool.name(), "memory_index");
        let def = tool.definition();
        assert_eq!(def.name, "memory_index");
    }

    #[test]
    fn test_memory_search_custom_max_results() {
        let tool = MemorySearchTool::new().with_max_results(20);
        assert_eq!(tool.max_results, 20);
    }

    #[test]
    fn test_memory_search_with_store() {
        let store = Arc::new(MemoryVectorStore::new());
        let tool = MemorySearchTool::new().with_store(store);
        assert!(tool.store.is_some());
    }

    #[test]
    fn test_memory_get_with_store() {
        let store = Arc::new(MemoryVectorStore::new());
        let tool = MemoryGetTool::new().with_store(store);
        assert!(tool.store.is_some());
    }

    #[test]
    fn test_memory_store_tool_with_store() {
        let store = Arc::new(MemoryVectorStore::new());
        let tool = MemoryStoreTool::new().with_store(store);
        assert!(tool.store.is_some());
    }

    #[test]
    fn test_memory_index_tool_with_store() {
        let store = Arc::new(MemoryVectorStore::new());
        let tool = MemoryIndexTool::new().with_store(store);
        assert!(tool.store.is_some());
    }

    #[test]
    fn test_simple_embedding() {
        let embedding = generate_simple_embedding("hello world");
        assert_eq!(embedding.len(), 128);

        // Check it's normalized (magnitude ~= 1.0)
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_chunk_text() {
        let text = "abcdefghij"; // 10 chars
        let chunks = chunk_text(text, 4, 2);
        // step = 4-2 = 2; starts: 0, 2, 4, 6, 8
        assert_eq!(chunks.len(), 5);
        assert_eq!(chunks[0], "abcd");
        assert_eq!(chunks[1], "cdef");
        assert_eq!(chunks[2], "efgh");
        assert_eq!(chunks[3], "ghij");
        assert_eq!(chunks[4], "ij");
    }

    #[test]
    fn test_chunk_text_empty() {
        let chunks = chunk_text("", 100, 20);
        assert!(chunks.is_empty());
    }

    #[test]
    fn test_chunk_text_smaller_than_chunk_size() {
        let chunks = chunk_text("hello", 100, 20);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], "hello");
    }

    #[tokio::test]
    async fn test_memory_search_with_store_integration() {
        let store = Arc::new(MemoryVectorStore::new());

        // Add some test entries
        let entry1 = MemoryEntry::new("Hello world", generate_simple_embedding("Hello world"));
        let entry2 = MemoryEntry::new("Goodbye world", generate_simple_embedding("Goodbye world"));
        store.insert(entry1).await.unwrap();
        store.insert(entry2).await.unwrap();

        let tool = MemorySearchTool::new().with_store(store);

        let args = serde_json::json!({
            "query": "Hello",
            "limit": 5
        });

        let result = tool
            .execute("test-id", args, &ToolContext::default())
            .await
            .unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_memory_get_with_store_integration() {
        let store = Arc::new(MemoryVectorStore::new());

        // Add a test entry
        let entry = MemoryEntry::new("Test content", vec![1.0, 0.0, 0.0]);
        let entry_id = entry.id.clone();
        store.insert(entry).await.unwrap();

        let tool = MemoryGetTool::new().with_store(store);

        let args = serde_json::json!({
            "id": entry_id
        });

        let result = tool
            .execute("test-id", args, &ToolContext::default())
            .await
            .unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_memory_store_tool_no_provider() {
        let store = Arc::new(MemoryVectorStore::new());
        let tool = MemoryStoreTool::new().with_store(store);

        let args = serde_json::json!({ "content": "test data" });
        let result = tool
            .execute("test-id", args, &ToolContext::default())
            .await
            .unwrap();

        // Should fail because no embedding provider is configured
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_memory_index_tool_no_provider() {
        let store = Arc::new(MemoryVectorStore::new());
        let tool = MemoryIndexTool::new().with_store(store);

        let args = serde_json::json!({ "path": "/tmp/test.txt" });
        let result = tool
            .execute("test-id", args, &ToolContext::default())
            .await
            .unwrap();

        // Should fail because no embedding provider is configured
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_memory_get_path_lookup() {
        let store = Arc::new(MemoryVectorStore::new());

        // Add an entry with a path in metadata
        let entry = MemoryEntry::new("file content here", vec![1.0, 0.0, 0.0])
            .with_metadata("path", serde_json::Value::String("/docs/readme.md".to_string()));
        store.insert(entry).await.unwrap();

        let tool = MemoryGetTool::new().with_store(store);

        let args = serde_json::json!({ "path": "/docs/readme.md" });
        let result = tool
            .execute("test-id", args, &ToolContext::default())
            .await
            .unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_memory_get_path_not_found() {
        let store = Arc::new(MemoryVectorStore::new());
        let tool = MemoryGetTool::new().with_store(store);

        let args = serde_json::json!({ "path": "/nonexistent/path" });
        let result = tool
            .execute("test-id", args, &ToolContext::default())
            .await
            .unwrap();

        assert!(result.is_error);
    }
}
