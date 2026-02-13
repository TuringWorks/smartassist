//! Channel error types.

use std::io;
use thiserror::Error;

/// Errors that can occur during channel operations.
#[derive(Debug, Error)]
pub enum ChannelError {
    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// HTTP request error.
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Channel not found.
    #[error("Channel not found: {0}")]
    NotFound(String),

    /// Channel not connected.
    #[error("Channel not connected: {0}")]
    NotConnected(String),

    /// Channel already exists.
    #[error("Channel already exists: {0}")]
    AlreadyExists(String),

    /// Authentication error.
    #[error("Authentication failed: {0}")]
    Auth(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: retry after {retry_after_secs} seconds")]
    RateLimit {
        /// Seconds to wait before retrying.
        retry_after_secs: u64,
    },

    /// Message too large.
    #[error("Message too large: {size} bytes (max: {max} bytes)")]
    MessageTooLarge {
        /// Actual message size.
        size: usize,
        /// Maximum allowed size.
        max: usize,
    },

    /// Invalid message format.
    #[error("Invalid message format: {0}")]
    InvalidMessage(String),

    /// Attachment error.
    #[error("Attachment error: {0}")]
    Attachment(String),

    /// Routing error.
    #[error("Routing error: {0}")]
    Routing(String),

    /// Delivery error.
    #[error("Delivery failed: {0}")]
    Delivery(String),

    /// Channel configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Permission denied.
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Channel-specific error.
    #[error("Channel error ({channel}): {message}")]
    Channel {
        /// Channel type.
        channel: String,
        /// Error message.
        message: String,
    },

    /// Timeout error.
    #[error("Operation timed out")]
    Timeout,

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ChannelError {
    /// Create a channel-specific error.
    pub fn channel(channel: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Channel {
            channel: channel.into(),
            message: message.into(),
        }
    }

    /// Create a not found error.
    pub fn not_found(name: impl Into<String>) -> Self {
        Self::NotFound(name.into())
    }

    /// Create a not connected error.
    pub fn not_connected(name: impl Into<String>) -> Self {
        Self::NotConnected(name.into())
    }

    /// Create an auth error.
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth(message.into())
    }

    /// Create a rate limit error.
    pub fn rate_limit(retry_after_secs: u64) -> Self {
        Self::RateLimit { retry_after_secs }
    }

    /// Check if this error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            Self::RateLimit { .. } | Self::Timeout | Self::Io(_)
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
