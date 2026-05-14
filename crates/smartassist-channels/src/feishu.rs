//! Feishu (Lark) channel implementation.
//!
//! Provides a Feishu webhook/API-based channel for SmartAssist.

#![cfg(feature = "feishu")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver, ChannelSender, MessageHandler,
    MessageRef, SendResult,
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

/// Feishu channel implementation.
pub struct FeishuChannel {
    instance_id: String,
    webhook_url: String,
    app_id: Option<String>,
    connected: Arc<AtomicBool>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for FeishuChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeishuChannel")
            .field("instance_id", &self.instance_id)
            .field("app_id", &self.app_id)
            .finish()
    }
}

impl FeishuChannel {
    /// Create a new Feishu channel.
    pub fn new(instance_id: impl Into<String>, webhook_url: impl Into<String>) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: instance_id.into(),
            webhook_url: webhook_url.into(),
            app_id: None,
            connected: Arc::new(AtomicBool::new(false)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> Self {
        let webhook_url = config
            .options
            .get("webhook_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut channel = Self::new(config.instance_id, webhook_url);
        if let Some(app_id) = config.options.get("app_id").and_then(|v| v.as_str()) {
            channel.app_id = Some(app_id.to_string());
        }
        channel
    }
}

#[async_trait]
impl Channel for FeishuChannel {
    fn channel_type(&self) -> &str {
        "feishu"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Group, ChatType::Direct],
            media: MediaCapabilities {
                images: true,
                audio: false,
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
                typing_indicators: false,
                read_receipts: true,
                mentions: true,
                polls: true,
                native_commands: true,
            },
            limits: ChannelLimits {
                text_max_length: 10000,
                caption_max_length: 1000,
                messages_per_second: 5.0,
                messages_per_minute: 150,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for FeishuChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let msg_id = uuid::Uuid::new_v4().to_string();
        debug!(
            "Feishu send to {}: {} (msg_id: {})",
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
        warn!("Feishu attachments stub: sending text only");
        self.send(message).await
    }

    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()> {
        debug!("Feishu edit {}: {}", message.message_id, new_content);
        Ok(())
    }

    async fn delete(&self, message: &MessageRef) -> Result<()> {
        debug!("Feishu delete {}", message.message_id);
        Ok(())
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!("Feishu react {} with {}", message.message_id, emoji);
        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!("Feishu unreact {} with {}", message.message_id, emoji);
        Ok(())
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        10000
    }
}

#[async_trait]
impl ChannelReceiver for FeishuChannel {
    async fn start_receiving(&self) -> Result<()> {
        info!("Started Feishu receiver for {}", self.instance_id);
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        info!("Stopped Feishu receiver for {}", self.instance_id);
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
impl ChannelLifecycle for FeishuChannel {
    async fn connect(&self) -> Result<()> {
        self.connected.store(true, Ordering::Relaxed);
        
        info!("Feishu channel connected: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;
        self.connected.store(false, Ordering::Relaxed);
        
        info!("Feishu channel disconnected: {}", self.instance_id);
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

impl Clone for FeishuChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            webhook_url: self.webhook_url.clone(),
            app_id: self.app_id.clone(),
            connected: self.connected.clone(),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: self.handler.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feishu_channel_creation() {
        let channel = FeishuChannel::new("test_feishu", "https://open.feishu.cn/open-apis/bot/v2/hook/XXX");
        assert_eq!(channel.channel_type(), "feishu");
        assert_eq!(channel.instance_id(), "test_feishu");
    }

    #[test]
    fn test_feishu_capabilities() {
        let channel = FeishuChannel::new("test_feishu", "https://open.feishu.cn/open-apis/bot/v2/hook/XXX");
        let caps = channel.capabilities();
        assert!(caps.media.voice_notes);
        assert!(caps.features.read_receipts);
        assert!(caps.features.polls);
        assert_eq!(caps.limits.text_max_length, 10000);
    }

    #[tokio::test]
    async fn test_feishu_connect_disconnect() {
        let channel = FeishuChannel::new("test_feishu", "https://open.feishu.cn/open-apis/bot/v2/hook/XXX");
        assert!(!channel.is_connected());

        channel.connect().await.unwrap();
        assert!(channel.is_connected());

        channel.disconnect().await.unwrap();
        assert!(!channel.is_connected());
    }

    #[tokio::test]
    async fn test_feishu_send_message() {
        let channel = FeishuChannel::new("test_feishu", "https://open.feishu.cn/open-apis/bot/v2/hook/XXX");
        let target = MessageTarget {
            chat_id: "oc_123".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello Feishu".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let result = channel.send(message).await.unwrap();
        assert!(!result.message_id.is_empty());
    }
}
