//! MCP client implementation.
//!
//! Connects to an external MCP server via stdio or other transports.

use crate::error::{McpError, Result};
use crate::protocol::*;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::{debug, info};

/// MCP client connected to an external server process.
pub struct McpClient {
    /// Server process.
    process: Child,
    /// Stdin of the server process.
    stdin: ChildStdin,
    /// Request ID counter.
    next_id: AtomicU64,
}

impl McpClient {
    /// Spawn a new MCP client connected to a server command.
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Self> {
        info!("Spawning MCP server: {} {:?}", command, args);

        let mut cmd = Command::new(command);
        for arg in args {
            cmd.arg(arg);
        }

        let mut process = cmd
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(McpError::Io)?;

        let stdin = process
            .stdin
            .take()
            .ok_or_else(|| McpError::Transport("Failed to open server stdin".to_string()))?;

        let stdout = process
            .stdout
            .take()
            .ok_or_else(|| McpError::Transport("Failed to open server stdout".to_string()))?;

        let mut client = Self {
            process,
            stdin,
            next_id: AtomicU64::new(1),
        };

        // Initialize the connection
        client.initialize(stdout).await?;

        Ok(client)
    }

    async fn initialize(&mut self,
        stdout: ChildStdout,
    ) -> Result<()> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let params = InitializeParams {
            protocol_version: MCP_PROTOCOL_VERSION.to_string(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "smartassist-mcp-client".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(id.into()),
            method: "initialize".to_string(),
            params: Some(serde_json::to_value(params)?),
        };

        self.send_request(&request).await?;

        // Read initialize response
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        if let Ok(Some(line)) = lines.next_line().await {
            let response: JsonRpcResponse = serde_json::from_str(&line).map_err(McpError::Json)?;
            if response.error.is_some() {
                return Err(McpError::Protocol("Initialize failed".to_string()));
            }
            debug!("MCP initialize response: {:?}", response.result);
        }

        // Send initialized notification
        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/initialized".to_string(),
            params: None,
        };
        self.send_notification(&notification).await?;

        info!("MCP client initialized");
        Ok(())
    }

    async fn send_request(&mut self, request: &JsonRpcRequest) -> Result<()> {
        let json = serde_json::to_string(request)?;
        debug!("MCP client send: {}", json);
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn send_notification(&mut self, notification: &JsonRpcNotification) -> Result<()> {
        let json = serde_json::to_string(notification)?;
        debug!("MCP client notify: {}", json);
        self.stdin.write_all(json.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    /// List available tools from the server.
    pub async fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(id.into()),
            method: "tools/list".to_string(),
            params: None,
        };

        self.send_request(&request).await?;

        // TODO: read response from stdout
        // For now return empty since reading requires async coordination
        Ok(vec![])
    }

    /// Call a tool on the server.
    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: Option<Value>,
    ) -> Result<CallToolResult> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let params = CallToolParams {
            name: name.to_string(),
            arguments,
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(id.into()),
            method: "tools/call".to_string(),
            params: Some(serde_json::to_value(params)?),
        };

        self.send_request(&request).await?;

        // TODO: read response from stdout
        Err(McpError::Internal("Response reading not yet implemented".to_string()))
    }

    /// Shut down the client and kill the server process.
    pub async fn shutdown(mut self) -> Result<()> {
        info!("Shutting down MCP client");
        let _ = self.process.kill().await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: spawn tests require an actual MCP server binary.
    // These are placeholder tests for structure validation.

    #[test]
    fn test_mcp_tool_struct() {
        let tool = McpTool {
            name: "test".to_string(),
            description: None,
            input_schema: serde_json::json!({}),
        };
        assert_eq!(tool.name, "test");
    }
}
