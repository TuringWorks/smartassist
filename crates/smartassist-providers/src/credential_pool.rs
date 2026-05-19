//! Credential pool for managing multiple API keys per provider.
//!
//! Supports round-robin key selection, automatic rotation on auth failure,
//! and key-level health tracking. When one key fails authentication, it's
//! temporarily disabled and the next key is tried.

use secrecy::SecretString;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Health status of a single API key.
#[derive(Debug, Clone)]
pub enum KeyHealth {
    /// Key is healthy and available for use.
    Healthy,
    /// Key is temporarily disabled due to an error.
    Disabled {
        /// When the key was disabled.
        since: Instant,
        /// Reason for disabling.
        reason: String,
        /// How long to wait before retrying.
        retry_after: Option<Duration>,
    },
    /// Key has been permanently revoked.
    Revoked { reason: String },
}

/// A single credential entry in the pool.
#[derive(Debug)]
pub struct CredentialEntry {
    /// The API key (secret).
    key: SecretString,
    /// Health status of this key.
    health: KeyHealth,
    /// Number of successful uses.
    success_count: u64,
    /// Number of failed uses.
    failure_count: u64,
    /// Last time this key was used.
    last_used: Option<Instant>,
}

/// A credential pool that manages multiple API keys for a single provider.
///
/// Keys are selected in round-robin order, skipping disabled or revoked keys.
/// When an auth failure occurs, the offending key is temporarily disabled
/// and the pool rotates to the next available key.
pub struct CredentialPool {
    /// Provider name this pool is for.
    provider: String,
    /// Credential entries.
    entries: Arc<RwLock<Vec<CredentialEntry>>>,
    /// Current round-robin index.
    current_index: Arc<RwLock<usize>>,
}

impl std::fmt::Debug for CredentialPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CredentialPool")
            .field("provider", &self.provider)
            .field("current_index", &self.current_index)
            .finish_non_exhaustive()
    }
}

impl CredentialPool {
    /// Create a new credential pool with the given API keys.
    pub fn new(provider: impl Into<String>, keys: Vec<SecretString>) -> Self {
        let provider_str = provider.into();
        let entries: Vec<CredentialEntry> = keys
            .into_iter()
            .map(|key| CredentialEntry {
                key,
                health: KeyHealth::Healthy,
                success_count: 0,
                failure_count: 0,
                last_used: None,
            })
            .collect();

        if entries.is_empty() {
            warn!("Creating credential pool for '{}' with no keys", provider_str);
        } else {
            info!(
                "Creating credential pool for '{}' with {} keys",
                provider_str,
                entries.len()
            );
        }

        Self {
            provider: provider_str,
            entries: Arc::new(RwLock::new(entries)),
            current_index: Arc::new(RwLock::new(0)),
        }
    }

    /// Create a single-key pool (convenience for the common case).
    pub fn single(provider: impl Into<String>, key: SecretString) -> Self {
        Self::new(provider, vec![key])
    }

    /// Get the next available API key using round-robin selection.
    ///
    /// Skips disabled keys (unless their retry_after has elapsed) and
    /// revoked keys. Returns `None` if no keys are available.
    pub async fn get_key(&self) -> Option<SecretString> {
        let mut entries = self.entries.write().await;
        let mut index = self.current_index.write().await;
        let len = entries.len();

        if len == 0 {
            return None;
        }

        // Try each key starting from the current index
        for _ in 0..len {
            let i = *index % len;
            *index = (*index + 1) % len;

            // Check if this key is available
            let available = match &entries[i].health {
                KeyHealth::Healthy => true,
                KeyHealth::Disabled { retry_after, since, .. } => {
                    // Check if the retry_after period has elapsed
                    if let Some(retry) = retry_after {
                        since.elapsed() >= *retry
                    } else {
                        // No retry_after means permanently disabled until explicit reset
                        false
                    }
                }
                KeyHealth::Revoked { .. } => false,
            };

            if available {
                // Re-enable if it was disabled but retry_after elapsed
                if !matches!(entries[i].health, KeyHealth::Healthy) {
                    debug!("Re-enabling key at index {} for provider '{}'", i, self.provider);
                    entries[i].health = KeyHealth::Healthy;
                }
                entries[i].last_used = Some(Instant::now());
                return Some(entries[i].key.clone());
            }
        }

        // All keys exhausted
        warn!("All keys exhausted for provider '{}'", self.provider);
        None
    }

    /// Report a successful use of the current key.
    pub async fn report_success(&self) {
        let mut entries = self.entries.write().await;
        let index = self.current_index.read().await;
        if *index < entries.len() {
            // The current index points to the next key, so the one that
            // just succeeded is at (index + len - 1) % len
            let last_used_idx = (*index + entries.len() - 1) % entries.len();
            entries[last_used_idx].success_count += 1;
        }
    }

