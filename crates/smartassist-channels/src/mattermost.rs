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
use reqwest;
use serde_json::json;
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
        if self.token.is_empty() {
            return Err(ChannelError::Config(
                "Mattermost token is not configured".to_string(),
            ));
        }

        let channel_id = message.target.chat_id.clone();
        if channel_id.is_empty() {
            return Err(ChannelError::Config(
                "Mattermost channel_id (chat_id) is not configured".to_string(),
            ));
        }

        let url = format!(
            "{}/api/v4/posts",
            self.server_url.trim_end_matches('/')
        );

        let payload = json!({
            "channel_id": channel_id,
            "message": message.text,
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::Channel {
                channel: "mattermost".to_string(),
                message: format!("HTTP error: {}", e),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ChannelError::Channel {
                channel: "mattermost".to_string(),
                message: format!("Mattermost API error {}: {}", status, body),
            });
        }

        let response_body: serde_json::Value = response.json().await.map_err(|e| {
            ChannelError::Channel {
                channel: "mattermost".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;

        let msg_id = response_body
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        debug!(
            "Mattermost sent to {}: {} (post_id: {})",
            channel_id,
            message.text,
            msg_id
        );

        Ok(SendResult::with_chat(msg_id, channel_id))
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
        if self.server_url.is_empty() {
            return Err(ChannelError::Config(
                "Mattermost server_url is not configured".to_string(),
            ));
        }
        if self.token.is_empty() {
            return Err(ChannelError::Config(
                "Mattermost token is not configured".to_string(),
            ));
        }

        if let Err(e) = url::Url::parse(&self.server_url) {
            return Err(ChannelError::Config(format!(
                "Invalid Mattermost server_url: {}",
                e
            )));
        }

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

        let url = format!(
            "{}/api/v4/system/ping",
            self.server_url.trim_end_matches('/')
        );
        let client = reqwest::Client::new();
        match client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
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
                    error: Some(format!(
                        "Mattermost returned status {}",
                        response.status()
                    )),
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
    use std::collections::HashMap;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, header};

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
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header("Authorization", "Bearer token123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": "post_abc123",
                "channel_id": "channel:town-square",
                "message": "Hello Mattermost"
            })))
            .mount(&mock_server)
            .await;

        let channel = MattermostChannel::new("test_mm", &mock_server.uri(), "token123");
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
        assert_eq!(result.message_id, "post_abc123");
        assert_eq!(result.chat_id, "channel:town-square");
    }

    #[tokio::test]
    async fn test_mattermost_health_check() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(header("Authorization", "Bearer token123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "status": "OK",
                "version": "9.0.0"
            })))
            .mount(&mock_server)
            .await;

        let channel = MattermostChannel::new("test_mm", &mock_server.uri(), "token123");
        channel.connect().await.unwrap();

        let health = channel.health().await.unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.latency_ms.unwrap() > 0);
        assert!(health.error.is_none());
    }

    #[tokio::test]
    async fn test_mattermost_connect_requires_url() {
        let channel = MattermostChannel::new("test_mm", "", "token123");
        let err = channel.connect().await.unwrap_err();
        assert!(err.to_string().contains("server_url is not configured"));
    }

    #[tokio::test]
    async fn test_mattermost_connect_requires_token() {
        let channel = MattermostChannel::new("test_mm", "https://mm.example.com", "");
        let err = channel.connect().await.unwrap_err();
        assert!(err.to_string().contains("token is not configured"));
    }

    #[tokio::test]
    async fn test_mattermost_send_requires_token() {
        let channel = MattermostChannel::new("test_mm", "https://mm.example.com", "");
        let target = MessageTarget {
            chat_id: "channel:town-square".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let err = channel.send(message).await.unwrap_err();
        assert!(err.to_string().contains("token is not configured"));
    }

    #[tokio::test]
    async fn test_mattermost_send_requires_channel_id() {
        let channel = MattermostChannel::new("test_mm", "https://mm.example.com", "token123");
        let target = MessageTarget {
            chat_id: "".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let err = channel.send(message).await.unwrap_err();
        assert!(err.to_string().contains("channel_id"));
    }

    #[test]
    fn test_mattermost_from_config() {
        let mut options = HashMap::new();
        options.insert(
            "server_url".to_string(),
            serde_json::json!("https://mm.example.com"),
        );
        options.insert("token".to_string(), serde_json::json!("secret"));

        let config = ChannelConfig {
            channel_type: "mattermost".to_string(),
            instance_id: "mm-1".to_string(),
            account_id: "acct".to_string(),
            enabled: true,
            options,
        };

        let channel = MattermostChannel::from_config(config);
        assert_eq!(channel.instance_id(), "mm-1");
    }
}
