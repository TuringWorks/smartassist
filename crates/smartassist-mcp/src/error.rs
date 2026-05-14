//! MCP error types.

use thiserror::Error;

/// Errors that can occur in the MCP subsystem.
#[derive(Debug, Error)]
pub enum McpError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Protocol error.
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Method not found.
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Invalid parameters.
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    /// Server not initialized.
    #[error("Server not initialized")]
    NotInitialized,

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// Transport error.
    #[error("Transport error: {0}")]
    Transport(String),

    /// Tool execution error.
    #[error("Tool error: {0}")]
    Tool(String),

    /// Timeout.
    #[error("Timeout: {0}")]
    Timeout(String),
}

/// Standard MCP JSON-RPC error codes.
impl McpError {
    /// Get the JSON-RPC error code.
    pub fn code(&self) -> i32 {
        match self {
            Self::MethodNotFound(_) => -32601,
            Self::InvalidParams(_) => -32602,
            Self::Json(_) => -32700,
            Self::NotInitialized => -32002,
            Self::Protocol(_) => -32001,
            _ => -32603,
        }
    }
}

/// Result type alias.
pub type Result<T> = std::result::Result<T, McpError>;
