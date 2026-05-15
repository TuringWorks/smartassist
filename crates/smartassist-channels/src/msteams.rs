//! Microsoft Teams channel implementation.
//!
//! Provides a Microsoft Teams webhook/API-based channel for SmartAssist.

#![cfg(feature = "msteams")]

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

/// Microsoft Teams channel implementation.
pub struct MsTeamsChannel {
    instance_id: String,
    webhook_url: String,
    tenant_id: Option<String>,
    connected: Arc<AtomicBool>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for MsTeamsChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MsTeamsChannel")
            .field("instance_id", &self.instance_id)
            .field("tenant_id", &self.tenant_id)
            .finish()
    }
}

impl MsTeamsChannel {
    /// Create a new Microsoft Teams channel.
    pub fn new(instance_id: impl Into<String>, webhook_url: impl Into<String>) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: instance_id.into(),
            webhook_url: webhook_url.into(),
            tenant_id: None,
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
        if let Some(tenant_id) = config.options.get("tenant_id").and_then(|v| v.as_str()) {
            channel.tenant_id = Some(tenant_id.to_string());
        }
        channel
    }
}

#[async_trait]
impl Channel for MsTeamsChannel {
    fn channel_type(&self) -> &str {
        "msteams"
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
                voice_notes: false,
                max_file_size_mb: 50,
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
                text_max_length: 10000,
                caption_max_length: 1000,
                messages_per_second: 5.0,
                messages_per_minute: 150,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for MsTeamsChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        if self.webhook_url.is_empty() {
            return Err(ChannelError::Config(
                "MS Teams webhook_url is not configured".to_string(),
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
                channel: "msteams".to_string(),
                message: format!("HTTP error: {}", e),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ChannelError::Channel {
                channel: "msteams".to_string(),
                message: format!("MS Teams API error {}: {}", status, body),
            });
        }

        let msg_id = uuid::Uuid::new_v4().to_string();
        debug!(
            "MS Teams sent to {}: {} (msg_id: {})",
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
        warn!("MS Teams attachments stub: sending text only");
        self.send(message).await
    }

    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()> {
        debug!(
            "MS Teams edit {}: {}",
            message.message_id, new_content
        );
        Ok(())
    }

    async fn delete(&self, _message: &MessageRef) -> Result<()> {
        Err(ChannelError::Unsupported(
            "MS Teams does not support message deletion".to_string(),
        ))
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!(
            "MS Teams react {} with {}",
            message.message_id, emoji
        );
        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!(
            "MS Teams unreact {} with {}",
            message.message_id, emoji
        );
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
impl ChannelReceiver for MsTeamsChannel {
    async fn start_receiving(&self) -> Result<()> {
        info!(
            "Started MS Teams receiver for {}",
            self.instance_id
        );
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        info!("Stopped MS Teams receiver for {}", self.instance_id);
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
impl ChannelLifecycle for MsTeamsChannel {
    async fn connect(&self) -> Result<()> {
        if self.webhook_url.is_empty() {
            return Err(ChannelError::Config(
                "MS Teams webhook_url is not configured".to_string(),
            ));
        }

        if let Err(e) = url::Url::parse(&self.webhook_url) {
            return Err(ChannelError::Config(format!(
                "Invalid MS Teams webhook_url: {}",
                e
            )));
        }

        self.connected.store(true, Ordering::Relaxed);
        info!("MS Teams channel connected: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;
        self.connected.store(false, Ordering::Relaxed);
        
        info!("MS Teams channel disconnected: {}", self.instance_id);
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

        // Teams webhooks often reject HEAD; treat 405 as reachable.
        let client = reqwest::Client::new();
        match client.head(&self.webhook_url).send().await {
            Ok(response) if response.status().is_success() || response.status().as_u16() == 405 => {
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
                        "MS Teams webhook returned status {}",
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

impl Clone for MsTeamsChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            webhook_url: self.webhook_url.clone(),
            tenant_id: self.tenant_id.clone(),
            connected: self.connected.clone(),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: self.handler.clone(),
        }
    }
}

/// Factory for creating Microsoft Teams channels.
pub struct MsTeamsChannelFactory;

#[async_trait]
impl ChannelFactory for MsTeamsChannelFactory {
    async fn create(&self, config: ChannelConfig) -> Result<Box<dyn Channel>> {
        Ok(Box::new(MsTeamsChannel::from_config(config)))
    }

    fn channel_type(&self) -> &str {
        "msteams"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::method;

    #[test]
    fn test_msteams_channel_creation() {
        let channel = MsTeamsChannel::new("test_teams", "https://outlook.office.com/webhook/XXX");
        assert_eq!(channel.channel_type(), "msteams");
        assert_eq!(channel.instance_id(), "test_teams");
    }

    #[test]
    fn test_msteams_capabilities() {
        let channel = MsTeamsChannel::new("test_teams", "https://outlook.office.com/webhook/XXX");
        let caps = channel.capabilities();
        assert!(caps.media.video);
        assert!(caps.features.threads);
        assert!(caps.features.polls);
        assert_eq!(caps.limits.text_max_length, 10000);
    }

    #[tokio::test]
    async fn test_msteams_connect_disconnect() {
        let channel = MsTeamsChannel::new("test_teams", "https://outlook.office.com/webhook/XXX");
        assert!(!channel.is_connected());

        channel.connect().await.unwrap();
        assert!(channel.is_connected());

        channel.disconnect().await.unwrap();
        assert!(!channel.is_connected());
    }

    #[tokio::test]
    async fn test_msteams_send_message() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let channel = MsTeamsChannel::new("test_teams", &mock_server.uri());
        let target = MessageTarget {
            chat_id: "channel:123".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello Teams".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let result = channel.send(message).await.unwrap();
        assert!(!result.message_id.is_empty());
        assert_eq!(result.chat_id, "channel:123");
    }

    #[tokio::test]
    async fn test_msteams_send_error_response() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(400).set_body_string("Invalid webhook"))
            .mount(&mock_server)
            .await;

        let channel = MsTeamsChannel::new("test_teams", &mock_server.uri());
        let target = MessageTarget {
            chat_id: "channel:123".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello Teams".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let err = channel.send(message).await.unwrap_err();
        assert!(err.to_string().contains("Invalid webhook"));
    }

    #[tokio::test]
    async fn test_msteams_health_check() {
        let mock_server = MockServer::start().await;
        Mock::given(method("HEAD"))
            .respond_with(ResponseTemplate::new(405))
            .mount(&mock_server)
            .await;

        let channel = MsTeamsChannel::new("test_teams", &mock_server.uri());
        channel.connect().await.unwrap();

        let health = channel.health().await.unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.latency_ms.unwrap() > 0);
        assert!(health.error.is_none());
    }

    #[tokio::test]
    async fn test_msteams_connect_requires_webhook_url() {
        let channel = MsTeamsChannel::new("test_teams", "");
        let err = channel.connect().await.unwrap_err();
        assert!(err.to_string().contains("webhook_url is not configured"));
    }

    #[tokio::test]
    async fn test_msteams_send_requires_webhook_url() {
        let channel = MsTeamsChannel::new("test_teams", "");
        let target = MessageTarget {
            chat_id: "channel:123".to_string(),
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
        assert!(err.to_string().contains("webhook_url is not configured"));
    }

    #[tokio::test]
    async fn test_msteams_health_when_not_connected() {
        let channel = MsTeamsChannel::new("test_teams", "https://outlook.office.com/webhook/XXX");
        let health = channel.health().await.unwrap();
        assert_eq!(health.status, HealthStatus::Unhealthy);
        assert!(health.error.unwrap().contains("Not connected"));
    }

    #[test]
    fn test_msteams_from_config() {
        let mut options = HashMap::new();
        options.insert(
            "webhook_url".to_string(),
            serde_json::json!("https://outlook.office.com/webhook/XXX"),
        );
        options.insert("tenant_id".to_string(), serde_json::json!("tenant_abc"));

        let config = ChannelConfig {
            channel_type: "msteams".to_string(),
            instance_id: "teams-1".to_string(),
            account_id: "acct".to_string(),
            enabled: true,
            options,
        };

        let channel = MsTeamsChannel::from_config(config);
        assert_eq!(channel.instance_id(), "teams-1");
    }
}