    /// Report an authentication failure for the current key.
    ///
    /// Disables the key for `retry_after` duration. If `retry_after` is
    /// `None`, the key is disabled until explicitly reset.
    pub async fn report_auth_failure(&self, retry_after: Option<Duration>) {
        let mut entries = self.entries.write().await;
        let index = self.current_index.read().await;
        if !entries.is_empty() {
            let last_used_idx = (*index + entries.len() - 1) % entries.len();
            entries[last_used_idx].failure_count += 1;
            entries[last_used_idx].health = KeyHealth::Disabled {
                since: Instant::now(),
                reason: "Authentication failure".to_string(),
                retry_after,
            };
            warn!(
                "Disabled key at index {} for provider '{}' (failures: {})",
                last_used_idx, self.provider, entries[last_used_idx].failure_count
            );
        }
    }

    /// Report a rate limit hit for the current key.
    ///
    /// Disables the key for the specified `retry_after` duration.
    pub async fn report_rate_limit(&self, retry_after: Duration) {
        let mut entries = self.entries.write().await;
        let index = self.current_index.read().await;
        if !entries.is_empty() {
            let last_used_idx = (*index + entries.len() - 1) % entries.len();
            entries[last_used_idx].health = KeyHealth::Disabled {
                since: Instant::now(),
                reason: "Rate limited".to_string(),
                retry_after: Some(retry_after),
            };
            debug!(
                "Rate-limited key at index {} for provider '{}' (retry after {:?})",
                last_used_idx, self.provider, retry_after
            );
        }
    }

    /// Revoke a key permanently.
    pub async fn revoke_key(&self, reason: impl Into<String>) {
        let mut entries = self.entries.write().await;
        let index = self.current_index.read().await;
        if !entries.is_empty() {
            let last_used_idx = (*index + entries.len() - 1) % entries.len();
            entries[last_used_idx].health = KeyHealth::Revoked {
                reason: reason.into(),
            };
            entries[last_used_idx].failure_count += 1;
            warn!(
                "Revoked key at index {} for provider '{}'",
                last_used_idx, self.provider
            );
        }
    }

    /// Reset all disabled keys to healthy status.
    pub async fn reset_all(&self) {
        let mut entries = self.entries.write().await;
        for entry in entries.iter_mut() {
            if matches!(entry.health, KeyHealth::Disabled { .. }) {
                entry.health = KeyHealth::Healthy;
            }
        }
        debug!("Reset all disabled keys for provider '{}'", self.provider);
    }

    /// Get the number of available (healthy) keys.
    pub async fn available_count(&self) -> usize {
        let entries = self.entries.read().await;
        entries
            .iter()
            .filter(|e| matches!(e.health, KeyHealth::Healthy))
            .count()
    }

