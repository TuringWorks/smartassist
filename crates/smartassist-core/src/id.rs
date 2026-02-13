//! ID generation utilities.

use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Generate a new UUID v4.
pub fn uuid() -> String {
    Uuid::new_v4().to_string()
}

/// Generate a short random ID (8 characters).
pub fn short_id() -> String {
    let bytes: [u8; 4] = rand::random();
    hex::encode(bytes)
}

/// Generate a slug-safe ID from random bytes.
pub fn slug_id() -> String {
    let bytes: [u8; 6] = rand::random();
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

/// Generate a SHA256 hash of the input.
pub fn sha256(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

/// Generate a short hash (first 16 characters of SHA256).
pub fn short_hash(input: &str) -> String {
    sha256(input)[..16].to_string()
}

/// Normalize an identifier.
///
/// - Converts to lowercase
/// - Replaces spaces and dashes with underscores
/// - Removes non-alphanumeric characters (except underscores)
pub fn normalize(id: &str) -> String {
    id.to_lowercase()
        .replace([' ', '-'], "_")
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '_')
        .collect()
}

/// Check if an ID is valid (alphanumeric + underscores only).
pub fn is_valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Generate a timestamp-based ID (useful for sorting).
pub fn timestamp_id() -> String {
    use chrono::Utc;
    let now = Utc::now();
    let ts = now.format("%Y%m%d%H%M%S%3f").to_string();
    let random: [u8; 4] = rand::random();
    format!("{}-{}", ts, hex::encode(random))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid() {
        let id = uuid();
        assert_eq!(id.len(), 36);
        assert!(id.contains('-'));
    }

    #[test]
    fn test_short_id() {
        let id = short_id();
        assert_eq!(id.len(), 8);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("My Agent"), "my_agent");
        assert_eq!(normalize("test-agent"), "test_agent");
        assert_eq!(normalize("UPPER_CASE"), "upper_case");
        assert_eq!(normalize("special!@#chars"), "specialchars");
    }

    #[test]
    fn test_is_valid_id() {
        assert!(is_valid_id("valid_id"));
        assert!(is_valid_id("valid123"));
        assert!(is_valid_id("Valid_ID_123"));
        assert!(!is_valid_id(""));
        assert!(!is_valid_id("invalid-id"));
        assert!(!is_valid_id("invalid id"));
    }

    #[test]
    fn test_sha256() {
        let hash = sha256("hello");
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_timestamp_id() {
        let id1 = timestamp_id();
        let id2 = timestamp_id();
        assert_ne!(id1, id2);
        assert!(id1 < id2 || id1.len() == id2.len());
    }
}
