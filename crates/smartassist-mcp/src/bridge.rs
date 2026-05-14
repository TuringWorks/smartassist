//! MCP bridge between SmartAssist tools and MCP protocol.
//!
//! Wraps [`smartassist_agent::ToolRegistry`] as an MCP [`ToolHandler`].

use crate::error::{McpError, Result};
use crate::protocol::*;
use crate::server::ToolHandler;
use serde_json::Value;
use smartassist_agent::{ToolExecutor, ToolRegistry};
use std::sync::Arc;

/// Bridge that exposes SmartAssist tools via MCP.
pub struct SmartAssistToolBridge {
    registry: Arc<ToolRegistry>,
    executor: Arc<ToolExecutor>,
}

impl SmartAssistToolBridge {
    /// Create a new bridge from a tool registry and executor.
    pub fn new(registry: Arc<ToolRegistry>, executor: Arc<ToolExecutor>) -> Self {
        Self { registry, executor }
    }

    /// Convert a SmartAssist tool to an MCP tool definition.
    fn to_mcp_tool(&self, name: &str, description: Option<&str>) -> McpTool {
        McpTool {
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            // Tools use JSON schema for args; use a permissive default
            input_schema: serde_json::json!({
                "type": "object",
                "additionalProperties": true
            }),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for SmartAssistToolBridge {
    async fn list_tools(&self) -> Vec<McpTool> {
        // TODO: enumerate tools from registry once the API exposes names.
        // For now, return a static set of known tools as a placeholder.
        vec![
            self.to_mcp_tool("read_file", Some("Read the contents of a file")),
            self.to_mcp_tool("write_file", Some("Write content to a file")),
            self.to_mcp_tool("bash", Some("Execute a bash command")),
            self.to_mcp_tool("search", Some("Search for text in files")),
        ]
    }

    async fn call_tool(&self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<CallToolResult> {
        let args = arguments.unwrap_or_else(|| serde_json::json!({}));

        // For now, simulate tool results based on name.
        // In a full implementation, this would dispatch via ToolExecutor.
        let text = match name {
            "read_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("unknown");
                format!("Simulated read of file: {}", path)
            }
            "write_file" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("unknown");
                format!("Simulated write to file: {}", path)
            }
            "bash" => {
                let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
                format!("Simulated bash: {}", cmd)
            }
            "search" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                format!("Simulated search for: {}", query)
            }
            _ => return Err(McpError::MethodNotFound(name.to_string())),
        };

        Ok(CallToolResult {
            content: vec![McpContent::Text { text }],
            is_error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_bridge() -> SmartAssistToolBridge {
        let registry = Arc::new(ToolRegistry::new());
        let executor = Arc::new(ToolExecutor::new(registry.clone()));
        SmartAssistToolBridge::new(registry, executor)
    }

    #[tokio::test]
    async fn test_list_tools() {
        let bridge = test_bridge();
        let tools = bridge.list_tools().await;
        assert_eq!(tools.len(), 4);
        assert_eq!(tools[0].name, "read_file");
    }

    #[tokio::test]
    async fn test_call_read_file() {
        let bridge = test_bridge();
        let result = bridge
            .call_tool(
                "read_file",
                Some(serde_json::json!({ "path": "/etc/hosts" })),
            )
            .await
            .unwrap();
        assert_eq!(result.content.len(), 1);
        match &result.content[0] {
            McpContent::Text { text } => assert!(text.contains("/etc/hosts")),
            _ => panic!("Expected text content"),
        }
    }

    #[tokio::test]
    async fn test_call_unknown_tool() {
        let bridge = test_bridge();
        let result = bridge.call_tool("unknown_tool", None).await;
        assert!(matches!(result, Err(McpError::MethodNotFound(_))));
    }
}
