//! Plugin SDK error types.

use thiserror::Error;

/// Plugin SDK error type.
#[derive(Error, Debug)]
pub enum PluginError {
    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Initialization error.
    #[error("Initialization error: {0}")]
    Initialization(String),

    /// Runtime error.
    #[error("Runtime error: {0}")]
    Runtime(String),

    /// Plugin not found.
    #[error("Plugin not found: {0}")]
    NotFound(String),

    /// Plugin already registered.
    #[error("Plugin already registered: {0}")]
    AlreadyRegistered(String),

    /// Incompatible version.
    #[error("Incompatible version: {0}")]
    IncompatibleVersion(String),

    /// Hook error.
    #[error("Hook error: {0}")]
    Hook(String),

    /// Channel error.
    #[error("Channel error: {0}")]
    Channel(#[from] smartassist_channels::ChannelError),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Other error.
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

impl PluginError {
    /// Create a configuration error.
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    /// Create an initialization error.
    pub fn initialization(msg: impl Into<String>) -> Self {
        Self::Initialization(msg.into())
    }

    /// Create a runtime error.
    pub fn runtime(msg: impl Into<String>) -> Self {
        Self::Runtime(msg.into())
    }

    /// Create a hook error.
    pub fn hook(msg: impl Into<String>) -> Self {
        Self::Hook(msg.into())
    }
}

/// Result type for plugin operations.
pub type Result<T> = std::result::Result<T, PluginError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PluginError::config("Missing API key");
        assert_eq!(err.to_string(), "Configuration error: Missing API key");

        let err = PluginError::NotFound("my-plugin".to_string());
        assert_eq!(err.to_string(), "Plugin not found: my-plugin");
    }
}
