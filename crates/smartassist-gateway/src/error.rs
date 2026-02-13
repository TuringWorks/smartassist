//! Gateway error types.

use thiserror::Error;

/// Errors that can occur in the gateway.
#[derive(Debug, Error)]
pub enum GatewayError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// WebSocket error.
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    /// RPC error.
    #[error("RPC error: {0}")]
    Rpc(String),

    /// Method not found.
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Invalid parameters.
    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    /// Authentication error.
    #[error("Authentication error: {0}")]
    Auth(String),

    /// Agent error.
    #[error("Agent error: {0}")]
    Agent(String),

    /// Session error.
    #[error("Session error: {0}")]
    Session(String),

    /// Not found error.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl GatewayError {
    /// Get the JSON-RPC error code.
    pub fn code(&self) -> i32 {
        match self {
            Self::MethodNotFound(_) => -32601,
            Self::InvalidParams(_) => -32602,
            Self::Json(_) => -32700,
            Self::Auth(_) => -32001,
            Self::NotFound(_) => -32002,
            _ => -32603,
        }
    }
}
