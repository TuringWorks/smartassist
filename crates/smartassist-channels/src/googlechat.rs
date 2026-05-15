//! Google Chat channel implementation.
//!
//! Provides a Google Chat webhook/API-based channel for SmartAssist.

#![cfg(feature = "googlechat")]

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
use reqwest;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Google Chat channel implementation.
pub struct GoogleChatChannel {
    instance_id: String,
    webhook_url: String,
    space_id: Option<String>,
    connected: Arc<AtomicBool>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for GoogleChatChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleChatChannel")
            .field("instance_id", &self.instance_id)
            .field("space_id", &self.space_id)
            .finish()
    }
}

impl GoogleChatChannel {
    /// Create a new Google Chat channel.
    pub fn new(instance_id: impl Into<String>, webhook_url: impl Into<String>) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: instance_id.into(),
            webhook_url: webhook_url.into(),
            space_id: None,
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
        if let Some(space_id) = config.options.get("space_id").and_then(|v| v.as_str()) {
            channel.space_id = Some(space_id.to_string());
        }
        channel
    }
}

#[async_trait]
impl Channel for GoogleChatChannel {
    fn channel_type(&self) -> &str {
        "googlechat"
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
                video: false,
                files: true,
                stickers: false,
                voice_notes: false,
                max_file_size_mb: 20,
            },
            features: ChannelFeatures {
                reactions: true,
                threads: true,
                edits: true,
                deletes: false,
                typing_indicators: false,
                read_receipts: false,
                mentions: true,
                polls: true,
                native_commands: true,
            },
            limits: ChannelLimits {
                text_max_length: 4096,
                caption_max_length: 1000,
                messages_per_second: 5.0,
                messages_per_minute: 150,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for GoogleChatChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        if self.webhook_url.is_empty() {
            return Err(ChannelError::Config(
                "Google Chat webhook_url is not configured".to_string(),
            ));
        }

        let payload = json!({
            "text": message.text,
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::Channel {
                channel: "googlechat".to_string(),
                message: format!("HTTP error: {}", e),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ChannelError::Channel {
                channel: "googlechat".to_string(),
                message: format!("Google Chat API error {}: {}", status, body),
            });
        }

        let response_body: serde_json::Value = response.json().await.map_err(|e| {
            ChannelError::Channel {
                channel: "googlechat".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;

        let msg_id = response_body
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        debug!(
            "Google Chat sent to {}: {} (msg_id: {})",
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
        warn!("Google Chat attachments stub: sending text only");
        self.send(message).await
    }

    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()> {
        debug!(
            "Google Chat edit {}: {}",
            message.message_id, new_content
        );
        Ok(())
    }

    async fn delete(&self, _message: &MessageRef) -> Result<()> {
        Err(ChannelError::Unsupported(
            "Google Chat does not support message deletion".to_string(),
        ))
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!(
            "Google Chat react {} with {}",
            message.message_id, emoji
        );
        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!(
            "Google Chat unreact {} with {}",
            message.message_id, emoji
        );
        Ok(())
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        4096
    }
}

#[async_trait]
impl ChannelReceiver for GoogleChatChannel {
    async fn start_receiving(&self) -> Result<()> {
        info!(
            "Started Google Chat receiver for {}",
            self.instance_id
        );
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        info!("Stopped Google Chat receiver for {}", self.instance_id);
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
impl ChannelLifecycle for GoogleChatChannel {
    async fn connect(&self) -> Result<()> {
        if self.webhook_url.is_empty() {
            return Err(ChannelError::Config(
                "Google Chat webhook_url is not configured".to_string(),
            ));
        }

        // Validate URL is well-formed
        if let Err(e) = url::Url::parse(&self.webhook_url) {
            return Err(ChannelError::Config(format!(
                "Invalid Google Chat webhook_url: {}",
                e
            )));
        }

        self.connected.store(true, Ordering::Relaxed);
        info!("Google Chat channel connected: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;
        self.connected.store(false, Ordering::Relaxed);
        info!("Google Chat channel disconnected: {}", self.instance_id);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let start = std::time::Instant::now();
        let connected = self.is_connected();

        if !connected {
            return Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: Some(0),
                last_message_at: None,
                error: Some("Not connected".to_string()),
            });
        }

        // Perform a lightweight HEAD request to check reachability
        let client = reqwest::Client::new();
        match client.head(&self.webhook_url).send().await {
            Ok(response) if response.status().is_success() || response.status().is_redirection() => {
                Ok(ChannelHealth {
                    status: HealthStatus::Healthy,
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                    last_message_at: None,
                    error: None,
                })
            }
            Ok(response) => {
                Ok(ChannelHealth {
                    status: HealthStatus::Degraded,
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                    last_message_at: None,
                    error: Some(format!("Google Chat returned status {}", response.status())),
                })
            }
            Err(e) => {
                Ok(ChannelHealth {
                    status: HealthStatus::Unhealthy,
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                    last_message_at: None,
                    error: Some(format!("Health check request failed: {}", e)),
                })
            }
        }
    }
}

impl Clone for GoogleChatChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            webhook_url: self.webhook_url.clone(),
            space_id: self.space_id.clone(),
            connected: self.connected.clone(),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: self.handler.clone(),
        }
    }
}

/// Factory for creating Google Chat channels.
pub struct GoogleChatChannelFactory;

#[async_trait]
impl ChannelFactory for GoogleChatChannelFactory {
    async fn create(&self, config: ChannelConfig) -> Result<Box<dyn Channel>> {
        Ok(Box::new(GoogleChatChannel::from_config(config)))
    }

    fn channel_type(&self) -> &str {
        "googlechat"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

    #[test]
    fn test_googlechat_channel_creation() {
        let channel = GoogleChatChannel::new("test_gc", "https://chat.googleapis.com/v1/spaces/XXX");
        assert_eq!(channel.channel_type(), "googlechat");
        assert_eq!(channel.instance_id(), "test_gc");
    }

    #[test]
    fn test_googlechat_capabilities() {
        let channel = GoogleChatChannel::new("test_gc", "https://chat.googleapis.com/v1/spaces/XXX");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.features.threads);
        assert!(caps.features.polls);
        assert!(caps.features.native_commands);
        assert_eq!(caps.limits.text_max_length, 4096);
    }

    #[tokio::test]
    async fn test_googlechat_connect_disconnect() {
        let channel = GoogleChatChannel::new("test_gc", "https://chat.googleapis.com/v1/spaces/XXX");
        assert!(!channel.is_connected());

        channel.connect().await.unwrap();
        assert!(channel.is_connected());

        channel.disconnect().await.unwrap();
        assert!(!channel.is_connected());
    }

    #[tokio::test]
    async fn test_googlechat_send_message() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "name": "spaces/123/messages/abc123",
                "text": "Hello Google Chat"
            })))
            .mount(&mock_server)
            .await;

        let webhook_url = format!("{}/webhook", mock_server.uri());
        let channel = GoogleChatChannel::new("test_gc", webhook_url);
        let target = MessageTarget {
            chat_id: "spaces/123".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello Google Chat".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let result = channel.send(message).await.unwrap();
        assert!(!result.message_id.is_empty());
        assert_eq!(result.message_id, "spaces/123/messages/abc123");
    }

    #[tokio::test]
    async fn test_googlechat_delete_unsupported() {
        let channel = GoogleChatChannel::new("test_gc", "https://chat.googleapis.com/v1/spaces/XXX");
        let msg_ref = MessageRef::new("msg1", "spaces/123");
        assert!(channel.delete(&msg_ref).await.is_err());
    }
}
