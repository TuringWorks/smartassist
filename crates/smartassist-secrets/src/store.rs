//! Secret storage backends.
//!
//! Defines the [`SecretStore`] trait and provides [`FileSecretStore`], a
//! file-system-backed implementation that encrypts each secret as a JSON file
//! under `~/.smartassist/secrets/`.

use std::path::PathBuf;

use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::crypto;
use crate::error::{Result, SecretError};
use crate::types::{DecryptedSecret, SecretRef};

/// Maximum allowed length for a secret name.
const MAX_NAME_LEN: usize = 128;

/// Async trait for secret storage backends.
#[async_trait]
pub trait SecretStore: Send + Sync {
    /// Store a secret under the given name, encrypting the value.
    async fn set(&self, name: &str, value: &str) -> Result<()>;

    /// Retrieve and decrypt a secret by name.
    async fn get(&self, name: &str) -> Result<DecryptedSecret>;

    /// Check whether a secret with the given name exists.
    async fn exists(&self, name: &str) -> Result<bool>;

    /// List all stored secrets (metadata only, no plaintext).
    async fn list(&self) -> Result<Vec<SecretRef>>;

    /// Delete a secret by name.
    async fn delete(&self, name: &str) -> Result<()>;
}

/// On-disk representation of an encrypted secret.
#[derive(Debug, Serialize, Deserialize)]
struct StoredSecret {
    /// AES-256-GCM encrypted value, base64-encoded.
    encrypted_value: String,
    /// HKDF salt, hex-encoded.
    salt: String,
    /// When the secret was first created.
    created_at: chrono::DateTime<Utc>,
    /// When the secret was last updated.
    updated_at: chrono::DateTime<Utc>,
    /// Number of times the secret has been read.
    usage_count: u64,
}

/// A file-system-backed secret store.
///
/// Each secret is stored as an individual JSON file at
/// `{base_dir}/{name}.json`. Files are created with mode `0600` on Unix.
pub struct FileSecretStore {
    base_dir: PathBuf,
    master_key: Vec<u8>,
}

impl FileSecretStore {
    /// Create a new store rooted at `base_dir` using the provided master key.
    pub fn new(base_dir: PathBuf, master_key: Vec<u8>) -> Self {
        Self {
            base_dir,
            master_key,
        }
    }

    /// Create a store using the default directory (`~/.smartassist/secrets/`) and
    /// the master key resolved via [`crate::keychain::get_or_create_master_key`].
    pub fn from_default_dir() -> Result<Self> {
        let base_dir = smartassist_core::paths::base_dir()
            .map_err(|e| SecretError::StorageError(e.to_string()))?
            .join("secrets");
        let master_key = crate::keychain::get_or_create_master_key()?;
        Ok(Self::new(base_dir, master_key))
    }

    /// Ensure the base directory exists with restrictive permissions.
    async fn ensure_dir(&self) -> Result<()> {
        tokio::fs::create_dir_all(&self.base_dir).await?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o700);
            tokio::fs::set_permissions(&self.base_dir, perms).await?;
        }

        Ok(())
    }

    /// Resolve the path for a secret file.
    fn secret_path(&self, name: &str) -> PathBuf {
        self.base_dir.join(format!("{name}.json"))
    }
}

/// Validate that a secret name contains only safe characters.
///
/// Allowed: ASCII alphanumeric, underscore, hyphen. Max length 128.
fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(SecretError::InvalidName(
            "name must not be empty".to_string(),
        ));
    }
    if name.len() > MAX_NAME_LEN {
        return Err(SecretError::InvalidName(format!(
            "name exceeds maximum length of {MAX_NAME_LEN} characters"
        )));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(SecretError::InvalidName(format!(
            "name contains invalid characters (allowed: alphanumeric, underscore, hyphen): {name}"
        )));
    }
    Ok(())
}

/// Write `data` to `path` with mode 0600 on Unix.
async fn write_secret_file(path: &std::path::Path, data: &[u8]) -> Result<()> {
    tokio::fs::write(path, data).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        tokio::fs::set_permissions(path, perms).await?;
    }

    Ok(())
}

#[async_trait]
impl SecretStore for FileSecretStore {
    async fn set(&self, name: &str, value: &str) -> Result<()> {
        validate_name(name)?;
        self.ensure_dir().await?;

        let (encrypted, salt) = crypto::encrypt(&self.master_key, value.as_bytes())?;
        let now = Utc::now();

        let stored = StoredSecret {
            encrypted_value: base64::Engine::encode(
                &base64::engine::general_purpose::STANDARD,
                &encrypted,
            ),
            salt: hex::encode(&salt),
            created_at: now,
            updated_at: now,
            usage_count: 0,
        };

        let json = serde_json::to_string_pretty(&stored)?;
        let path = self.secret_path(name);
        debug!(name, path = %path.display(), "writing secret");
        write_secret_file(&path, json.as_bytes()).await?;
        Ok(())
    }

    async fn get(&self, name: &str) -> Result<DecryptedSecret> {
        validate_name(name)?;

        let path = self.secret_path(name);
        if !path.exists() {
            return Err(SecretError::NotFound(name.to_string()));
        }

        let data = tokio::fs::read_to_string(&path).await?;
        let mut stored: StoredSecret = serde_json::from_str(&data)?;

        let encrypted = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &stored.encrypted_value,
        )
        .map_err(|e| SecretError::DecryptionFailed(format!("base64 decode failed: {e}")))?;
        let salt = hex::decode(&stored.salt)
            .map_err(|e| SecretError::DecryptionFailed(format!("hex decode failed: {e}")))?;

