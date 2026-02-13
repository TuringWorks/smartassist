//! Agent error types.

use std::io;
use thiserror::Error;

/// Errors that can occur during agent operations.
#[derive(Debug, Error)]
pub enum AgentError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// HTTP request error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Session not found.
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Session already exists.
    #[error("Session already exists: {0}")]
    SessionExists(String),

    /// Agent not found.
    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    /// Tool not found.
    #[error("Tool not found: {0}")]
    ToolNotFound(String),

    /// Tool execution error.
    #[error("Tool execution failed: {0}")]
    ToolExecution(String),

    /// Tool requires approval.
    #[error("Tool requires approval: {tool}")]
    ApprovalRequired {
        /// Tool name.
        tool: String,
        /// Approval request ID.
        request_id: String,
    },

    /// Approval denied.
    #[error("Approval denied for tool: {0}")]
    ApprovalDenied(String),

    /// Approval timeout.
    #[error("Approval timeout for tool: {0}")]
    ApprovalTimeout(String),

    /// Model API error.
    #[error("Model API error: {0}")]
    ModelApi(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: retry after {retry_after_secs} seconds")]
    RateLimit {
        /// Seconds to wait before retrying.
        retry_after_secs: u64,
    },

    /// Context limit exceeded.
    #[error("Context limit exceeded: {tokens} tokens (max: {max})")]
    ContextLimit {
        /// Actual token count.
        tokens: usize,
        /// Maximum allowed.
        max: usize,
    },

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Sandbox error.
    #[error("Sandbox error: {0}")]
    Sandbox(String),

    /// Channel error.
    #[error("Channel error: {0}")]
    Channel(String),

    /// Provider not configured.
    #[error("Provider not configured: {0}")]
    ProviderNotConfigured(String),

    /// Invalid state.
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Cancelled.
    #[error("Operation cancelled")]
    Cancelled,

    /// Timeout.
    #[error("Operation timed out")]
    Timeout,

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl AgentError {
    /// Create a model API error.
    pub fn model_api(msg: impl Into<String>) -> Self {
        Self::ModelApi(msg.into())
    }

    /// Create a tool execution error.
    pub fn tool_execution(msg: impl Into<String>) -> Self {
        Self::ToolExecution(msg.into())
    }

    /// Create a config error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create a rate limit error.
    pub fn rate_limit(retry_after_secs: u64) -> Self {
        Self::RateLimit { retry_after_secs }
    }

    /// Create a provider error.
    pub fn provider(msg: impl Into<String>) -> Self {
        Self::ModelApi(msg.into())
    }

    /// Check if this error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            Self::RateLimit { .. } | Self::Timeout | Self::Io(_) | Self::Http(_)
        )
    }

    /// Get retry delay if applicable.
    pub fn retry_delay(&self) -> Option<std::time::Duration> {
        match self {
            Self::RateLimit { retry_after_secs } => {
                Some(std::time::Duration::from_secs(*retry_after_secs))
            }
            Self::Timeout => Some(std::time::Duration::from_secs(1)),
            _ => None,
        }
    }
}

impl From<smartassist_sandbox::SandboxError> for AgentError {
    fn from(e: smartassist_sandbox::SandboxError) -> Self {
        Self::Sandbox(e.to_string())
    }
}

impl From<smartassist_channels::ChannelError> for AgentError {
    fn from(e: smartassist_channels::ChannelError) -> Self {
        Self::Channel(e.to_string())
    }
}

impl From<smartassist_core::error::SecurityError> for AgentError {
    fn from(e: smartassist_core::error::SecurityError) -> Self {
        Self::ToolExecution(e.to_string())
    }
}
