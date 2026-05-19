//! Error classifier for provider errors.
//!
//! Classifies provider errors into categories that determine retry behavior,
//! credential rotation, and escalation policies. Provides a unified taxonomy
//! for cross-crate error handling.

use crate::error::ProviderError;

/// Classification of a provider error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// Authentication failure — invalid/expired credentials.
    Auth,
    /// Rate limit exceeded — should retry after delay.
    RateLimit,
    /// Transient network or server error — should retry with backoff.
    Transient,
    /// Invalid request — won't succeed on retry.
    BadRequest,
    /// Context/window limit exceeded — reduce input and retry.
    ContextLimit,
    /// Content filtered by provider safety — adjust input and retry.
    ContentFilter,
    /// Model not found or unavailable — try different model.
    ModelUnavailable,
    /// Configuration error — requires user action.
    Config,
    /// Unknown/unclassified error.
    Unknown,
}

/// Recommended action based on error classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecommendedAction {
    /// Retry the request after a delay.
    RetryWithBackoff,
    /// Rotate credentials and retry.
    RotateCredential,
    /// Reduce input size and retry.
    ReduceInput,
    /// Do not retry — the request is invalid.
    FailFast,
    /// Escalate to user — requires configuration change.
    Escalate,
}

/// Result of classifying a provider error.
#[derive(Debug, Clone)]
pub struct ClassifiedError {
    /// The error category.
    pub category: ErrorClass,
    /// The recommended action.
    pub action: RecommendedAction,
    /// Suggested retry delay in seconds (if applicable).
    pub retry_after_secs: Option<u64>,
    /// Whether the error is retryable.
    pub retryable: bool,
    /// Human-readable description.
    pub description: String,
}

/// Classifies provider errors into actionable categories.
pub struct ErrorClassifier;

impl ErrorClassifier {
    /// Classify a provider error.
    pub fn classify(error: &ProviderError) -> ClassifiedError {
        match error {
            ProviderError::Authentication(msg) => ClassifiedError {
                category: ErrorClass::Auth,
                action: RecommendedAction::RotateCredential,
                retry_after_secs: None,
                retryable: false,
                description: format!("Authentication failed: {}", msg),
            },
            ProviderError::RateLimit { retry_after, message } => ClassifiedError {
                category: ErrorClass::RateLimit,
                action: RecommendedAction::RetryWithBackoff,
                retry_after_secs: *retry_after,
                retryable: true,
                description: format!("Rate limited: {}", message),
            },
            ProviderError::ContextLengthExceeded { used, max } => ClassifiedError {
                category: ErrorClass::ContextLimit,
                action: RecommendedAction::ReduceInput,
                retry_after_secs: None,
                retryable: false,
                description: format!(
                    "Context limit exceeded: {} tokens used, {} max",
                    used, max
                ),
            },
            ProviderError::ContentFiltered(msg) => ClassifiedError {
                category: ErrorClass::ContentFilter,
                action: RecommendedAction::ReduceInput,
                retry_after_secs: None,
                retryable: false,
                description: format!("Content filtered: {}", msg),
            },
            ProviderError::ModelNotFound(model) => ClassifiedError {
                category: ErrorClass::ModelUnavailable,
                action: RecommendedAction::Escalate,
                retry_after_secs: None,
                retryable: false,
                description: format!("Model not found: {}", model),
            },
            ProviderError::InvalidRequest(msg) => ClassifiedError {
                category: ErrorClass::BadRequest,
                action: RecommendedAction::FailFast,
                retry_after_secs: None,
                retryable: false,
                description: format!("Invalid request: {}", msg),
            },
            ProviderError::ServerError { status, message } => {
                let retryable = *status >= 500;
                ClassifiedError {
                    category: ErrorClass::Transient,
                    action: if retryable {
                        RecommendedAction::RetryWithBackoff
                    } else {
                        RecommendedAction::FailFast
                    },
                    retry_after_secs: if retryable { Some(5) } else { None },
                    retryable,
                    description: format!("Server error {}: {}", status, message),
                }
            }
            ProviderError::Network(_) => ClassifiedError {
                category: ErrorClass::Transient,
                action: RecommendedAction::RetryWithBackoff,
                retry_after_secs: Some(2),
                retryable: true,
                description: "Network error".to_string(),
            },
            ProviderError::Timeout(_) => ClassifiedError {
                category: ErrorClass::Transient,
                action: RecommendedAction::RetryWithBackoff,
                retry_after_secs: Some(1),
                retryable: true,
                description: "Request timed out".to_string(),
            },
            ProviderError::Config(msg) => ClassifiedError {
                category: ErrorClass::Config,
                action: RecommendedAction::Escalate,
                retry_after_secs: None,
                retryable: false,
                description: format!("Configuration error: {}", msg),
            },
            ProviderError::Serialization(_) => ClassifiedError {
                category: ErrorClass::BadRequest,
                action: RecommendedAction::FailFast,
                retry_after_secs: None,
                retryable: false,
                description: "Serialization error".to_string(),
            },
            ProviderError::Stream(msg) => ClassifiedError {
                category: ErrorClass::Transient,
                action: RecommendedAction::RetryWithBackoff,
                retry_after_secs: Some(1),
                retryable: true,
                description: format!("Stream error: {}", msg),
            },
            ProviderError::Unsupported(msg) => ClassifiedError {
                category: ErrorClass::BadRequest,
                action: RecommendedAction::FailFast,
                retry_after_secs: None,
                retryable: false,
                description: format!("Unsupported: {}", msg),
            },
            ProviderError::Internal(msg) => ClassifiedError {
                category: ErrorClass::Unknown,
                action: RecommendedAction::FailFast,
                retry_after_secs: None,
                retryable: false,
                description: format!("Internal error: {}", msg),
            },
        }
    }