        let plaintext = crypto::decrypt(&self.master_key, &encrypted, &salt)?;
        let value = String::from_utf8(plaintext)
            .map_err(|e| SecretError::DecryptionFailed(format!("invalid UTF-8: {e}")))?;

        // Update usage count and persist.
        stored.usage_count += 1;
        let json = serde_json::to_string_pretty(&stored)?;
        write_secret_file(&path, json.as_bytes()).await?;

        debug!(name, "read secret (usage_count={})", stored.usage_count);
        Ok(DecryptedSecret::new(value))
    }

    async fn exists(&self, name: &str) -> Result<bool> {
        validate_name(name)?;
        Ok(self.secret_path(name).exists())
    }

    async fn list(&self) -> Result<Vec<SecretRef>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut refs = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.base_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();

            // Read minimal metadata from the file.
            match tokio::fs::read_to_string(&path).await {
                Ok(data) => match serde_json::from_str::<StoredSecret>(&data) {
                    Ok(stored) => {
                        refs.push(SecretRef {
                            name,
                            provider: None,
                            created_at: stored.created_at,
                        });
                    }
                    Err(e) => {
                        tracing::warn!(path = %path.display(), "skipping malformed secret file: {e}");
                    }
                },
                Err(e) => {
                    tracing::warn!(path = %path.display(), "could not read secret file: {e}");
                }
            }
        }

        refs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(refs)
    }

    async fn delete(&self, name: &str) -> Result<()> {
        validate_name(name)?;

        let path = self.secret_path(name);
        if !path.exists() {
            return Err(SecretError::NotFound(name.to_string()));
        }

        debug!(name, path = %path.display(), "deleting secret");
        tokio::fs::remove_file(&path).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn test_store() -> (FileSecretStore, TempDir) {
        let tmp = TempDir::new().unwrap();
        let master_key = crypto::generate_master_key();
        let store = FileSecretStore::new(tmp.path().to_path_buf(), master_key);
        (store, tmp)
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let (store, _tmp) = test_store();
        store.set("api_key", "sk-abc123").await.unwrap();

        let secret = store.get("api_key").await.unwrap();
        assert_eq!(secret.expose(), "sk-abc123");
    }

    #[tokio::test]
    async fn test_exists() {
        let (store, _tmp) = test_store();
        assert!(!store.exists("missing").await.unwrap());

        store.set("present", "value").await.unwrap();
        assert!(store.exists("present").await.unwrap());
    }

    #[tokio::test]
    async fn test_list() {
        let (store, _tmp) = test_store();
        store.set("alpha", "a").await.unwrap();
        store.set("beta", "b").await.unwrap();

        let refs = store.list().await.unwrap();
        let names: Vec<&str> = refs.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn test_delete() {
        let (store, _tmp) = test_store();
        store.set("to_delete", "value").await.unwrap();
        assert!(store.exists("to_delete").await.unwrap());

        store.delete("to_delete").await.unwrap();
        assert!(!store.exists("to_delete").await.unwrap());
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let (store, _tmp) = test_store();
        let result = store.delete("nonexistent").await;
        assert!(matches!(result, Err(SecretError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_get_not_found() {
        let (store, _tmp) = test_store();
        let result = store.get("nonexistent").await;
        assert!(matches!(result, Err(SecretError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_overwrite_secret() {
        let (store, _tmp) = test_store();
        store.set("key", "old_value").await.unwrap();
        store.set("key", "new_value").await.unwrap();

        let secret = store.get("key").await.unwrap();
        assert_eq!(secret.expose(), "new_value");
    }

    #[tokio::test]
    async fn test_usage_count_increments() {
        let (store, _tmp) = test_store();
        store.set("counted", "value").await.unwrap();

        // Read twice to bump usage_count.
        let _ = store.get("counted").await.unwrap();
        let _ = store.get("counted").await.unwrap();

        // Verify by reading the raw file.
        let path = store.secret_path("counted");
        let data = tokio::fs::read_to_string(&path).await.unwrap();
        let stored: serde_json::Value = serde_json::from_str(&data).unwrap();
        assert_eq!(stored["usage_count"], 2);
    }

    #[test]
    fn test_validate_name_valid() {
        assert!(validate_name("api_key").is_ok());
        assert!(validate_name("my-secret-1").is_ok());
        assert!(validate_name("ABC123").is_ok());
    }

    #[test]
    fn test_validate_name_empty() {
        assert!(matches!(
            validate_name(""),
            Err(SecretError::InvalidName(_))
        ));
    }

    #[test]
    fn test_validate_name_too_long() {
        let long = "a".repeat(MAX_NAME_LEN + 1);
        assert!(matches!(
            validate_name(&long),
            Err(SecretError::InvalidName(_))
        ));
    }

    #[test]
    fn test_validate_name_invalid_chars() {
        assert!(matches!(
            validate_name("has spaces"),
            Err(SecretError::InvalidName(_))
        ));
        assert!(matches!(
            validate_name("path/traversal"),
            Err(SecretError::InvalidName(_))
        ));
        assert!(matches!(
            validate_name("dots.bad"),
            Err(SecretError::InvalidName(_))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let (store, _tmp) = test_store();
        store.set("perm_test", "value").await.unwrap();

        let path = store.secret_path("perm_test");
        let metadata = tokio::fs::metadata(&path).await.unwrap();
        let mode = metadata.permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "secret file should have 0600 permissions");
    }
}
