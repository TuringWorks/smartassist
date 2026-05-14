//! MCP server implementation.
//!
//! Supports stdio transport for external MCP clients.

use crate::error::{McpError, Result};
use crate::protocol::*;
use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// A handler for MCP tool calls.
#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    /// List available tools.
    async fn list_tools(&self) -> Vec<McpTool>;

    /// Call a tool by name with arguments.
    async fn call_tool(&self, name: &str, arguments: Option<Value>) -> Result<CallToolResult>;
}

/// MCP server over stdio transport.
pub struct McpServer {
    /// Whether the server has been initialized.
    initialized: RwLock<bool>,
    /// Registered tool handlers by namespace.
    tool_handlers: DashMap<String, Arc<dyn ToolHandler>>,
    /// Server capabilities.
    capabilities: ServerCapabilities,
}

impl McpServer {
    /// Create a new MCP server.
    pub fn new() -> Self {
        Self {
            initialized: RwLock::new(false),
            tool_handlers: DashMap::new(),
            capabilities: ServerCapabilities {
                tools: Some(serde_json::json!({})),
                resources: None,
                prompts: None,
                logging: None,
            },
        }
    }

    /// Register a tool handler under a namespace.
    pub fn register_tools(&self, namespace: impl Into<String>, handler: Arc<dyn ToolHandler>) {
        let ns = namespace.into();
        info!("Registering MCP tool handler: {}", ns);
        self.tool_handlers.insert(ns, handler);
    }

    /// Run the server on stdio.
    pub async fn run_stdio(&self) -> Result<()> {
        info!("MCP server starting on stdio");

        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();
        let mut stdout = stdout;

        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            debug!("MCP recv: {}", line);

            let response = match self.handle_message(&line).await {
                Some(resp) => resp,
                None => continue,
            };

            let resp_json = serde_json::to_string(&response)?;
            debug!("MCP send: {}", resp_json);

            stdout.write_all(resp_json.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        info!("MCP server shutting down");
        Ok(())
    }

    /// Handle a single JSON-RPC message.
    async fn handle_message(&self, line: &str) -> Option<JsonRpcResponse> {
        let request: JsonRpcRequest = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(e) => {
                warn!("Failed to parse JSON-RPC request: {}", e);
                return Some(error_response(None, -32700, format!("Parse error: {}", e)));
            }
        };

        let id = request.id.clone();

        // Notifications don't get responses
        if id.is_none() {
            self.handle_notification(request).await;
            return None;
        }

        let result = match request.method.as_str() {
            "initialize" => self.handle_initialize(request.params).await,
            "tools/list" => self.handle_tools_list().await,
            "tools/call" => self.handle_tools_call(request.params).await,
            "ping" => Ok(serde_json::json!({})),
            _ => {
                if !*self.initialized.read().await {
                    return Some(error_response(id, -32002, "Server not initialized"));
                }
                Err(McpError::MethodNotFound(request.method))
            }
        };

        let response = match result {
            Ok(result) => success_response(id, result),
            Err(e) => error_response(id, e.code(), e.to_string()),
        };

        Some(response)
    }

    async fn handle_notification(&self, request: JsonRpcRequest) {
        match request.method.as_str() {
            "notifications/initialized" => {
                debug!("Client initialized notification received");
            }
            _ => {
                debug!("Unhandled notification: {}", request.method);
            }
        }
    }

