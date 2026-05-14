//! Error types for the talk system.

use thiserror::Error;

/// Result type for talk operations.
pub type Result<T> = std::result::Result<T, TalkError>;

/// Talk system error types.
#[derive(Debug, Error)]
pub enum TalkError {
    /// Session not found.
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Session already exists.
    #[error("Session already exists: {0}")]
    SessionAlreadyExists(String),

    /// Audio pipeline error.
    #[error("Audio pipeline error: {0}")]
    AudioPipeline(String),

    /// Transcription error.
    #[error("Transcription error: {0}")]
    Transcription(String),

    /// Codec error.
    #[error("Codec error: {0}")]
    Codec(String),

    /// Device not available.
    #[error("Audio device not available: {0}")]
    DeviceNotAvailable(String),

    /// Unsupported operation.
    #[error("Unsupported operation: {0}")]
    Unsupported(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl TalkError {
    /// Create a session not found error.
    pub fn session_not_found(id: impl Into<String>) -> Self {
        Self::SessionNotFound(id.into())
    }

    /// Create an audio pipeline error.
    pub fn audio_pipeline(msg: impl Into<String>) -> Self {
        Self::AudioPipeline(msg.into())
    }

    /// Create a transcription error.
    pub fn transcription(msg: impl Into<String>) -> Self {
        Self::Transcription(msg.into())
    }

    /// Create a codec error.
    pub fn codec(msg: impl Into<String>) -> Self {
        Self::Codec(msg.into())
    }

    /// Create a device not available error.
    pub fn device_not_available(name: impl Into<String>) -> Self {
        Self::DeviceNotAvailable(name.into())
    }

    /// Create an unsupported operation error.
    pub fn unsupported(msg: impl Into<String>) -> Self {
        Self::Unsupported(msg.into())
    }

    /// Create a config error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create an internal error.
    pub fn internal(msg: impl Into<String>) -> Self {
        Self::Internal(msg.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = TalkError::session_not_found("sess-123");
        assert!(matches!(err, TalkError::SessionNotFound(_)));
        assert!(err.to_string().contains("sess-123"));

        let err = TalkError::audio_pipeline("buffer underrun");
        assert!(matches!(err, TalkError::AudioPipeline(_)));

        let err = TalkError::unsupported("stereo capture");
        assert!(matches!(err, TalkError::Unsupported(_)));
    }
}
