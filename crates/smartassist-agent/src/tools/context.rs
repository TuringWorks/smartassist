//! Context management tools.
//!
//! Provides tools for managing conversation context, history,
//! and working state during agent execution.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;

/// A context entry representing a piece of information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    /// Unique identifier.
    pub id: String,
    /// Entry type (file, code, note, etc.).
    pub entry_type: String,
    /// Content or reference.
    pub content: String,
    /// Source (file path, URL, etc.).
    pub source: Option<String>,
    /// Timestamp when added.
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Tags for categorization.
    pub tags: Vec<String>,
}

/// Context store for managing working context.
#[derive(Debug, Default)]
pub struct ContextStore {
    entries: VecDeque<ContextEntry>,
    max_entries: usize,
}

impl ContextStore {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: 100,
        }
    }

    pub fn with_max_entries(max: usize) -> Self {
        Self {
            entries: VecDeque::new(),
            max_entries: max,
        }
    }

    /// Add an entry to the context.
    pub fn add(&mut self, entry: ContextEntry) {
        if self.entries.len() >= self.max_entries {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }

    /// Get all entries.
    pub fn get_all(&self) -> Vec<&ContextEntry> {
        self.entries.iter().collect()
    }

    /// Get entries by type.
    pub fn get_by_type(&self, entry_type: &str) -> Vec<&ContextEntry> {
        self.entries
            .iter()
            .filter(|e| e.entry_type == entry_type)
            .collect()
    }

    /// Get entries by tag.
    pub fn get_by_tag(&self, tag: &str) -> Vec<&ContextEntry> {
        self.entries
            .iter()
            .filter(|e| e.tags.contains(&tag.to_string()))
            .collect()
    }

    /// Get recent entries.
    pub fn get_recent(&self, count: usize) -> Vec<&ContextEntry> {
        self.entries.iter().rev().take(count).collect()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get entry count.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Shared context store.
pub type SharedContextStore = Arc<RwLock<ContextStore>>;

/// Tool for adding context entries.
pub struct ContextAddTool {
    store: SharedContextStore,
}

impl ContextAddTool {
    pub fn new(store: SharedContextStore) -> Self {
        Self { store }
    }

    pub fn with_new_store() -> Self {
        Self {
            store: Arc::new(RwLock::new(ContextStore::new())),
        }
    }
}

#[async_trait]
impl Tool for ContextAddTool {
    fn name(&self) -> &str {
        "context_add"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "context_add".to_string(),
            description: "Add information to the working context for later reference."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The content to add to context"
                    },
                    "entry_type": {
                        "type": "string",
                        "enum": ["file", "code", "note", "reference", "decision"],
                        "default": "note",
                        "description": "Type of context entry"
                    },
                    "source": {
                        "type": "string",
                        "description": "Source of the content (e.g., file path)"
                    },
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Tags for categorization"
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
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("content is required"))?;

        let entry_type = args
            .get("entry_type")
            .and_then(|v| v.as_str())
            .unwrap_or("note")
            .to_string();

        let source = args.get("source").and_then(|v| v.as_str()).map(String::from);

        let tags: Vec<String> = args
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let entry = ContextEntry {
            id: uuid::Uuid::new_v4().to_string(),
            entry_type: entry_type.clone(),
            content: content.to_string(),
            source,
            timestamp: chrono::Utc::now(),
            tags,
        };

        let entry_id = entry.id.clone();

        {
            let mut store = self.store.write().await;
            store.add(entry);
        }

        debug!("Added context entry: id={}, type={}", entry_id, entry_type);

        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "id": entry_id,
                "entry_type": entry_type,
                "message": "Context entry added successfully",
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for retrieving context entries.
pub struct ContextGetTool {
    store: SharedContextStore,
}

impl ContextGetTool {
    pub fn new(store: SharedContextStore) -> Self {
        Self { store }
    }

    pub fn with_new_store() -> Self {
        Self {
            store: Arc::new(RwLock::new(ContextStore::new())),
        }
    }
}

#[async_trait]
impl Tool for ContextGetTool {
    fn name(&self) -> &str {
        "context_get"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "context_get".to_string(),
            description: "Retrieve entries from the working context.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "entry_type": {
                        "type": "string",
                        "description": "Filter by entry type"
                    },
                    "tag": {
                        "type": "string",
                        "description": "Filter by tag"
                    },
                    "count": {
                        "type": "integer",
                        "default": 10,
                        "description": "Number of recent entries to return"
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
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let entry_type = args.get("entry_type").and_then(|v| v.as_str());
        let tag = args.get("tag").and_then(|v| v.as_str());
        let count = args
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let store = self.store.read().await;

        let entries: Vec<&ContextEntry> = if let Some(t) = entry_type {
            store.get_by_type(t)
        } else if let Some(tag) = tag {
            store.get_by_tag(tag)
        } else {
            store.get_recent(count)
        };

        let entries_json: Vec<serde_json::Value> = entries
            .iter()
            .take(count)
            .map(|e| {
                serde_json::json!({
                    "id": e.id,
                    "entry_type": e.entry_type,
                    "content": e.content,
                    "source": e.source,
                    "timestamp": e.timestamp.to_rfc3339(),
                    "tags": e.tags,
                })
            })
            .collect();

        let duration = start.elapsed();

        debug!("Retrieved {} context entries", entries_json.len());

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "entries": entries_json,
                "count": entries_json.len(),
                "total": store.len(),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for clearing context.
pub struct ContextClearTool {
    store: SharedContextStore,
}

impl ContextClearTool {
    pub fn new(store: SharedContextStore) -> Self {
        Self { store }
    }

    pub fn with_new_store() -> Self {
        Self {
            store: Arc::new(RwLock::new(ContextStore::new())),
        }
    }
}

#[async_trait]
impl Tool for ContextClearTool {
    fn name(&self) -> &str {
        "context_clear"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "context_clear".to_string(),
            description: "Clear the working context.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        _args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let count;
        {
            let mut store = self.store.write().await;
            count = store.len();
            store.clear();
        }

        debug!("Cleared {} context entries", count);

        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "cleared": count,
                "message": format!("Cleared {} context entries", count),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_store() {
        let mut store = ContextStore::new();

        let entry = ContextEntry {
            id: "1".to_string(),
            entry_type: "note".to_string(),
            content: "Test content".to_string(),
            source: None,
            timestamp: chrono::Utc::now(),
            tags: vec!["test".to_string()],
        };

        store.add(entry);
        assert_eq!(store.len(), 1);
        assert!(!store.is_empty());

        let entries = store.get_all();
        assert_eq!(entries.len(), 1);

        store.clear();
        assert!(store.is_empty());
    }

    #[test]
    fn test_context_store_max_entries() {
        let mut store = ContextStore::with_max_entries(2);

        for i in 0..5 {
            store.add(ContextEntry {
                id: i.to_string(),
                entry_type: "note".to_string(),
                content: format!("Content {}", i),
                source: None,
                timestamp: chrono::Utc::now(),
                tags: vec![],
            });
        }

        assert_eq!(store.len(), 2);
        // Should have entries 3 and 4 (oldest ones evicted)
        let entries = store.get_all();
        assert_eq!(entries[0].id, "3");
        assert_eq!(entries[1].id, "4");
    }

    #[test]
    fn test_context_add_tool_creation() {
        let tool = ContextAddTool::with_new_store();
        assert_eq!(tool.name(), "context_add");
    }

    #[test]
    fn test_context_get_tool_creation() {
        let tool = ContextGetTool::with_new_store();
        assert_eq!(tool.name(), "context_get");
    }

    #[test]
    fn test_context_clear_tool_creation() {
        let tool = ContextClearTool::with_new_store();
        assert_eq!(tool.name(), "context_clear");
    }

    #[tokio::test]
    async fn test_context_workflow() {
        let store = Arc::new(RwLock::new(ContextStore::new()));
        let add_tool = ContextAddTool::new(store.clone());
        let get_tool = ContextGetTool::new(store.clone());
        let clear_tool = ContextClearTool::new(store.clone());
        let ctx = ToolContext::default();

        // Add an entry
        let result = add_tool
            .execute(
                "test_id",
                serde_json::json!({
                    "content": "Test content",
                    "entry_type": "note",
                    "tags": ["test"]
                }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(!result.is_error);

        // Get entries
        let result = get_tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);
        assert_eq!(result.output.get("count").and_then(|v| v.as_u64()), Some(1));

        // Clear
        let result = clear_tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);
        assert_eq!(result.output.get("cleared").and_then(|v| v.as_u64()), Some(1));
    }
}
