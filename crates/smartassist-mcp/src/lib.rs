//! SmartAssist MCP (Model Context Protocol) integration.
//!
//! Provides an MCP server/client for exposing SmartAssist tools to external
//! MCP-compatible consumers (Claude Desktop, Claude Code, etc.).
//!
//! # Modules
//!
//! - [`protocol`] — MCP JSON-RPC message types
//! - [`server`] — MCP server implementation with stdio transport
//! - [`client`] — MCP client for connecting to external servers
//! - [`bridge`] — Bridge from SmartAssist [`ToolRegistry`] to MCP tools
//!
//! # Example
//!
//! ```rust,no_run
//! use smartassist_mcp::{McpServer, SmartAssistToolBridge};
//! use smartassist_agent::{ToolRegistry, ToolExecutor};
//! use std::sync::Arc;
//!
//! #[tokio::main]
//! async fn main() {
//!     let server = McpServer::new();
//!
//!     let registry = Arc::new(ToolRegistry::new());
//!     let executor = Arc::new(ToolExecutor::new(registry.clone()));
//!     let bridge = Arc::new(SmartAssistToolBridge::new(registry, executor));
//!
//!     server.register_tools("smartassist", bridge);
//!     // server.run_stdio().await.unwrap();
//! }
//! ```

pub mod bridge;
pub mod client;
pub mod error;
pub mod protocol;
pub mod server;

pub use bridge::SmartAssistToolBridge;
pub use client::McpClient;
pub use error::{McpError, Result};
pub use protocol::*;
pub use server::{McpServer, ToolHandler};
