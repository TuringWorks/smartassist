//! Secure string handling with memory protection.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// A string that is zeroed on drop for secure credential handling.
///
/// This type ensures that sensitive data like API keys and tokens
/// are cleared from memory when no longer needed.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretString {
    inner: String,
}

impl SecretString {
    /// Create a new secret string.
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            inner: value.into(),
        }
    }

    /// Expose the secret value.
    ///
    /// Use sparingly - only when the actual value is needed.
    pub fn expose_secret(&self) -> &str {
        &self.inner
    }

    /// Check if the secret is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Get the length of the secret.
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Default for SecretString {
    fn default() -> Self {
        Self {
            inner: String::new(),
        }
    }
}

// Never print secrets
impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("[REDACTED]")
    }
}

impl PartialEq for SecretString {
    fn eq(&self, other: &Self) -> bool {
        // Use constant-time comparison for security
        constant_time_eq(self.inner.as_bytes(), other.inner.as_bytes())
    }
}

impl Eq for SecretString {}

impl<'de> Deserialize<'de> for SecretString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Self::new(s))
    }
}

impl Serialize for SecretString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize as the actual value (for config files)
        self.inner.serialize(serializer)
    }
}

impl From<String> for SecretString {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

impl From<&str> for SecretString {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

/// Constant-time byte comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secret_string_redacted() {
        let secret = SecretString::new("my-api-key");
        assert_eq!(format!("{:?}", secret), "[REDACTED]");
        assert_eq!(format!("{}", secret), "[REDACTED]");
    }

    #[test]
    fn test_secret_string_expose() {
        let secret = SecretString::new("my-api-key");
        assert_eq!(secret.expose_secret(), "my-api-key");
    }

    #[test]
    fn test_secret_string_equality() {
        let a = SecretString::new("secret");
        let b = SecretString::new("secret");
        let c = SecretString::new("different");

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hell"));
    }
}
