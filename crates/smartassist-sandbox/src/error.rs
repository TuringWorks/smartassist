//! Sandbox error types.

use std::io;
use thiserror::Error;

/// Errors that can occur during sandbox operations.
#[derive(Debug, Error)]
pub enum SandboxError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Command execution failed.
    #[error("Command execution failed: {0}")]
    ExecutionFailed(String),

    /// Command timed out.
    #[error("Command timed out after {0} seconds")]
    Timeout(u64),

    /// Command was killed by signal.
    #[error("Command killed by signal {0}")]
    Signal(i32),

    /// Sandbox setup failed.
    #[error("Sandbox setup failed: {0}")]
    SetupFailed(String),

    /// Invalid sandbox profile.
    #[error("Invalid sandbox profile: {0}")]
    InvalidProfile(String),

    /// Resource limit exceeded.
    #[error("Resource limit exceeded: {0}")]
    ResourceLimitExceeded(String),

    /// Permission denied.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// PTY error.
    #[error("PTY error: {0}")]
    Pty(String),

    /// Seccomp error (Linux).
    #[error("Seccomp error: {0}")]
    Seccomp(String),

    /// Landlock error (Linux).
    #[error("Landlock error: {0}")]
    Landlock(String),

    /// Namespace error (Linux).
    #[error("Namespace error: {0}")]
    Namespace(String),

    /// Capability error (Linux).
    #[error("Capability error: {0}")]
    Capability(String),

    /// Unsupported platform.
    #[error("Sandbox feature not supported on this platform")]
    UnsupportedPlatform,

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),
}

impl SandboxError {
    /// Create a new execution failed error.
    pub fn execution_failed(msg: impl Into<String>) -> Self {
        Self::ExecutionFailed(msg.into())
    }

    /// Create a new setup failed error.
    pub fn setup_failed(msg: impl Into<String>) -> Self {
        Self::SetupFailed(msg.into())
    }

    /// Create a new PTY error.
    pub fn pty(msg: impl Into<String>) -> Self {
        Self::Pty(msg.into())
    }

    /// Check if this error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(self, Self::Timeout(_) | Self::ResourceLimitExceeded(_))
    }
}
