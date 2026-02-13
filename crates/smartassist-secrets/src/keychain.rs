//! OS keychain integration for master key storage.
//!
//! The master key is resolved in priority order:
//! 1. `SMARTASSIST_MASTER_KEY` environment variable (hex-encoded)
//! 2. OS keychain (macOS Keychain via Security.framework)
//! 3. Generate a new key and store it in the keychain
//!
//! On Linux and other platforms the keychain path is not yet implemented;
//! only the environment variable fallback is used.

use crate::crypto;
use crate::error::{Result, SecretError};
use tracing::debug;
#[cfg(not(target_os = "macos"))]
use tracing::warn;

const SERVICE_NAME: &str = "smartassist";
const ACCOUNT_NAME: &str = "master_key";

/// Environment variable name for the master key (hex-encoded).
const ENV_VAR: &str = "SMARTASSIST_MASTER_KEY";

/// Retrieve the master key, creating one if it does not exist yet.
///
/// Resolution order:
/// 1. `SMARTASSIST_MASTER_KEY` env var (hex-encoded 32 bytes)
/// 2. OS keychain lookup
/// 3. Generate + persist to keychain
pub fn get_or_create_master_key() -> Result<Vec<u8>> {
    // 1. Try environment variable first.
    if let Ok(hex_key) = std::env::var(ENV_VAR) {
        debug!("using master key from environment variable");
        let key = hex::decode(hex_key.trim()).map_err(|e| {
            SecretError::KeychainError(format!("invalid hex in {ENV_VAR}: {e}"))
        })?;
        if key.len() != 32 {
            return Err(SecretError::KeychainError(format!(
                "{ENV_VAR} must decode to exactly 32 bytes, got {}",
                key.len()
            )));
        }
        return Ok(key);
    }

    // 2. Try OS keychain.
    if let Some(key) = get_from_keychain()? {
        debug!("using master key from OS keychain");
        return Ok(key);
    }

    // 3. Generate a new key and store it.
    debug!("generating new master key and storing in keychain");
    let key = crypto::generate_master_key();
    store_in_keychain(&key)?;
    Ok(key)
}

/// Delete the master key from the OS keychain (for reset workflows).
pub fn delete_master_key() -> Result<()> {
    delete_from_keychain()
}

// ---------------------------------------------------------------------------
// macOS keychain implementation
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
fn get_from_keychain() -> Result<Option<Vec<u8>>> {
    use security_framework::passwords::get_generic_password;

    match get_generic_password(SERVICE_NAME, ACCOUNT_NAME) {
        Ok(data) => {
            // The key is stored as a hex string in the keychain.
            let hex_str = String::from_utf8(data.to_vec()).map_err(|e| {
                SecretError::KeychainError(format!("keychain data is not valid UTF-8: {e}"))
            })?;
            let key = hex::decode(hex_str.trim()).map_err(|e| {
                SecretError::KeychainError(format!("keychain data is not valid hex: {e}"))
            })?;
            if key.len() != 32 {
                return Err(SecretError::KeychainError(format!(
                    "keychain key has wrong length: {} (expected 32)",
                    key.len()
                )));
            }
            Ok(Some(key))
        }
        Err(e) => {
            // errSecItemNotFound is the expected "not stored yet" case.
            let msg = e.to_string();
            if msg.contains("not found") || msg.contains("-25300") {
                Ok(None)
            } else {
                Err(SecretError::KeychainError(format!(
                    "keychain read failed: {e}"
                )))
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn store_in_keychain(key: &[u8]) -> Result<()> {
    use security_framework::passwords::set_generic_password;

    let hex_key = hex::encode(key);
    set_generic_password(SERVICE_NAME, ACCOUNT_NAME, hex_key.as_bytes()).map_err(|e| {
        SecretError::KeychainError(format!("keychain write failed: {e}"))
    })
}

#[cfg(target_os = "macos")]
fn delete_from_keychain() -> Result<()> {
    use security_framework::passwords::delete_generic_password;

    match delete_generic_password(SERVICE_NAME, ACCOUNT_NAME) {
        Ok(()) => Ok(()),
        Err(e) => {
            let msg = e.to_string();
            // Treat "not found" as success -- nothing to delete.
            if msg.contains("not found") || msg.contains("-25300") {
                Ok(())
            } else {
                Err(SecretError::KeychainError(format!(
                    "keychain delete failed: {e}"
                )))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Linux stub -- env-var-only for now
// ---------------------------------------------------------------------------

#[cfg(target_os = "linux")]
fn get_from_keychain() -> Result<Option<Vec<u8>>> {
    // TODO: Implement secret-service (D-Bus) integration for Linux desktops.
    // For now only the SMARTASSIST_MASTER_KEY env var is supported on Linux.
    warn!("OS keychain not implemented on Linux; use {ENV_VAR} env var");
    Ok(None)
}

#[cfg(target_os = "linux")]
fn store_in_keychain(key: &[u8]) -> Result<()> {
    warn!(
        "OS keychain not implemented on Linux; master key cannot be persisted. \
         Set {ENV_VAR}={} to reuse this key.",
        hex::encode(key)
    );
    Ok(())
}

#[cfg(target_os = "linux")]
fn delete_from_keychain() -> Result<()> {
    // Nothing stored, nothing to delete.
    Ok(())
}

// ---------------------------------------------------------------------------
// Fallback for other platforms
// ---------------------------------------------------------------------------

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn get_from_keychain() -> Result<Option<Vec<u8>>> {
    warn!("OS keychain not available on this platform; use {ENV_VAR} env var");
    Ok(None)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn store_in_keychain(key: &[u8]) -> Result<()> {
    warn!(
        "OS keychain not available on this platform; master key cannot be persisted. \
         Set {ENV_VAR}={} to reuse this key.",
        hex::encode(key)
    );
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn delete_from_keychain() -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test the env-var path, which works on all platforms (including CI).
    #[test]
    fn test_master_key_from_env_var() {
        let key = crypto::generate_master_key();
        let hex_key = hex::encode(&key);

        // Temporarily set the env var for this test.
        std::env::set_var(ENV_VAR, &hex_key);
        let result = get_or_create_master_key().unwrap();
        assert_eq!(result, key);

        // Clean up.
        std::env::remove_var(ENV_VAR);
    }

    #[test]
    fn test_invalid_hex_in_env_var() {
        std::env::set_var(ENV_VAR, "not-valid-hex!");
        let result = get_or_create_master_key();
        assert!(result.is_err());
        std::env::remove_var(ENV_VAR);
    }

    #[test]
    fn test_wrong_length_key_in_env_var() {
        // 16 bytes instead of 32.
        std::env::set_var(ENV_VAR, hex::encode([0u8; 16]));
        let result = get_or_create_master_key();
        assert!(result.is_err());
        std::env::remove_var(ENV_VAR);
    }
}
