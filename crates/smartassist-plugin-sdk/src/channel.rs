//! Channel plugin support.
//!
//! This module provides traits for plugins that add new messaging channels.

use crate::{PluginContext, Result};
use async_trait::async_trait;
use smartassist_channels::{Channel, ChannelConfig};
use std::sync::Arc;

/// Trait for plugins that provide messaging channels.
#[async_trait]
pub trait ChannelPlugin: Send + Sync {
    /// Get the channel type identifier (e.g., "telegram", "discord").
    fn channel_type(&self) -> &str;

    /// Get the channel factory.
    fn factory(&self) -> &dyn ChannelPluginFactory;

    /// Create a channel instance from configuration.
    async fn create_channel(&self, config: ChannelConfig) -> Result<Arc<dyn Channel>>;

    /// Validate channel configuration.
    fn validate_config(&self, config: &ChannelConfig) -> Result<()> {
        // Default implementation accepts any config
        let _ = config;
        Ok(())
    }

    /// Get default configuration for this channel type.
    fn default_config(&self) -> ChannelConfig {
        ChannelConfig::new(
            self.channel_type(),
            format!("{}_default", self.channel_type()),
            format!("{}_default_account", self.channel_type()),
        )
    }

    /// Get documentation URL for this channel.
    fn documentation_url(&self) -> Option<&str> {
        None
    }
}

/// Factory trait for creating channel instances.
#[async_trait]
pub trait ChannelPluginFactory: Send + Sync {
    /// Create a new channel instance.
    async fn create(
        &self,
        config: ChannelConfig,
        ctx: &PluginContext,
    ) -> Result<Arc<dyn Channel>>;

    /// Get required configuration keys.
    fn required_config_keys(&self) -> Vec<&str> {
        vec![]
    }

    /// Get optional configuration keys with descriptions.
    fn optional_config_keys(&self) -> Vec<(&str, &str)> {
        vec![]
    }
}

/// Builder for channel plugins.
pub struct ChannelPluginBuilder {
    channel_type: String,
    docs_url: Option<String>,
}

impl ChannelPluginBuilder {
    /// Create a new channel plugin builder.
    pub fn new(channel_type: impl Into<String>) -> Self {
        Self {
            channel_type: channel_type.into(),
            docs_url: None,
        }
    }

    /// Set the documentation URL.
    pub fn with_docs(mut self, url: impl Into<String>) -> Self {
        self.docs_url = Some(url.into());
        self
    }

    /// Get the channel type.
    pub fn channel_type(&self) -> &str {
        &self.channel_type
    }

    /// Get the documentation URL.
    pub fn docs_url(&self) -> Option<&str> {
        self.docs_url.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_plugin_builder() {
        let builder = ChannelPluginBuilder::new("my-channel")
            .with_docs("https://docs.example.com/my-channel");

        assert_eq!(builder.channel_type(), "my-channel");
        assert_eq!(builder.docs_url(), Some("https://docs.example.com/my-channel"));
    }
}
