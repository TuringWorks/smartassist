//! Error types for model providers.

use thiserror::Error;

/// Result type for provider operations.
pub type Result<T> = std::result::Result<T, ProviderError>;

/// Provider error types.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Authentication error (invalid API key, etc.).
    #[error("Authentication error: {0}")]
    Authentication(String),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded: {message}. Retry after {retry_after:?} seconds")]
    RateLimit {
        message: String,
        retry_after: Option<u64>,
    },

    /// Invalid request (bad parameters, etc.).
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    /// Model not found or not available.
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    /// Context length exceeded.
    #[error("Context length exceeded: {used} tokens used, {max} maximum")]
    ContextLengthExceeded { used: usize, max: usize },

    /// Content filtered (safety filters triggered).
    #[error("Content filtered: {0}")]
    ContentFiltered(String),

    /// Server error from the provider.
    #[error("Server error: {status} - {message}")]
    ServerError { status: u16, message: String },

    /// Network error.
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    /// Timeout error.
    #[error("Request timed out after {0} seconds")]
    Timeout(u64),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Stream error.
    #[error("Stream error: {0}")]
    Stream(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Unsupported operation.
    #[error("Unsupported operation: {0}")]
    Unsupported(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ProviderError {
    /// Create an authentication error.
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Authentication(message.into())
    }

    /// Create a rate limit error.
    pub fn rate_limit(message: impl Into<String>, retry_after: Option<u64>) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after,
        }
    }

    /// Create an invalid request error.
    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::InvalidRequest(message.into())
    }

    /// Create a model not found error.
    pub fn model_not_found(model: impl Into<String>) -> Self {
        Self::ModelNotFound(model.into())
    }

    /// Create a context length exceeded error.
    pub fn context_exceeded(used: usize, max: usize) -> Self {
        Self::ContextLengthExceeded { used, max }
    }

    /// Create a content filtered error.
    pub fn content_filtered(message: impl Into<String>) -> Self {
        Self::ContentFiltered(message.into())
    }

    /// Create a server error.
    pub fn server_error(status: u16, message: impl Into<String>) -> Self {
        Self::ServerError {
            status,
            message: message.into(),
        }
    }

    /// Create a stream error.
    pub fn stream(message: impl Into<String>) -> Self {
        Self::Stream(message.into())
    }

    /// Create a config error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config(message.into())
    }

    /// Create an unsupported operation error.
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported(message.into())
    }

    /// Create an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }

    /// Check if this error is retryable.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimit { .. } | Self::Timeout(_) | Self::Network(_) => true,
            Self::ServerError { status, .. } => *status >= 500,
            _ => false,
        }
    }

    /// Get retry delay if applicable.
    pub fn retry_after(&self) -> Option<u64> {
        match self {
            Self::RateLimit { retry_after, .. } => *retry_after,
            Self::Timeout(_) => Some(1),
            Self::ServerError { status, .. } if *status >= 500 => Some(5),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = ProviderError::auth("Invalid API key");
        assert!(matches!(err, ProviderError::Authentication(_)));

        let err = ProviderError::rate_limit("Too many requests", Some(60));
        assert!(matches!(err, ProviderError::RateLimit { .. }));
        assert!(err.is_retryable());
        assert_eq!(err.retry_after(), Some(60));
    }

    #[test]
    fn test_retryable() {
        assert!(ProviderError::rate_limit("", None).is_retryable());
        assert!(ProviderError::Timeout(30).is_retryable());
        assert!(ProviderError::server_error(500, "").is_retryable());
        assert!(ProviderError::server_error(503, "").is_retryable());

        assert!(!ProviderError::auth("").is_retryable());
        assert!(!ProviderError::invalid_request("").is_retryable());
        assert!(!ProviderError::server_error(400, "").is_retryable());
    }
}