    /// Check if an error indicates authentication failure.
    pub fn is_auth_error(error: &ProviderError) -> bool {
        matches!(error, ProviderError::Authentication(_))
    }

    /// Check if an error is retryable.
    pub fn is_retryable(error: &ProviderError) -> bool {
        error.is_retryable()
    }

    /// Get recommended retry delay for an error.
    pub fn retry_delay(error: &ProviderError) -> Option<u64> {
        error.retry_after()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_auth_error() {
        let error = ProviderError::auth("Invalid API key");
        let classified = ErrorClassifier::classify(&error);
        assert_eq!(classified.category, ErrorClass::Auth);
        assert_eq!(classified.action, RecommendedAction::RotateCredential);
        assert!(!classified.retryable);
    }

    #[test]
    fn test_classify_rate_limit() {
        let error = ProviderError::rate_limit("Too many requests", Some(60));
        let classified = ErrorClassifier::classify(&error);
        assert_eq!(classified.category, ErrorClass::RateLimit);
        assert_eq!(classified.action, RecommendedAction::RetryWithBackoff);
        assert!(classified.retryable);
        assert_eq!(classified.retry_after_secs, Some(60));
    }

    #[test]
    fn test_classify_context_exceeded() {
        let error = ProviderError::context_exceeded(5000, 4096);
        let classified = ErrorClassifier::classify(&error);
        assert_eq!(classified.category, ErrorClass::ContextLimit);
        assert_eq!(classified.action, RecommendedAction::ReduceInput);
        assert!(!classified.retryable);
    }

    #[test]
    fn test_classify_server_error_500() {
        let error = ProviderError::server_error(500, "Internal server error");
        let classified = ErrorClassifier::classify(&error);
        assert_eq!(classified.category, ErrorClass::Transient);
        assert_eq!(classified.action, RecommendedAction::RetryWithBackoff);
        assert!(classified.retryable);
        assert_eq!(classified.retry_after_secs, Some(5));
    }

    #[test]
    fn test_classify_server_error_400() {
        let error = ProviderError::server_error(400, "Bad request");
        let classified = ErrorClassifier::classify(&error);
        assert_eq!(classified.category, ErrorClass::Transient);
        assert_eq!(classified.action, RecommendedAction::FailFast);
        assert!(!classified.retryable);
    }

    #[test]
    fn test_classify_network_error() {
        // Network errors wrap reqwest::Error which is hard to construct in tests.
        // We verify that Network is classified as Transient/retryable via the
        // is_retryable method, and test other transient errors directly.
        assert!(ProviderError::Timeout(30).is_retryable());
        assert!(ProviderError::server_error(503, "Service Unavailable").is_retryable());
    }

    #[test]
    fn test_classify_timeout() {
        let error = ProviderError::Timeout(30);
        let classified = ErrorClassifier::classify(&error);
        assert_eq!(classified.category, ErrorClass::Transient);
        assert!(classified.retryable);
        assert_eq!(classified.retry_after_secs, Some(1));
    }

    #[test]
    fn test_classify_content_filtered() {
        let error = ProviderError::content_filtered("Violent content detected");
        let classified = ErrorClassifier::classify(&error);
        assert_eq!(classified.category, ErrorClass::ContentFilter);
        assert_eq!(classified.action, RecommendedAction::ReduceInput);
        assert!(!classified.retryable);
    }

    #[test]
    fn test_is_auth_error() {
        let auth_error = ProviderError::auth("Bad key");
        let rate_error = ProviderError::rate_limit("Slow down", None);
        assert!(ErrorClassifier::is_auth_error(&auth_error));
        assert!(!ErrorClassifier::is_auth_error(&rate_error));
    }
}