    async fn handle_initialize(&self, params: Option<Value>) -> Result<Value> {
        let _params: InitializeParams = match params {
            Some(p) => serde_json::from_value(p).map_err(|e| McpError::InvalidParams(e.to_string()))?,
            None => return Err(McpError::InvalidParams("Missing initialize params".to_string())),
        };

        let mut initialized = self.initialized.write().await;
        *initialized = true;

        let result = InitializeResult {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: self.capabilities.clone(),
            server_info: Implementation {
                name: "smartassist-mcp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        info!("MCP client initialized");
        Ok(serde_json::to_value(result)?)
    }

    async fn handle_tools_list(&self) -> Result<Value> {
        if !*self.initialized.read().await {
            return Err(McpError::NotInitialized);
        }

        let mut tools = Vec::new();
        for entry in self.tool_handlers.iter() {
            let handler_tools = entry.value().list_tools().await;
            tools.extend(handler_tools);
        }

        Ok(serde_json::json!({ "tools": tools }))
    }

    async fn handle_tools_call(&self, params: Option<Value>) -> Result<Value> {
        if !*self.initialized.read().await {
            return Err(McpError::NotInitialized);
        }

        let params: CallToolParams = match params {
            Some(p) => serde_json::from_value(p).map_err(|e| McpError::InvalidParams(e.to_string()))?,
            None => return Err(McpError::InvalidParams("Missing tool call params".to_string())),
        };

        // Try each handler until one succeeds
        for entry in self.tool_handlers.iter() {
            match entry.value().call_tool(&params.name, params.arguments.clone()).await {
                Ok(result) => return Ok(serde_json::to_value(result)?),
                Err(McpError::MethodNotFound(_)) => continue,
                Err(e) => return Err(e),
            }
        }

        Err(McpError::MethodNotFound(params.name))
    }
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestToolHandler;

    #[async_trait::async_trait]
    impl ToolHandler for TestToolHandler {
        async fn list_tools(&self) -> Vec<McpTool> {
            vec![McpTool {
                name: "test_tool".to_string(),
                description: Some("A test tool".to_string()),
                input_schema: serde_json::json!({"type": "object"}),
            }]
        }

        async fn call_tool(&self, name: &str, _args: Option<Value>) -> Result<CallToolResult> {
            if name == "test_tool" {
                Ok(CallToolResult {
                    content: vec![McpContent::Text {
                        text: "ok".to_string(),
                    }],
                    is_error: None,
                })
            } else {
                Err(McpError::MethodNotFound(name.to_string()))
            }
        }
    }

    #[tokio::test]
    async fn test_server_initialize() {
        let server = McpServer::new();

        let params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test".to_string(),
                version: "1.0".to_string(),
            },
        };

        let result = server
            .handle_initialize(Some(serde_json::to_value(params).unwrap()))
            .await
            .unwrap();

        assert_eq!(result["protocolVersion"], MCP_PROTOCOL_VERSION);
        assert_eq!(result["serverInfo"]["name"], "smartassist-mcp");
    }

    #[tokio::test]
    async fn test_tools_list_before_init() {
        let server = McpServer::new();
        let result = server.handle_tools_list().await;
        assert!(matches!(result, Err(McpError::NotInitialized)));
    }

    #[tokio::test]
    async fn test_tools_list_and_call() {
        let server = McpServer::new();
        server.register_tools("default", Arc::new(TestToolHandler));

        // Initialize
        let params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test".to_string(),
                version: "1.0".to_string(),
            },
        };
        server.handle_initialize(Some(serde_json::to_value(params).unwrap())).await.unwrap();

        // List tools
        let result = server.handle_tools_list().await.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "test_tool");

        // Call tool
        let call_params = CallToolParams {
            name: "test_tool".to_string(),
            arguments: None,
        };
        let result = server.handle_tools_call(Some(serde_json::to_value(call_params).unwrap())).await.unwrap();
        let content = result["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "text");
    }

    #[tokio::test]
    async fn test_tools_call_not_found() {
        let server = McpServer::new();
        server.register_tools("default", Arc::new(TestToolHandler));

        let params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "test".to_string(),
                version: "1.0".to_string(),
            },
        };
        server.handle_initialize(Some(serde_json::to_value(params).unwrap())).await.unwrap();

        let call_params = CallToolParams {
            name: "missing_tool".to_string(),
            arguments: None,
        };
        let result = server.handle_tools_call(Some(serde_json::to_value(call_params).unwrap())).await;
        assert!(matches!(result, Err(McpError::MethodNotFound(_))));
    }
}
