//! Error types for secret management.

use thiserror::Error;

/// Errors that can occur during secret operations.
#[derive(Debug, Error)]
pub enum SecretError {
    #[error("Secret not found: {0}")]
    NotFound(String),

    #[error("Decryption failed: {0}")]
    DecryptionFailed(String),

    #[error("Encryption failed: {0}")]
    EncryptionFailed(String),

    #[error("Keychain error: {0}")]
    KeychainError(String),

    #[error("Access denied")]
    AccessDenied,

    #[error("Secret expired")]
    Expired,

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Invalid secret name: {0}")]
    InvalidName(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience result alias for secret operations.
pub type Result<T> = std::result::Result<T, SecretError>;
