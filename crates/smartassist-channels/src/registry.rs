//! Channel registry for managing channel instances.

use crate::error::ChannelError;
use crate::traits::{Channel, ChannelConfig, ChannelFactory};
use crate::Result;
use smartassist_core::types::ChannelHealth;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Registry for managing channel instances.
pub struct ChannelRegistry {
    /// Registered channels by instance ID.
    channels: RwLock<HashMap<String, RegisteredChannel>>,

    /// Channel factories by channel type.
    factories: RwLock<HashMap<String, Arc<dyn ChannelFactory>>>,
}

/// A registered channel with its metadata.
pub struct RegisteredChannel {
    /// The channel instance.
    pub channel: Arc<dyn Channel>,

    /// Channel configuration.
    pub config: ChannelConfig,

    /// Whether the channel is enabled.
    pub enabled: bool,
}

impl Default for ChannelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelRegistry {
    /// Create a new channel registry.
    pub fn new() -> Self {
        Self {
            channels: RwLock::new(HashMap::new()),
            factories: RwLock::new(HashMap::new()),
        }
    }

    /// Register a channel factory.
    pub async fn register_factory(&self, factory: Arc<dyn ChannelFactory>) {
        let mut factories = self.factories.write().await;
        let channel_type = factory.channel_type().to_string();
        info!("Registering factory for channel type: {}", channel_type);
        factories.insert(channel_type, factory);
    }

    /// Create and register a channel from configuration.
    pub async fn create_channel(&self, config: ChannelConfig) -> Result<Arc<dyn Channel>> {
        // Get the factory
        let factories = self.factories.read().await;
        let factory = factories.get(&config.channel_type).ok_or_else(|| {
            ChannelError::Config(format!(
                "No factory registered for channel type: {}",
                config.channel_type
            ))
        })?;

        // Create the channel
        let channel = factory.create(config.clone()).await?;
        let channel: Arc<dyn Channel> = channel.into();

        // Register it
        let mut channels = self.channels.write().await;
        let instance_id = config.instance_id.clone();

        if channels.contains_key(&instance_id) {
            return Err(ChannelError::AlreadyExists(instance_id));
        }

        channels.insert(
            instance_id.clone(),
            RegisteredChannel {
                channel: channel.clone(),
                config,
                enabled: true,
            },
        );

        info!("Created and registered channel: {}", instance_id);
        Ok(channel)
    }

    /// Register an existing channel instance.
    pub async fn register(&self, config: ChannelConfig, channel: Arc<dyn Channel>) -> Result<()> {
        let mut channels = self.channels.write().await;
        let instance_id = config.instance_id.clone();

        if channels.contains_key(&instance_id) {
            return Err(ChannelError::AlreadyExists(instance_id));
        }

        channels.insert(
            instance_id.clone(),
            RegisteredChannel {
                channel,
                config,
                enabled: true,
            },
        );

        info!("Registered channel: {}", instance_id);
        Ok(())
    }

    /// Unregister a channel.
    pub async fn unregister(&self, instance_id: &str) -> Result<()> {
        let mut channels = self.channels.write().await;

        if let Some(registered) = channels.remove(instance_id) {
            // Disconnect the channel
            if let Err(e) = registered.channel.disconnect().await {
                warn!("Error disconnecting channel {}: {}", instance_id, e);
            }
            info!("Unregistered channel: {}", instance_id);
            Ok(())
        } else {
            Err(ChannelError::not_found(instance_id))
        }
    }

    /// Get a channel by instance ID.
    pub async fn get(&self, instance_id: &str) -> Option<Arc<dyn Channel>> {
        let channels = self.channels.read().await;
        channels.get(instance_id).map(|r| r.channel.clone())
    }

    /// Get a channel configuration.
    pub async fn get_config(&self, instance_id: &str) -> Option<ChannelConfig> {
        let channels = self.channels.read().await;
        channels.get(instance_id).map(|r| r.config.clone())
    }

    /// List all registered channel instance IDs.
    pub async fn list(&self) -> Vec<String> {
        let channels = self.channels.read().await;
        channels.keys().cloned().collect()
    }

