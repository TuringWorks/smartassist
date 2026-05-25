//! Learning tools for document ingestion and knowledge retrieval.
//!
//! - [`LearningIngestTool`] — Ingest a document and extract learnings
//! - [`LearningSearchTool`] — Search learnings by keyword
//! - [`LearningListTool`] — List learning categories and entries

use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use smartassist_learnings::{
    CompositeParser, LearningExtractor, LearningSource, LearningStore,
};
use std::sync::Arc;
use std::time::Instant;
use tracing::debug;

// ---------------------------------------------------------------------------
// LearningIngestTool
// ---------------------------------------------------------------------------

/// Tool to ingest a document and extract learnings.
pub struct LearningIngestTool {
    store: Option<Arc<LearningStore>>,
    parser: CompositeParser,
}

impl Default for LearningIngestTool {
    fn default() -> Self {
        Self::new()
    }
}

impl LearningIngestTool {
    /// Create a new learning ingest tool.
    pub fn new() -> Self {
        Self {
            store: None,
            parser: CompositeParser::new(),
        }
    }

    /// Set the learning store.
    pub fn with_store(mut self, store: Arc<LearningStore>) -> Self {
        self.store = Some(store);
        self
    }
}

#[async_trait]
impl super::Tool for LearningIngestTool {
    fn name(&self) -> &str {
        "learning_ingest"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "learning_ingest".to_string(),
            description: "Ingest a document file and extract learnings into the knowledge store".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the document file to ingest"
                    },
                    "category": {
                        "type": "string",
                        "description": "Category to store learnings in (default: 'documents')"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Tags for the learning entries (optional)"
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
        _context: &super::ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let store = match &self.store {
            Some(s) => s.clone(),
            None => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, "Learning store not configured")
                        .with_duration(duration),
                );
            }
        };

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'path' argument"))?;

        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("documents");

        let tags: Vec<String> = args
            .get("tags")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        debug!("Learning ingest: path='{}', category='{}'", path, category);

        // Parse the document
        let file_path = std::path::PathBuf::from(path);
        let doc = match self.parser.parse_file(&file_path).await {
            Ok(d) => d,
            Err(e) => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, format!("Failed to parse document: {}", e))
                        .with_duration(duration),
                );
            }
        };

        // Extract learning entries
        let source = LearningSource::Document { name: path.to_string() };
        let extractor = LearningExtractor::extractive();
        let entries = match extractor.extract(&doc, source, tags) {
            Ok(e) => e,
            Err(e) => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, format!("Failed to extract learnings: {}", e))
                        .with_duration(duration),
                );
            }
        };

        let count = entries.len();

        // Store each entry
        for entry in &entries {
            if let Err(e) = store.add_entry(category, entry) {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, format!("Failed to store learning: {}", e))
                        .with_duration(duration),
                );
            }
        }

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "path": path,
                "category": category,
                "entries_extracted": count,
                "message": format!("Ingested {} learning entries from {}", count, path),
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Memory
    }
}

// ---------------------------------------------------------------------------
// LearningSearchTool
// ---------------------------------------------------------------------------

/// Tool to search learnings by keyword.
pub struct LearningSearchTool {
    store: Option<Arc<LearningStore>>,
}

impl Default for LearningSearchTool {
    fn default() -> Self {
        Self::new()
    }
}

impl LearningSearchTool {
    /// Create a new learning search tool.
    pub fn new() -> Self {
        Self { store: None }
    }

    /// Set the learning store.
    pub fn with_store(mut self, store: Arc<LearningStore>) -> Self {
        self.store = Some(store);
        self
    }
}

#[async_trait]
impl super::Tool for LearningSearchTool {
    fn name(&self) -> &str {
        "learning_search"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "learning_search".to_string(),
            description: "Search the learnings store for relevant knowledge".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum results to return (default: 10)"
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
        _context: &super::ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let store = match &self.store {
            Some(s) => s.clone(),
            None => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, "Learning store not configured")
                        .with_duration(duration),
                );
            }
        };

        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'query' argument"))?;

        let limit = args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        debug!("Learning search: query='{}', limit={}", query, limit);

        let results = match store.search_entries(query, limit) {
            Ok(r) => r,
            Err(e) => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, format!("Search failed: {}", e))
                        .with_duration(duration),
                );
            }
        };

        let count = results.len();
        let entries: Vec<serde_json::Value> = results
            .into_iter()
            .map(|(entry, category)| {
                serde_json::json!({
                    "id": entry.id,
                    "title": entry.title,
                    "content": entry.content,
                    "category": category,
                    "confidence": entry.confidence,
                    "tags": entry.tags,
                    "source": entry.source.to_string(),
                    "created_at": entry.created_at.to_rfc3339(),
                })
            })
            .collect();

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "query": query,
                "results": entries,
                "count": count,
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Memory
    }
}

