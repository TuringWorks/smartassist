//! Error types for SmartAssist core.

use std::path::PathBuf;
use thiserror::Error;

/// Core result type alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Top-level error type for SmartAssist core operations.
#[derive(Debug, Error)]
pub enum Error {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Security error: {0}")]
    Security(#[from] SecurityError),
}

/// Configuration-related errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Configuration file not found: {0}")]
    NotFound(PathBuf),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON5 parse error: {0}")]
    Json5(String),
}

/// Security-related errors.
#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("Blocked environment variable: {0}")]
    BlockedEnvVar(String),

    #[error("Path traversal attempt: {attempted} escapes {workspace}")]
    PathTraversal {
        attempted: PathBuf,
        workspace: PathBuf,
    },

    #[error("Absolute path not allowed")]
    AbsolutePathNotAllowed,

    #[error("Insecure file permissions: {0:o}")]
    InsecurePermissions(u32),

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Insufficient scope: required {required}, have {available}")]
    InsufficientScope {
        required: String,
        available: String,
    },

    #[error("Approval denied: {0}")]
    ApprovalDenied(String),

    #[error("Approval timeout")]
    ApprovalTimeout,

    #[error("Prompt injection detected: {pattern}")]
    InjectionDetected { pattern: String, severity: String },

    #[error("Secret leak detected: {pattern_name}")]
    LeakDetected { pattern_name: String, action: String },

    #[error("Input validation failed: {reason}")]
    InputValidation { reason: String },

    #[error("Safety policy violation: {rule}")]
    PolicyViolation { rule: String, severity: String },
}

/// Channel-related errors.
#[derive(Debug, Error)]
pub enum ChannelError {
    #[error("Channel not found: {0}")]
    NotFound(String),

    #[error("Channel not connected")]
    NotConnected,

    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Network error: {0}")]
    Network(String),

    #[error("Service unavailable")]
    ServiceUnavailable,

    #[error("Invalid target: {0}")]
    InvalidTarget(String),

    #[error("Unsupported media type")]
    UnsupportedMediaType,

    #[error("Message too long: {len} > {max}")]
    MessageTooLong { len: usize, max: usize },

    #[error("API error: {0}")]
    Api(String),
}

/// Agent-related errors.
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("Session error: {0}")]
    Session(String),

    #[error("Model error: {0}")]
    Model(#[from] ModelError),

    #[error("Tool error in {tool}: {message}")]
    Tool { tool: String, message: String },

    #[error("Sandbox error: {0}")]
    Sandbox(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Context length exceeded: {used} / {max}")]
    ContextLengthExceeded { used: usize, max: usize },
}

/// Model-related errors.
#[derive(Debug, Error)]
pub enum ModelError {
    #[error("Invalid model format: {0}")]
    InvalidFormat(String),

    #[error("Model not found: {0}")]
    NotFound(String),

    #[error("Provider not found: {0}")]
    ProviderNotFound(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Rate limited: retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("API error: {0}")]
    Api(String),
}

/// Tool-related errors.
#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Approval denied: {0}")]
    ApprovalDenied(String),

    #[error("Approval timeout")]
    ApprovalTimeout,

    #[error("Not allowed: {0}")]
    NotAllowed(String),

    #[error("Subagent error: {0}")]
    SubagentError(String),

    #[error("Sandbox violation: {0}")]
    SandboxViolation(String),

    #[error("Timeout after {seconds}s")]
    Timeout { seconds: u64 },
}
