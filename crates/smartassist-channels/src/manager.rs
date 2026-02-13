//! Channel manager for orchestrating messaging channels.
//!
//! The ChannelManager provides a unified interface for:
//! - Managing channel lifecycle (connect, disconnect)
//! - Routing inbound messages to agents
//! - Delivering outbound messages via the delivery pipeline
//! - Health monitoring and status reporting

use crate::delivery::{DeliveryConfig, DeliveryQueue};
use crate::error::ChannelError;
use crate::registry::{ChannelRegistry, RegistryStats};
use crate::routing::{RouteMatch, RouteRule, Router};
use crate::traits::{Channel, ChannelConfig, ChannelFactory, SendResult};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{
    AgentId, ChannelHealth, InboundMessage, MessageTarget, OutboundMessage,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// The central manager for all messaging channels.
pub struct ChannelManager {
    /// Channel registry for managing instances.
    registry: Arc<ChannelRegistry>,

    /// Message router.
    router: Arc<RwLock<Router>>,

    /// Delivery queue for outbound messages.
    delivery_queue: Arc<DeliveryQueue>,

    /// Broadcast channel for inbound messages.
    inbound_tx: broadcast::Sender<InboundMessage>,

    /// Message handler for routing decisions.
    message_handler: Arc<RwLock<Option<Arc<dyn ManagerMessageHandler>>>>,

    /// Running state.
    running: Arc<RwLock<bool>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<mpsc::Sender<()>>>>,
}

/// Handler for processing routed messages.
#[async_trait]
pub trait ManagerMessageHandler: Send + Sync {
    /// Handle a routed inbound message.
    async fn handle_message(
        &self,
        message: InboundMessage,
        route: RouteMatch,
    ) -> Result<()>;
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelManager {
    /// Create a new channel manager.
    pub fn new() -> Self {
        let (inbound_tx, _) = broadcast::channel(1000);

        Self {
            registry: Arc::new(ChannelRegistry::new()),
            router: Arc::new(RwLock::new(Router::new())),
            delivery_queue: Arc::new(DeliveryQueue::new(DeliveryConfig::default())),
            inbound_tx,
            message_handler: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create with custom components.
    pub fn with_components(
        registry: Arc<ChannelRegistry>,
        router: Router,
        delivery_queue: Arc<DeliveryQueue>,
    ) -> Self {
        let (inbound_tx, _) = broadcast::channel(1000);

        Self {
            registry,
            router: Arc::new(RwLock::new(router)),
            delivery_queue,
            inbound_tx,
            message_handler: Arc::new(RwLock::new(None)),
            running: Arc::new(RwLock::new(false)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Get the channel registry.
    pub fn registry(&self) -> &Arc<ChannelRegistry> {
        &self.registry
    }

    /// Get the delivery queue.
    pub fn delivery_queue(&self) -> &Arc<DeliveryQueue> {
        &self.delivery_queue
    }

    // --- Factory Registration ---

    /// Register a channel factory.
    pub async fn register_factory(&self, factory: Arc<dyn ChannelFactory>) {
        self.registry.register_factory(factory).await;
    }

    // --- Channel Management ---

    /// Create and register a channel from configuration.
    pub async fn create_channel(&self, config: ChannelConfig) -> Result<Arc<dyn Channel>> {
        self.registry.create_channel(config).await
    }

    /// Register an existing channel.
    pub async fn register_channel(
        &self,
        config: ChannelConfig,
        channel: Arc<dyn Channel>,
    ) -> Result<()> {
        self.registry.register(config, channel).await
    }

    /// Get a channel by instance ID.
    pub async fn get_channel(&self, instance_id: &str) -> Option<Arc<dyn Channel>> {
        self.registry.get(instance_id).await
    }

    /// List all registered channels.
    pub async fn list_channels(&self) -> Vec<String> {
        self.registry.list().await
    }

    /// Unregister a channel.
    pub async fn remove_channel(&self, instance_id: &str) -> Result<()> {
        self.registry.unregister(instance_id).await
    }

    // --- Routing ---

    /// Set the default agent for routing.
    pub async fn set_default_agent(&self, agent_id: AgentId) {
        let mut router = self.router.write().await;
        *router = std::mem::take(&mut *router).with_default_agent(agent_id);
    }

    /// Add a routing rule.
    pub async fn add_route(&self, rule: RouteRule) {
        let mut router = self.router.write().await;
        router.add_rule(rule);
    }

    /// Remove a routing rule.
    pub async fn remove_route(&self, rule_id: &str) {
        let mut router = self.router.write().await;
        router.remove_rule(rule_id);
    }

    /// Route a message to an agent.
    pub async fn route_message(&self, message: &InboundMessage) -> Result<RouteMatch> {
        let router = self.router.read().await;
        router.route(message)
    }

    // --- Message Handler ---

    /// Set the message handler for routed messages.
    pub async fn set_message_handler(&self, handler: Arc<dyn ManagerMessageHandler>) {
        let mut h = self.message_handler.write().await;
        *h = Some(handler);
    }

    /// Subscribe to inbound messages.
    pub fn subscribe(&self) -> broadcast::Receiver<InboundMessage> {
        self.inbound_tx.subscribe()
    }

    // --- Sending Messages ---

    /// Send a message through a specific channel.
    pub async fn send(
        &self,
        channel_id: &str,
        message: OutboundMessage,
    ) -> Result<SendResult> {
        let channel = self.registry.get(channel_id).await.ok_or_else(|| {
            ChannelError::not_found(channel_id)
        })?;

        channel.send(message).await
    }

    /// Send a message to a specific target (auto-selects channel).
    pub async fn send_to(
        &self,
        channel_type: &str,
        target: MessageTarget,
        text: impl Into<String>,
    ) -> Result<SendResult> {
        // Find a connected channel of the specified type
        let channels = self.registry.list_by_type(channel_type).await;

        for instance_id in channels {
            if let Some(channel) = self.registry.get(&instance_id).await {
                if channel.is_connected() {
                    let message = OutboundMessage {
                        target: target.clone(),
                        text: text.into(),
                        media: vec![],
                        mentions: vec![],
                        reply_to: None,
                        options: Default::default(),
                    };
                    return channel.send(message).await;
                }
            }
        }

        Err(ChannelError::not_found(format!(
            "No connected channel of type '{}'",
            channel_type
        )))
    }

    /// Queue a message for delivery.
    pub async fn queue_message(
        &self,
        channel_id: &str,
        message: OutboundMessage,
    ) -> Result<String> {
        let id = self
            .delivery_queue
            .enqueue(channel_id.to_string(), message)
            .await?;
        Ok(id)
    }

    // --- Lifecycle ---

    /// Start the channel manager.
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }

        info!("Starting channel manager");

        // Connect all enabled channels
        let results = self.registry.connect_all().await;
        for (id, result) in &results {
            match result {
                Ok(()) => info!("Channel {} connected", id),
                Err(e) => error!("Channel {} failed to connect: {}", id, e),
            }
        }

        // Set up message receiving for all channels
        self.start_message_receiving().await?;

        // Start delivery processing
        self.start_delivery_processing().await?;

        *running = true;
        info!("Channel manager started");

        Ok(())
    }

    /// Stop the channel manager.
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if !*running {
            return Ok(());
        }

        info!("Stopping channel manager");

        // Send shutdown signal
        let mut shutdown = self.shutdown.write().await;
        if let Some(tx) = shutdown.take() {
            let _ = tx.send(()).await;
        }

        // Disconnect all channels
        self.registry.disconnect_all().await;

        *running = false;
        info!("Channel manager stopped");

        Ok(())
    }

    /// Check if the manager is running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Start receiving messages from all channels.
    async fn start_message_receiving(&self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = mpsc::channel(1);

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        let registry = self.registry.clone();
        let inbound_tx = self.inbound_tx.clone();
        let router = self.router.clone();
        let handler = self.message_handler.clone();

        tokio::spawn(async move {
            info!("Starting message receive loop");

            loop {
                tokio::select! {
                    _ = shutdown_rx.recv() => {
                        info!("Message receive loop shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                        // Poll all channels for messages
                        let channel_ids = registry.list().await;

                        for id in channel_ids {
                            if let Some(channel) = registry.get(&id).await {
                                if !channel.is_connected() {
                                    continue;
                                }

                                // Try to receive a message
                                match channel.try_receive().await {
                                    Ok(Some(message)) => {
                                        debug!("Received message from channel {}: {:?}", id, message.id);

                                        // Broadcast to subscribers
                                        if let Err(e) = inbound_tx.send(message.clone()) {
                                            debug!("No subscribers for inbound messages: {}", e);
                                        }

                                        // Route the message
                                        let router_guard = router.read().await;
                                        match router_guard.route(&message) {
                                            Ok(route) => {
                                                let handler_guard = handler.read().await;
                                                if let Some(ref h) = *handler_guard {
                                                    if let Err(e) = h.handle_message(message, route).await {
                                                        warn!("Message handler error: {}", e);
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                warn!("Routing error for message: {}", e);
                                            }
                                        }
                                    }
                                    Ok(None) => {
                                        // No message available
                                    }
                                    Err(e) => {
                                        debug!("Error receiving from channel {}: {}", id, e);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Start processing the delivery queue.
    async fn start_delivery_processing(&self) -> Result<()> {
        let queue = self.delivery_queue.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            info!("Starting delivery processing loop");

            loop {
                // Check if still running
                if !*running.read().await {
                    break;
                }

                // Process queued messages using the delivery queue's built-in processing
                let stats = queue.stats().await;
                if stats.pending > 0 {
                    debug!("Processing {} queued messages", stats.pending);
                    let results = queue.process().await;
                    for result in results {
                        if result.success {
                            debug!("Message {} delivered successfully", result.id);
                        } else if let Some(ref err) = result.error {
                            warn!("Delivery failed for message {}: {}", result.id, err);
                        }
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        });

        Ok(())
    }

    // --- Health & Status ---

    /// Get health status for all channels.
    pub async fn health(&self) -> HashMap<String, ChannelHealth> {
        self.registry.health_check().await
    }

    /// Get registry statistics.
    pub async fn stats(&self) -> RegistryStats {
        self.registry.stats().await
    }

    /// Get manager status.
    pub async fn status(&self) -> ManagerStatus {
        let stats = self.registry.stats().await;
        let queue_stats = self.delivery_queue.stats().await;

        ManagerStatus {
            running: *self.running.read().await,
            channels_total: stats.total,
            channels_connected: stats.connected,
            channels_enabled: stats.enabled,
            queue_pending: queue_stats.pending,
            queue_delivered: queue_stats.delivered,
        }
    }
}

/// Manager status information.
#[derive(Debug, Clone)]
pub struct ManagerStatus {
    /// Whether the manager is running.
    pub running: bool,

    /// Total registered channels.
    pub channels_total: usize,

    /// Connected channels.
    pub channels_connected: usize,

    /// Enabled channels.
    pub channels_enabled: usize,

    /// Pending messages in queue.
    pub queue_pending: usize,

    /// Delivered messages.
    pub queue_delivered: usize,
}

/// Builder for creating a ChannelManager with configuration.
pub struct ChannelManagerBuilder {
    default_agent: Option<AgentId>,
    rules: Vec<RouteRule>,
    delivery_config: DeliveryConfig,
}

impl Default for ChannelManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ChannelManagerBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            default_agent: None,
            rules: Vec::new(),
            delivery_config: DeliveryConfig::default(),
        }
    }

    /// Set the default agent.
    pub fn default_agent(mut self, agent_id: AgentId) -> Self {
        self.default_agent = Some(agent_id);
        self
    }

    /// Add a routing rule.
    pub fn route(mut self, rule: RouteRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Set the delivery queue max size.
    pub fn queue_size(mut self, size: usize) -> Self {
        self.delivery_config.max_queue_size = size;
        self
    }

    /// Set the delivery configuration.
    pub fn delivery_config(mut self, config: DeliveryConfig) -> Self {
        self.delivery_config = config;
        self
    }

    /// Build the channel manager.
    pub fn build(self) -> ChannelManager {
        let mut router = Router::new();

        if let Some(agent_id) = self.default_agent {
            router = router.with_default_agent(agent_id);
        }

        for rule in self.rules {
            router.add_rule(rule);
        }

        ChannelManager::with_components(
            Arc::new(ChannelRegistry::new()),
            router,
            Arc::new(DeliveryQueue::new(self.delivery_config)),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_manager_creation() {
        let manager = ChannelManager::new();
        assert!(!manager.is_running().await);
        assert_eq!(manager.list_channels().await.len(), 0);
    }

    #[tokio::test]
    async fn test_builder() {
        let manager = ChannelManagerBuilder::new()
            .default_agent(AgentId::new("test_agent"))
            .queue_size(50)
            .build();

        assert!(!manager.is_running().await);
    }

    #[tokio::test]
    async fn test_status() {
        let manager = ChannelManager::new();
        let status = manager.status().await;

        assert!(!status.running);
        assert_eq!(status.channels_total, 0);
        assert_eq!(status.queue_pending, 0);
    }
}
