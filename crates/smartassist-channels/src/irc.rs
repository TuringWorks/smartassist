//! IRC channel implementation.
//!
//! Provides a lightweight IRC client channel for SmartAssist.
//! This is a stub implementation suitable for testing and light use;
//! a full implementation would integrate an IRC client library.

#![cfg(feature = "irc")]

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

/// IRC channel implementation.
pub struct IrcChannel {
    instance_id: String,
    server: String,
    channel: String,
    nickname: String,
    connected: Arc<AtomicBool>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for IrcChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IrcChannel")
            .field("instance_id", &self.instance_id)
            .field("server", &self.server)
            .field("channel", &self.channel)
            .field("nickname", &self.nickname)
            .finish()
    }
}

impl IrcChannel {
    /// Create a new IRC channel.
    pub fn new(
        instance_id: impl Into<String>,
        server: impl Into<String>,
        channel: impl Into<String>,
        nickname: impl Into<String>,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: instance_id.into(),
            server: server.into(),
            channel: channel.into(),
            nickname: nickname.into(),
            connected: Arc::new(AtomicBool::new(false)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> Self {
        let server = config
            .options
            .get("server")
            .and_then(|v| v.as_str())
            .unwrap_or("irc.libera.chat:6667")
            .to_string();
        let channel = config
            .options
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("#smartassist")
            .to_string();
        let nickname = config
            .options
            .get("nickname")
            .and_then(|v| v.as_str())
            .unwrap_or("smartassist-bot")
            .to_string();
        Self::new(config.instance_id, server, channel, nickname)
    }
}

#[async_trait]
impl Channel for IrcChannel {
    fn channel_type(&self) -> &str {
        "irc"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Group, ChatType::Direct],
            media: MediaCapabilities {
                images: false,
                audio: false,
                video: false,
                files: false,
                stickers: false,
                voice_notes: false,
                max_file_size_mb: 0,
            },
            features: ChannelFeatures {
                reactions: false,
                threads: false,
                edits: false,
                deletes: false,
                typing_indicators: false,
                read_receipts: false,
                mentions: true,
                polls: false,
                native_commands: true,
            },
            limits: ChannelLimits {
                text_max_length: 512,
                caption_max_length: 0,
                messages_per_second: 1.0,
                messages_per_minute: 30,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for IrcChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let msg_id = uuid::Uuid::new_v4().to_string();
        debug!(
            "IRC send to {}: {} (msg_id: {})",
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
        warn!("IRC does not support attachments; sending text only");
        self.send(message).await
    }

    async fn edit(&self, _message: &MessageRef, _new_content: &str) -> Result<()> {
        Err(ChannelError::Unsupported("IRC does not support message editing".to_string()))
    }

    async fn delete(&self, _message: &MessageRef) -> Result<()> {
        Err(ChannelError::Unsupported("IRC does not support message deletion".to_string()))
    }

    async fn react(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        Err(ChannelError::Unsupported("IRC does not support reactions".to_string()))
    }

    async fn unreact(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        Err(ChannelError::Unsupported("IRC does not support reactions".to_string()))
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        // IRC has no typing indicator
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        512
    }
}

#[async_trait]
impl ChannelReceiver for IrcChannel {
    async fn start_receiving(&self) -> Result<()> {
        info!(
            "Started IRC receiver for {} on {}",
            self.instance_id,
            self.server
        );
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        info!("Stopped IRC receiver for {}", self.instance_id);
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
impl ChannelLifecycle for IrcChannel {
    async fn connect(&self) -> Result<()> {
        self.connected.store(true, Ordering::Relaxed);
        info!("IRC channel connected: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;
        self.connected.store(false, Ordering::Relaxed);
        info!("IRC channel disconnected: {}", self.instance_id);
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

impl Clone for IrcChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            server: self.server.clone(),
            channel: self.channel.clone(),
            nickname: self.nickname.clone(),
            connected: self.connected.clone(),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: self.handler.clone(),
        }
    }
}

/// Factory for creating IRC channels.
pub struct IrcChannelFactory;

#[async_trait]
impl ChannelFactory for IrcChannelFactory {
    async fn create(&self, config: ChannelConfig) -> Result<Box<dyn Channel>> {
        Ok(Box::new(IrcChannel::from_config(config)))
    }

    fn channel_type(&self) -> &str {
        "irc"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_irc_channel_creation() {
        let channel = IrcChannel::new("test_irc", "irc.libera.chat:6667", "#test", "bot");
        assert_eq!(channel.channel_type(), "irc");
        assert_eq!(channel.instance_id(), "test_irc");
    }

    #[test]
    fn test_irc_capabilities() {
        let channel = IrcChannel::new("test_irc", "irc.libera.chat:6667", "#test", "bot");
        let caps = channel.capabilities();
        assert!(!caps.media.images);
        assert!(caps.features.mentions);
        assert!(caps.features.native_commands);
        assert_eq!(caps.limits.text_max_length, 512);
    }

    #[tokio::test]
    async fn test_irc_connect_disconnect() {
        let channel = IrcChannel::new("test_irc", "irc.libera.chat:6667", "#test", "bot");
        assert!(!channel.is_connected());

        channel.connect().await.unwrap();
        assert!(channel.is_connected());

        channel.disconnect().await.unwrap();
        assert!(!channel.is_connected());
    }

    #[tokio::test]
    async fn test_irc_send_message() {
        let channel = IrcChannel::new("test_irc", "irc.libera.chat:6667", "#test", "bot");
        let target = MessageTarget {
            chat_id: "#test".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello IRC".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let result = channel.send(message).await.unwrap();
        assert!(!result.message_id.is_empty());
        assert_eq!(result.chat_id, "#test");
    }

    #[tokio::test]
    async fn test_irc_unsupported_operations() {
        let channel = IrcChannel::new("test_irc", "irc.libera.chat:6667", "#test", "bot");
        let msg_ref = MessageRef::new("msg1", "#test");

        assert!(channel.edit(&msg_ref, "new").await.is_err());
        assert!(channel.delete(&msg_ref).await.is_err());
        assert!(channel.react(&msg_ref, "👍").await.is_err());
    }

    #[test]
    fn test_irc_from_config() {
        let mut options = HashMap::new();
        options.insert("server".to_string(), serde_json::json!("irc.example.com:6667"));
        options.insert("channel".to_string(), serde_json::json!("#general"));
        options.insert("nickname".to_string(), serde_json::json!("mybot"));

        let config = ChannelConfig {
            channel_type: "irc".to_string(),
            instance_id: "irc-1".to_string(),
            account_id: "acct".to_string(),
            enabled: true,
            options,
        };

        let channel = IrcChannel::from_config(config);
        assert_eq!(channel.instance_id(), "irc-1");
    }
}