    /// List channels by type.
    pub async fn list_by_type(&self, channel_type: &str) -> Vec<String> {
        let channels = self.channels.read().await;
        channels
            .iter()
            .filter(|(_, r)| r.config.channel_type == channel_type)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Enable a channel.
    pub async fn enable(&self, instance_id: &str) -> Result<()> {
        let mut channels = self.channels.write().await;

        if let Some(registered) = channels.get_mut(instance_id) {
            registered.enabled = true;
            debug!("Enabled channel: {}", instance_id);
            Ok(())
        } else {
            Err(ChannelError::not_found(instance_id))
        }
    }

    /// Disable a channel.
    pub async fn disable(&self, instance_id: &str) -> Result<()> {
        let mut channels = self.channels.write().await;

        if let Some(registered) = channels.get_mut(instance_id) {
            registered.enabled = false;
            debug!("Disabled channel: {}", instance_id);
            Ok(())
        } else {
            Err(ChannelError::not_found(instance_id))
        }
    }

    /// Check if a channel is enabled.
    pub async fn is_enabled(&self, instance_id: &str) -> bool {
        let channels = self.channels.read().await;
        channels
            .get(instance_id)
            .map(|r| r.enabled)
            .unwrap_or(false)
    }

    /// Connect all enabled channels.
    pub async fn connect_all(&self) -> Vec<(String, Result<()>)> {
        let channels = self.channels.read().await;
        let mut results = Vec::new();

        for (id, registered) in channels.iter() {
            if !registered.enabled {
                continue;
            }

            let result = registered.channel.connect().await;
            if let Err(ref e) = result {
                error!("Failed to connect channel {}: {}", id, e);
            } else {
                info!("Connected channel: {}", id);
            }
            results.push((id.clone(), result));
        }

        results
    }

    /// Disconnect all channels.
    pub async fn disconnect_all(&self) -> Vec<(String, Result<()>)> {
        let channels = self.channels.read().await;
        let mut results = Vec::new();

        for (id, registered) in channels.iter() {
            let result = registered.channel.disconnect().await;
            if let Err(ref e) = result {
                warn!("Error disconnecting channel {}: {}", id, e);
            }
            results.push((id.clone(), result));
        }

        results
    }

    /// Get health status for all channels.
    pub async fn health_check(&self) -> HashMap<String, ChannelHealth> {
        let channels = self.channels.read().await;
        let mut health_map = HashMap::new();

        for (id, registered) in channels.iter() {
            match registered.channel.health().await {
                Ok(health) => {
                    health_map.insert(id.clone(), health);
                }
                Err(e) => {
                    health_map.insert(
                        id.clone(),
                        ChannelHealth {
                            status: smartassist_core::types::HealthStatus::Unhealthy,
                            latency_ms: None,
                            last_message_at: None,
                            error: Some(e.to_string()),
                        },
                    );
                }
            }
        }

        health_map
    }

    /// Get the count of registered channels.
    pub async fn count(&self) -> usize {
        let channels = self.channels.read().await;
        channels.len()
    }

    /// Get the count of connected channels.
    pub async fn connected_count(&self) -> usize {
        let channels = self.channels.read().await;
        channels
            .values()
            .filter(|r| r.enabled && r.channel.is_connected())
            .count()
    }
}

/// Statistics about the channel registry.
#[derive(Debug, Clone, Default)]
pub struct RegistryStats {
    /// Total registered channels.
    pub total: usize,

    /// Enabled channels.
    pub enabled: usize,

    /// Connected channels.
    pub connected: usize,

    /// Channels by type.
    pub by_type: HashMap<String, usize>,
}

impl ChannelRegistry {
    /// Get registry statistics.
    pub async fn stats(&self) -> RegistryStats {
        let channels = self.channels.read().await;

        let mut stats = RegistryStats {
            total: channels.len(),
            ..Default::default()
        };

        for registered in channels.values() {
            if registered.enabled {
                stats.enabled += 1;
            }
            if registered.channel.is_connected() {
                stats.connected += 1;
            }
            *stats.by_type.entry(registered.config.channel_type.clone()).or_insert(0) += 1;
        }

        stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_creation() {
        let registry = ChannelRegistry::new();
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_registry_list() {
        let registry = ChannelRegistry::new();
        let list = registry.list().await;
        assert!(list.is_empty());
    }
}
