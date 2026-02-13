//! Core types for secret management.
//!
//! Provides the main data structures used to represent secrets in both
//! encrypted (at-rest) and decrypted (in-memory) forms.

use chrono::{DateTime, Utc};
use smartassist_core::SecretString;
use serde::{Deserialize, Serialize};
use std::fmt;

/// An encrypted secret as stored on disk.
///
/// The `encrypted_value` contains the AES-256-GCM ciphertext (base64-encoded)
/// and `salt` holds the HKDF salt used for key derivation (hex-encoded).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Secret {
    /// AES-256-GCM encrypted value, stored as a base64 string.
    pub encrypted_value: String,

    /// HKDF salt used for key derivation, stored as a hex string.
    pub salt: String,

    /// Timestamp when the secret was first created.
    pub created_at: DateTime<Utc>,

    /// Timestamp of the most recent update.
    pub updated_at: DateTime<Utc>,

    /// Number of times the secret has been read/decrypted.
    pub usage_count: u64,
}

/// A decrypted secret held in memory.
///
/// Wraps `SecretString` so the plaintext is zeroed on drop. Debug and Display
/// both emit `[REDACTED]` to prevent accidental logging.
pub struct DecryptedSecret {
    inner: SecretString,
}

impl DecryptedSecret {
    /// Create a new decrypted secret from raw plaintext.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            inner: SecretString::new(value),
        }
    }

    /// Expose the plaintext value. Use sparingly.
    pub fn expose(&self) -> &str {
        self.inner.expose_secret()
    }
}

impl fmt::Debug for DecryptedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for DecryptedSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl From<String> for DecryptedSecret {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// A lightweight reference to a stored secret.
///
/// Contains only metadata -- no plaintext or ciphertext -- so it is safe to
/// pass around, log, or serialize without leaking sensitive data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRef {
    /// Name / identifier of the secret.
    pub name: String,

    /// Optional provider the secret is associated with (e.g. "openai").
    pub provider: Option<String>,

    /// Timestamp when the secret was first created.
    pub created_at: DateTime<Utc>,
}

/// Parameters for creating a new secret.
pub struct CreateSecretParams {
    /// Name / identifier for the secret (must be alphanumeric + underscore/hyphen).
    pub name: String,

    /// Plaintext value to encrypt and store.
    pub value: String,

    /// Optional provider association.
    pub provider: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decrypted_secret_redacted_debug() {
        let secret = DecryptedSecret::new("super-secret");
        assert_eq!(format!("{:?}", secret), "[REDACTED]");
    }

    #[test]
    fn test_decrypted_secret_redacted_display() {
        let secret = DecryptedSecret::new("super-secret");
        assert_eq!(format!("{}", secret), "[REDACTED]");
    }

    #[test]
    fn test_decrypted_secret_expose() {
        let secret = DecryptedSecret::new("super-secret");
        assert_eq!(secret.expose(), "super-secret");
    }

    #[test]
    fn test_decrypted_secret_from_string() {
        let secret: DecryptedSecret = "my-key".to_string().into();
        assert_eq!(secret.expose(), "my-key");
    }

    #[test]
    fn test_secret_ref_serialization() {
        let secret_ref = SecretRef {
            name: "api_key".to_string(),
            provider: Some("openai".to_string()),
            created_at: Utc::now(),
        };
        let json = serde_json::to_string(&secret_ref).unwrap();
        let parsed: SecretRef = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "api_key");
        assert_eq!(parsed.provider, Some("openai".to_string()));
    }
}
