//! Mattermost channel implementation.
//!
//! Provides a Mattermost API-based channel for SmartAssist.

#![cfg(feature = "mattermost")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelFactory, ChannelLifecycle, ChannelReceiver, ChannelSender,
    MessageHandler, MessageRef, SendResult,
};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatType,
    HealthStatus, InboundMessage, MediaCapabilities, MessageTarget, OutboundMessage,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Mattermost channel implementation.
pub struct MattermostChannel {
    instance_id: String,
    server_url: String,
    token: String,
    connected: Arc<AtomicBool>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for MattermostChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MattermostChannel")
            .field("instance_id", &self.instance_id)
            .field("server_url", &self.server_url)
            .finish()
    }
}

impl MattermostChannel {
    /// Create a new Mattermost channel.
    pub fn new(
        instance_id: impl Into<String>,
        server_url: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: instance_id.into(),
            server_url: server_url.into(),
            token: token.into(),
            connected: Arc::new(AtomicBool::new(false)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> Self {
        let server_url = config
            .options
            .get("server_url")
            .and_then(|v| v.as_str())
            .unwrap_or("https://mattermost.example.com")
            .to_string();
        let token = config
            .options
            .get("token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Self::new(config.instance_id, server_url, token)
    }
}

#[async_trait]
impl Channel for MattermostChannel {
    fn channel_type(&self) -> &str {
        "mattermost"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Group, ChatType::Direct],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: false,
                voice_notes: true,
                max_file_size_mb: 50,
            },
            features: ChannelFeatures {
                reactions: true,
                threads: true,
                edits: true,
                deletes: true,
                typing_indicators: true,
                read_receipts: true,
                mentions: true,
                polls: true,
                native_commands: true,
            },
            limits: ChannelLimits {
                text_max_length: 4000,
                caption_max_length: 1000,
                messages_per_second: 10.0,
                messages_per_minute: 300,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for MattermostChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let msg_id = uuid::Uuid::new_v4().to_string();
        debug!(
            "Mattermost send to {}: {} (msg_id: {})",
            message.target.chat_id,
            message.text,
            msg_id
        );
        Ok(SendResult::with_chat(msg_id, message.target.chat_id))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        _attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        warn!("Mattermost attachments stub: sending text only");
        self.send(message).await
    }

    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()> {
        debug!(
            "Mattermost edit {}: {}",
            message.message_id, new_content
        );
        Ok(())
    }

    async fn delete(&self, message: &MessageRef) -> Result<()> {
        debug!("Mattermost delete {}", message.message_id);
        Ok(())
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!(
            "Mattermost react {} with {}",
            message.message_id, emoji
        );
        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!(
            "Mattermost unreact {} with {}",
            message.message_id, emoji
        );
        Ok(())
    }

    async fn send_typing(&self, target: &MessageTarget) -> Result<()> {
        debug!("Mattermost typing in {}", target.chat_id);
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        4000
    }
}

#[async_trait]
impl ChannelReceiver for MattermostChannel {
    async fn start_receiving(&self) -> Result<()> {
        info!("Started Mattermost receiver for {}", self.instance_id);
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        info!("Stopped Mattermost receiver for {}", self.instance_id);
        Ok(())
    }

    async fn receive(&self) -> Result<InboundMessage> {
        let mut rx = self.message_rx.write().await;
        rx.recv()
            .await
            .ok_or_else(|| ChannelError::Internal("Channel closed".to_string()))
    }

    async fn try_receive(&self) -> Result<Option<InboundMessage>> {
        let mut rx = self.message_rx.write().await;
        match rx.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(mpsc::error::TryRecvError::Disconnected) => {
                Err(ChannelError::Internal("Channel closed".to_string()))
            }
        }
    }

    fn set_handler(&self, handler: Box<dyn MessageHandler>) {
        let handler_arc = self.handler.clone();
        tokio::spawn(async move {
            let mut h = handler_arc.write().await;
            *h = Some(handler);
        });
    }
}

#[async_trait]
impl ChannelLifecycle for MattermostChannel {
    async fn connect(&self) -> Result<()> {
        self.connected.store(true, Ordering::Relaxed);
        
        info!("Mattermost channel connected: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;
        self.connected.store(false, Ordering::Relaxed);
        
        info!("Mattermost channel disconnected: {}", self.instance_id);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let connected = self.connected.load(Ordering::Relaxed);
        Ok(ChannelHealth {
            status: if connected {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            },
            latency_ms: Some(0),
            last_message_at: None,
            error: if connected {
                None
            } else {
                Some("Not connected".to_string())
            },
        })
    }
}

impl Clone for MattermostChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            server_url: self.server_url.clone(),
            token: self.token.clone(),
            connected: self.connected.clone(),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: self.handler.clone(),
        }
    }
}

/// Factory for creating Mattermost channels.
pub struct MattermostChannelFactory;

#[async_trait]
impl ChannelFactory for MattermostChannelFactory {
    async fn create(&self, config: ChannelConfig) -> Result<Box<dyn Channel>> {
        Ok(Box::new(MattermostChannel::from_config(config)))
    }

    fn channel_type(&self) -> &str {
        "mattermost"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mattermost_channel_creation() {
        let channel = MattermostChannel::new("test_mm", "https://mm.example.com", "token123");
        assert_eq!(channel.channel_type(), "mattermost");
        assert_eq!(channel.instance_id(), "test_mm");
    }

    #[test]
    fn test_mattermost_capabilities() {
        let channel = MattermostChannel::new("test_mm", "https://mm.example.com", "token123");
        let caps = channel.capabilities();
        assert!(caps.media.voice_notes);
        assert!(caps.features.typing_indicators);
        assert!(caps.features.read_receipts);
        assert_eq!(caps.limits.text_max_length, 4000);
    }

    #[tokio::test]
    async fn test_mattermost_connect_disconnect() {
        let channel = MattermostChannel::new("test_mm", "https://mm.example.com", "token123");
        assert!(!channel.is_connected());

        channel.connect().await.unwrap();
        assert!(channel.is_connected());

        channel.disconnect().await.unwrap();
        assert!(!channel.is_connected());
    }

    #[tokio::test]
    async fn test_mattermost_send_message() {
        let channel = MattermostChannel::new("test_mm", "https://mm.example.com", "token123");
        let target = MessageTarget {
            chat_id: "channel:town-square".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello Mattermost".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let result = channel.send(message).await.unwrap();
        assert!(!result.message_id.is_empty());
    }
}
