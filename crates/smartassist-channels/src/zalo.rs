//! Zalo channel implementation.
//!
//! Provides a Zalo Official Account API-based channel for SmartAssist.

#![cfg(feature = "zalo")]

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

/// Zalo channel implementation.
pub struct ZaloChannel {
    instance_id: String,
    oa_id: String,
    access_token: String,
    api_base: String,
    connected: Arc<AtomicBool>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for ZaloChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ZaloChannel")
            .field("instance_id", &self.instance_id)
            .field("oa_id", &self.oa_id)
            .finish()
    }
}

impl ZaloChannel {
    /// Create a new Zalo channel.
    pub fn new(
        instance_id: impl Into<String>,
        oa_id: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: instance_id.into(),
            oa_id: oa_id.into(),
            access_token: access_token.into(),
            api_base: "https://openapi.zalo.me".to_string(),
            connected: Arc::new(AtomicBool::new(false)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: Arc::new(RwLock::new(None)),
        }
    }

    /// Override the API base URL (used primarily for testing).
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> Self {
        let oa_id = config
            .options
            .get("oa_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let access_token = config
            .options
            .get("access_token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let mut channel = Self::new(config.instance_id, oa_id, access_token);
        if let Some(api_base) = config.options.get("api_base").and_then(|v| v.as_str()) {
            channel.api_base = api_base.to_string();
        }
        channel
    }
}

#[async_trait]
impl Channel for ZaloChannel {
    fn channel_type(&self) -> &str {
        "zalo"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: true,
                voice_notes: true,
                max_file_size_mb: 20,
            },
            features: ChannelFeatures {
                reactions: false,
                threads: false,
                edits: false,
                deletes: false,
                typing_indicators: false,
                read_receipts: false,
                mentions: false,
                polls: false,
                native_commands: false,
            },
            limits: ChannelLimits {
                text_max_length: 2000,
                caption_max_length: 500,
                messages_per_second: 5.0,
                messages_per_minute: 150,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for ZaloChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        if self.access_token.is_empty() {
            return Err(ChannelError::Config(
                "Zalo access_token is not configured".to_string(),
            ));
        }

        let user_id = message.target.chat_id.clone();
        if user_id.is_empty() {
            return Err(ChannelError::Config(
                "Zalo user_id (chat_id) is not configured".to_string(),
            ));
        }

        let url = format!(
            "{}/v3.0/oa/message/cs?access_token={}",
            self.api_base.trim_end_matches('/'),
            urlencoding::encode(&self.access_token)
        );

        let payload = json!({
            "recipient": {
                "user_id": user_id,
            },
            "message": {
                "text": message.text,
            },
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::Channel {
                channel: "zalo".to_string(),
                message: format!("HTTP error: {}", e),
            })?;

        let status = response.status();
        let response_body: serde_json::Value = response.json().await.map_err(|e| {
            ChannelError::Channel {
                channel: "zalo".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;

        let error_code = response_body.get("error").and_then(|v| v.as_i64()).unwrap_or(-1);
        if !status.is_success() || error_code != 0 {
            let msg = response_body
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown error");
            return Err(ChannelError::Channel {
                channel: "zalo".to_string(),
                message: format!("Zalo API error (HTTP {} / error {}): {}", status, error_code, msg),
            });
        }

        let msg_id = response_body
            .get("data")
            .and_then(|v| v.get("message_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        debug!(
            "Zalo sent to {}: {} (msg_id: {})",
            user_id,
            message.text,
            msg_id
        );

        Ok(SendResult::with_chat(msg_id, user_id))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        _attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        warn!("Zalo attachments stub: sending text only");
        self.send(message).await
    }

    async fn edit(&self, _message: &MessageRef, _new_content: &str) -> Result<()> {
        Err(ChannelError::Unsupported("Zalo does not support message editing".to_string()))
    }

    async fn delete(&self, _message: &MessageRef) -> Result<()> {
        Err(ChannelError::Unsupported("Zalo does not support message deletion".to_string()))
    }

    async fn react(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        Err(ChannelError::Unsupported("Zalo does not support reactions".to_string()))
    }

    async fn unreact(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        Err(ChannelError::Unsupported("Zalo does not support reactions".to_string()))
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        2000
    }
}

#[async_trait]
impl ChannelReceiver for ZaloChannel {
    async fn start_receiving(&self) -> Result<()> {
        info!("Started Zalo receiver for {}", self.instance_id);
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        info!("Stopped Zalo receiver for {}", self.instance_id);
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
impl ChannelLifecycle for ZaloChannel {
    async fn connect(&self) -> Result<()> {
        if self.oa_id.is_empty() {
            return Err(ChannelError::Config(
                "Zalo oa_id is not configured".to_string(),
            ));
        }
        if self.access_token.is_empty() {
            return Err(ChannelError::Config(
                "Zalo access_token is not configured".to_string(),
            ));
        }

        self.connected.store(true, Ordering::Relaxed);
        info!("Zalo channel connected: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;
        self.connected.store(false, Ordering::Relaxed);
        
        info!("Zalo channel disconnected: {}", self.instance_id);
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
            "{}/v3.0/oa/getoa?access_token={}",
            self.api_base.trim_end_matches('/'),
            urlencoding::encode(&self.access_token)
        );
        let client = reqwest::Client::new();
        match client.get(&url).send().await {
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
                        "Zalo returned status {}",
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

impl Clone for ZaloChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            oa_id: self.oa_id.clone(),
            access_token: self.access_token.clone(),
            api_base: self.api_base.clone(),
            connected: self.connected.clone(),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: self.handler.clone(),
        }
    }
}

/// Factory for creating Zalo channels.
pub struct ZaloChannelFactory;

#[async_trait]
impl ChannelFactory for ZaloChannelFactory {
    async fn create(&self, config: ChannelConfig) -> Result<Box<dyn Channel>> {
        Ok(Box::new(ZaloChannel::from_config(config)))
    }

    fn channel_type(&self) -> &str {
        "zalo"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, path, query_param};

    #[test]
    fn test_zalo_channel_creation() {
        let channel = ZaloChannel::new("test_zalo", "oa_123", "token123");
        assert_eq!(channel.channel_type(), "zalo");
        assert_eq!(channel.instance_id(), "test_zalo");
    }

    #[test]
    fn test_zalo_capabilities() {
        let channel = ZaloChannel::new("test_zalo", "oa_123", "token123");
        let caps = channel.capabilities();
        assert!(caps.media.stickers);
        assert!(caps.media.voice_notes);
        assert!(!caps.features.edits);
        assert_eq!(caps.limits.text_max_length, 2000);
    }

    #[tokio::test]
    async fn test_zalo_connect_disconnect() {
        let channel = ZaloChannel::new("test_zalo", "oa_123", "token123");
        assert!(!channel.is_connected());

        channel.connect().await.unwrap();
        assert!(channel.is_connected());

        channel.disconnect().await.unwrap();
        assert!(!channel.is_connected());
    }

    #[tokio::test]
    async fn test_zalo_send_message() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v3.0/oa/message/cs"))
            .and(query_param("access_token", "token123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "message_id": "zalo_msg_123"
                },
                "error": 0,
                "message": "Success"
            })))
            .mount(&mock_server)
            .await;

        let channel = ZaloChannel::new("test_zalo", "oa_123", "token123")
            .with_api_base(&mock_server.uri());
        let target = MessageTarget {
            chat_id: "user_456".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello Zalo".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let result = channel.send(message).await.unwrap();
        assert!(!result.message_id.is_empty());
        assert_eq!(result.message_id, "zalo_msg_123");
        assert_eq!(result.chat_id, "user_456");
    }

    #[tokio::test]
    async fn test_zalo_send_error_response() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v3.0/oa/message/cs"))
            .and(query_param("access_token", "token123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "error": -201,
                "message": "Invalid access token"
            })))
            .mount(&mock_server)
            .await;

        let channel = ZaloChannel::new("test_zalo", "oa_123", "token123")
            .with_api_base(&mock_server.uri());
        let target = MessageTarget {
            chat_id: "user_456".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello Zalo".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let err = channel.send(message).await.unwrap_err();
        assert!(err.to_string().contains("Invalid access token"));
    }

    #[tokio::test]
    async fn test_zalo_health_check() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v3.0/oa/getoa"))
            .and(query_param("access_token", "token123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": { "oa_id": "oa_123" },
                "error": 0,
                "message": "Success"
            })))
            .mount(&mock_server)
            .await;

        let channel = ZaloChannel::new("test_zalo", "oa_123", "token123")
            .with_api_base(&mock_server.uri());
        channel.connect().await.unwrap();

        let health = channel.health().await.unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.latency_ms.unwrap() > 0);
        assert!(health.error.is_none());
    }