// ---------------------------------------------------------------------------
// LearningListTool
// ---------------------------------------------------------------------------

/// Tool to list learning categories and entries.
pub struct LearningListTool {
    store: Option<Arc<LearningStore>>,
}

impl Default for LearningListTool {
    fn default() -> Self {
        Self::new()
    }
}

impl LearningListTool {
    /// Create a new learning list tool.
    pub fn new() -> Self {
        Self { store: None }
    }

    /// Set the learning store.
    pub fn with_store(mut self, store: Arc<LearningStore>) -> Self {
        self.store = Some(store);
        self
    }
}

#[async_trait]
impl super::Tool for LearningListTool {
    fn name(&self) -> &str {
        "learning_list"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "learning_list".to_string(),
            description: "List learning categories and their entries".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "description": "Category to list entries from (optional, lists all categories if omitted)"
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
        _context: &super::ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let store = match &self.store {
            Some(s) => s.clone(),
            None => {
                let duration = start.elapsed();
                return Ok(
                    ToolResult::error(tool_use_id, "Learning store not configured")
                        .with_duration(duration),
                );
            }
        };

        let category = args.get("category").and_then(|v| v.as_str());

        if let Some(cat) = category {
            // List entries in a specific category
            let entries = match store.list_entries(cat) {
                Ok(e) => e,
                Err(e) => {
                    let duration = start.elapsed();
                    return Ok(
                        ToolResult::error(tool_use_id, format!("Failed to list entries: {}", e))
                            .with_duration(duration),
                    );
                }
            };

            let entry_list: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "id": e.id,
                        "title": e.title,
                        "confidence": e.confidence,
                        "tags": e.tags,
                        "source": e.source.to_string(),
                        "created_at": e.created_at.to_rfc3339(),
                    })
                })
                .collect();

            let duration = start.elapsed();
            Ok(
                ToolResult::success(tool_use_id, serde_json::json!({
                    "category": cat,
                    "entries": entry_list,
                    "count": entry_list.len(),
                }))
                .with_duration(duration),
            )
        } else {
            // List all categories
            let categories = match store.categories() {
                Ok(c) => c,
                Err(e) => {
                    let duration = start.elapsed();
                    return Ok(
                        ToolResult::error(tool_use_id, format!("Failed to list categories: {}", e))
                            .with_duration(duration),
                    );
                }
            };

            let cat_list: Vec<serde_json::Value> = categories
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "name": c.name,
                        "entry_count": c.entry_count,
                    })
                })
                .collect();

            let duration = start.elapsed();
            Ok(
                ToolResult::success(tool_use_id, serde_json::json!({
                    "categories": cat_list,
                    "count": cat_list.len(),
                }))
                .with_duration(duration),
            )
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Memory
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::Tool;
    use tempfile::TempDir;

    fn setup_store() -> (TempDir, Arc<LearningStore>) {
        let dir = TempDir::new().unwrap();
        let store = LearningStore::with_dir(dir.path().join("md")).unwrap();
        (dir, Arc::new(store))
    }

    #[test]
    fn test_learning_ingest_tool_creation() {
        let tool = LearningIngestTool::new();
        assert_eq!(tool.name(), "learning_ingest");
    }

    #[test]
    fn test_learning_search_tool_creation() {
        let tool = LearningSearchTool::new();
        assert_eq!(tool.name(), "learning_search");
    }

    #[test]
    fn test_learning_list_tool_creation() {
        let tool = LearningListTool::new();
        assert_eq!(tool.name(), "learning_list");
    }

    #[test]
    fn test_learning_ingest_with_store() {
        let (_dir, store) = setup_store();
        let tool = LearningIngestTool::new().with_store(store);
        assert!(tool.store.is_some());
    }

    #[test]
    fn test_learning_search_with_store() {
        let (_dir, store) = setup_store();
        let tool = LearningSearchTool::new().with_store(store);
        assert!(tool.store.is_some());
    }

    #[test]
    fn test_learning_list_with_store() {
        let (_dir, store) = setup_store();
        let tool = LearningListTool::new().with_store(store);
        assert!(tool.store.is_some());
    }

    #[tokio::test]
    async fn test_learning_ingest_no_store() {
        let tool = LearningIngestTool::new();
        let args = serde_json::json!({ "path": "/tmp/test.md" });
        let result = tool
            .execute("test-id", args, &crate::tools::ToolContext::default())
            .await
            .unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_learning_search_no_store() {
        let tool = LearningSearchTool::new();
        let args = serde_json::json!({ "query": "test" });
        let result = tool
            .execute("test-id", args, &crate::tools::ToolContext::default())
            .await
            .unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_learning_list_no_store() {
        let tool = LearningListTool::new();
        let args = serde_json::json!({});
        let result = tool
            .execute("test-id", args, &crate::tools::ToolContext::default())
            .await
            .unwrap();
        assert!(result.is_error);
    }
}