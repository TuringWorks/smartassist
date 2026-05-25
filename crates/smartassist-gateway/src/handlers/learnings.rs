//! Learnings RPC method handlers.

use crate::handlers::HandlerContext;
use crate::Result;
use async_trait::async_trait;
use smartassist_learnings::LearningSource;
use std::sync::Arc;
use tracing::debug;

/// List learning categories and entry counts.
pub struct LearningsListHandler {
    ctx: Arc<HandlerContext>,
}

impl LearningsListHandler {
    pub fn new(ctx: Arc<HandlerContext>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl crate::methods::MethodHandler for LearningsListHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let store = match &self.ctx.learning_store {
            Some(s) => s,
            None => {
                return Ok(serde_json::json!({
                    "categories": [],
                    "count": 0,
                    "message": "Learnings not configured",
                }));
            }
        };

        let categories = store.categories()?;

        // Filter by category if provided
        let filtered: Vec<_> = if let Some(p) = &params {
            if let Some(cat) = p.get("category").and_then(|v| v.as_str()) {
                categories.into_iter().filter(|c| c.name == cat).collect()
            } else {
                categories
            }
        } else {
            categories
        };

        let cat_list: Vec<serde_json::Value> = filtered
            .iter()
            .map(|c| {
                serde_json::json!({
                    "name": c.name,
                    "entry_count": c.entry_count,
                })
            })
            .collect();

        Ok(serde_json::json!({
            "categories": cat_list,
            "count": cat_list.len(),
        }))
    }
}

/// Get a specific learning entry.
pub struct LearningsGetHandler {
    ctx: Arc<HandlerContext>,
}

impl LearningsGetHandler {
    pub fn new(ctx: Arc<HandlerContext>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl crate::methods::MethodHandler for LearningsGetHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let store = match &self.ctx.learning_store {
            Some(s) => s,
            None => {
                return Ok(serde_json::json!({
                    "error": "Learnings not configured",
                }));
            }
        };

        let params = params.unwrap_or_default();
        let category = params.get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("general");
        let id = params.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::GatewayError::InvalidParams("Missing 'id' parameter".to_string()))?;

        match store.get_entry(category, id) {
            Ok(entry) => Ok(serde_json::json!({
                "id": entry.id,
                "title": entry.title,
                "content": entry.content,
                "category": category,
                "confidence": entry.confidence,
                "tags": entry.tags,
                "source": entry.source.to_string(),
                "created_at": entry.created_at.to_rfc3339(),
                "updated_at": entry.updated_at.to_rfc3339(),
            })),
            Err(e) => Ok(serde_json::json!({
                "error": format!("Entry not found: {}", e),
            })),
        }
    }
}

/// Ingest a document and extract learnings.
pub struct LearningsIngestHandler {
    ctx: Arc<HandlerContext>,
}

impl LearningsIngestHandler {
    pub fn new(ctx: Arc<HandlerContext>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl crate::methods::MethodHandler for LearningsIngestHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let store = match &self.ctx.learning_store {
            Some(s) => s,
            None => {
                return Ok(serde_json::json!({
                    "error": "Learnings not configured",
                }));
            }
        };

        let params = params.unwrap_or_default();
        let path = params.get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::GatewayError::InvalidParams("Missing 'path' parameter".to_string()))?;

        let category = params.get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("documents");

        let tags: Vec<String> = params.get("tags")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        debug!("Learnings ingest: path='{}', category='{}'", path, category);

        // Parse the document
        let parser = smartassist_learnings::CompositeParser::new();
        let file_path = std::path::PathBuf::from(path);
        let doc = parser.parse_file(&file_path).await
            .map_err(|e| crate::error::GatewayError::Internal(format!("Failed to parse document: {}", e)))?;

        // Extract learnings
        let config = smartassist_core::config::LearningsConfig::default();
        let extractor = smartassist_learnings::LearningExtractor::new(config);
        let source = LearningSource::Document { name: path.to_string() };
        let entries = extractor.extract(&doc, source, tags)
            .map_err(|e| crate::error::GatewayError::Internal(format!("Failed to extract learnings: {}", e)))?;

        let count = entries.len();

        // Store entries
        for entry in &entries {
            store.add_entry(category, entry)
                .map_err(|e| crate::error::GatewayError::Internal(format!("Failed to store learning: {}", e)))?;
        }

        Ok(serde_json::json!({
            "path": path,
            "category": category,
            "entries_extracted": count,
            "message": format!("Ingested {} learning entries from {}", count, path),
        }))
    }
}

/// Search learnings by keyword.
pub struct LearningsSearchHandler {
    ctx: Arc<HandlerContext>,
}

impl LearningsSearchHandler {
    pub fn new(ctx: Arc<HandlerContext>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl crate::methods::MethodHandler for LearningsSearchHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let store = match &self.ctx.learning_store {
            Some(s) => s,
            None => {
                return Ok(serde_json::json!({
                    "results": [],
                    "count": 0,
                    "message": "Learnings not configured",
                }));
            }
        };

        let params = params.unwrap_or_default();
        let query = params.get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::GatewayError::InvalidParams("Missing 'query' parameter".to_string()))?;

        let limit = params.get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let results = store.search_entries(query, limit)
            .map_err(|e| crate::error::GatewayError::Internal(format!("Search failed: {}", e)))?;

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
                })
            })
            .collect();

        Ok(serde_json::json!({
            "query": query,
            "results": entries,
            "count": entries.len(),
        }))
    }
}

/// Delete a learning entry.
pub struct LearningsDeleteHandler {
    ctx: Arc<HandlerContext>,
}

impl LearningsDeleteHandler {
    pub fn new(ctx: Arc<HandlerContext>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl crate::methods::MethodHandler for LearningsDeleteHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let store = match &self.ctx.learning_store {
            Some(s) => s,
            None => {
                return Ok(serde_json::json!({
                    "error": "Learnings not configured",
                }));
            }
        };

        let params = params.unwrap_or_default();
        let category = params.get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("general");
        let id = params.get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::GatewayError::InvalidParams("Missing 'id' parameter".to_string()))?;

        match store.delete_entry(category, id) {
            Ok(()) => Ok(serde_json::json!({
                "success": true,
                "message": format!("Deleted entry {}/{}", category, id),
            })),
            Err(e) => Ok(serde_json::json!({
                "error": format!("Failed to delete entry: {}", e),
            })),
        }
    }
}