    /// Get the total number of keys (including disabled/revoked).
    pub async fn total_count(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Get health status summary for all keys.
    pub async fn health_summary(&self) -> Vec<(KeyHealth, u64, u64)> {
        let entries = self.entries.read().await;
        entries
            .iter()
            .map(|e| (e.health.clone(), e.success_count, e.failure_count))
            .collect()
    }

    /// Get the provider name.
    pub fn provider(&self) -> &str {
        &self.provider
    }
}

/// Manages credential pools for multiple providers.
#[derive(Debug)]
pub struct CredentialPoolManager {
    pools: Arc<RwLock<HashMap<String, Arc<CredentialPool>>>>,
}

impl CredentialPoolManager {
    /// Create a new empty pool manager.
    pub fn new() -> Self {
        Self {
            pools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a credential pool for a provider.
    pub async fn register(&self, provider: impl Into<String>, pool: CredentialPool) {
        let provider = provider.into();
        self.pools
            .write()
            .await
            .insert(provider.clone(), Arc::new(pool));
        info!("Registered credential pool for provider '{}'", provider);
    }

    /// Get the credential pool for a provider.
    pub async fn get_pool(&self, provider: &str) -> Option<Arc<CredentialPool>> {
        self.pools.read().await.get(provider).cloned()
    }

    /// Get an API key for a provider.
    pub async fn get_key(&self, provider: &str) -> Option<SecretString> {
        let pools = self.pools.read().await;
        if let Some(pool) = pools.get(provider) {
            pool.get_key().await
        } else {
            None
        }
    }

    /// Report auth failure for a provider's current key.
    pub async fn report_auth_failure(&self, provider: &str, retry_after: Option<Duration>) {
        let pools = self.pools.read().await;
        if let Some(pool) = pools.get(provider) {
            pool.report_auth_failure(retry_after).await;
        }
    }

    /// Report rate limit hit for a provider's current key.
    pub async fn report_rate_limit(&self, provider: &str, retry_after: Duration) {
        let pools = self.pools.read().await;
        if let Some(pool) = pools.get(provider) {
            pool.report_rate_limit(retry_after).await;
        }
    }

    /// Report successful API call for a provider.
    pub async fn report_success(&self, provider: &str) {
        let pools = self.pools.read().await;
        if let Some(pool) = pools.get(provider) {
            pool.report_success().await;
        }
    }

    /// List all registered providers.
    pub async fn providers(&self) -> Vec<String> {
        self.pools.read().await.keys().cloned().collect()
    }
}

impl Default for CredentialPoolManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrecy::{ExposeSecret, SecretString};

    fn make_key(s: &str) -> SecretString {
        SecretString::new(s.to_string().into())
    }

    #[tokio::test]
    async fn test_single_key_pool() {
        let pool = CredentialPool::single("test", make_key("sk-test-123"));
        assert_eq!(pool.total_count().await, 1);
        assert_eq!(pool.available_count().await, 1);

        let key = pool.get_key().await.unwrap();
        assert_eq!(key.expose_secret(), &"sk-test-123");
    }

    #[tokio::test]
    async fn test_round_robin() {
        let pool = CredentialPool::new(
            "test",
            vec![make_key("key-1"), make_key("key-2"), make_key("key-3")],
        );

        let k1 = pool.get_key().await.unwrap();
        let k2 = pool.get_key().await.unwrap();
        let k3 = pool.get_key().await.unwrap();

        assert_eq!(k1.expose_secret(), &"key-1");
        assert_eq!(k2.expose_secret(), &"key-2");
        assert_eq!(k3.expose_secret(), &"key-3");

        // Should cycle back
        let k4 = pool.get_key().await.unwrap();
        assert_eq!(k4.expose_secret(), &"key-1");
    }

    #[tokio::test]
    async fn test_auth_failure_disables_key() {
        let pool = CredentialPool::new("test", vec![make_key("key-1"), make_key("key-2")]);

        // Use key-1
        let _ = pool.get_key().await;
        // Report auth failure on key-1
        pool.report_auth_failure(Some(Duration::from_secs(60))).await;

        // Should skip key-1 and use key-2
        let key = pool.get_key().await.unwrap();
        assert_eq!(key.expose_secret(), &"key-2");

        assert_eq!(pool.available_count().await, 1);
    }

    #[tokio::test]
    async fn test_rate_limit_disables_key() {
        let pool = CredentialPool::new("test", vec![make_key("key-1"), make_key("key-2")]);

        let _ = pool.get_key().await;
        pool.report_rate_limit(Duration::from_secs(30)).await;

        let key = pool.get_key().await.unwrap();
        assert_eq!(key.expose_secret(), &"key-2");
    }

    #[tokio::test]
    async fn test_all_keys_exhausted() {
        let pool = CredentialPool::new("test", vec![make_key("key-1")]);
        pool.report_auth_failure(None).await;

        let key = pool.get_key().await;
        assert!(key.is_none());
    }

    #[tokio::test]
    async fn test_reset_all() {
        let pool = CredentialPool::new("test", vec![make_key("key-1")]);
        pool.report_auth_failure(Some(Duration::from_secs(3600))).await;
        assert_eq!(pool.available_count().await, 0);

        pool.reset_all().await;
        assert_eq!(pool.available_count().await, 1);
    }

    #[tokio::test]
    async fn test_revoke_key() {
        let pool = CredentialPool::new("test", vec![make_key("key-1"), make_key("key-2")]);
        let _ = pool.get_key().await;
        pool.revoke_key("Key compromised").await;

        // key-1 is revoked, should use key-2
        let key = pool.get_key().await.unwrap();
        assert_eq!(key.expose_secret(), &"key-2");
    }

    #[tokio::test]
    async fn test_pool_manager() {
        let manager = CredentialPoolManager::new();
        let pool1 = CredentialPool::single("anthropic", make_key("sk-ant-123"));
        let pool2 = CredentialPool::single("openai", make_key("sk-oai-456"));

        manager.register("anthropic", pool1).await;
        manager.register("openai", pool2).await;

        let key = manager.get_key("anthropic").await.unwrap();
        assert_eq!(key.expose_secret(), &"sk-ant-123");

        let key = manager.get_key("openai").await.unwrap();
        assert_eq!(key.expose_secret(), &"sk-oai-456");

        let providers = manager.providers().await;
        assert_eq!(providers.len(), 2);
    }

    #[tokio::test]
    async fn test_empty_pool() {
        let pool = CredentialPool::new("test", vec![]);
        let key = pool.get_key().await;
        assert!(key.is_none());
    }

    #[tokio::test]
    async fn test_success_count() {
        let pool = CredentialPool::new("test", vec![make_key("key-1")]);

        let _ = pool.get_key().await;
        pool.report_success().await;
        pool.report_success().await;

        let summary = pool.health_summary().await;
        assert_eq!(summary[0].1, 2); // success_count
        assert_eq!(summary[0].2, 0); // failure_count
    }
}