    #[tokio::test]
    async fn test_zalo_connect_requires_oa_id() {
        let channel = ZaloChannel::new("test_zalo", "", "token123");
        let err = channel.connect().await.unwrap_err();
        assert!(err.to_string().contains("oa_id is not configured"));
    }

    #[tokio::test]
    async fn test_zalo_connect_requires_access_token() {
        let channel = ZaloChannel::new("test_zalo", "oa_123", "");
        let err = channel.connect().await.unwrap_err();
        assert!(err.to_string().contains("access_token is not configured"));
    }

    #[tokio::test]
    async fn test_zalo_send_requires_access_token() {
        let channel = ZaloChannel::new("test_zalo", "oa_123", "");
        let target = MessageTarget {
            chat_id: "user_456".to_string(),
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
        assert!(err.to_string().contains("access_token is not configured"));
    }

    #[tokio::test]
    async fn test_zalo_send_requires_user_id() {
        let channel = ZaloChannel::new("test_zalo", "oa_123", "token123");
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
        assert!(err.to_string().contains("user_id"));
    }

    #[tokio::test]
    async fn test_zalo_unsupported_operations() {
        let channel = ZaloChannel::new("test_zalo", "oa_123", "token123");
        let msg_ref = MessageRef::new("msg1", "user_456");

        assert!(channel.edit(&msg_ref, "new").await.is_err());
        assert!(channel.delete(&msg_ref).await.is_err());
        assert!(channel.react(&msg_ref, "👍").await.is_err());
    }

    #[test]
    fn test_zalo_from_config() {
        let mut options = HashMap::new();
        options.insert("oa_id".to_string(), serde_json::json!("oa_123"));
        options.insert("access_token".to_string(), serde_json::json!("secret"));
        options.insert("api_base".to_string(), serde_json::json!("https://custom.zalo.api"));

        let config = ChannelConfig {
            channel_type: "zalo".to_string(),
            instance_id: "zalo-1".to_string(),
            account_id: "acct".to_string(),
            enabled: true,
            options,
        };

        let channel = ZaloChannel::from_config(config);
        assert_eq!(channel.instance_id(), "zalo-1");
    }
}
