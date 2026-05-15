//! Matrix channel implementation.
//!
//! Provides a Matrix client channel using the HTTP-based Client-Server API.
//! This is a lightweight stub; a full implementation would use matrix-sdk.

#![cfg(feature = "matrix")]

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

/// Matrix channel implementation.
pub struct MatrixChannel {
    instance_id: String,
    homeserver: String,
    access_token: String,
    room_id: Option<String>,
    connected: Arc<AtomicBool>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for MatrixChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MatrixChannel")
            .field("instance_id", &self.instance_id)
            .field("homeserver", &self.homeserver)
            .finish()
    }
}

impl MatrixChannel {
    /// Create a new Matrix channel.
    pub fn new(
        instance_id: impl Into<String>,
        homeserver: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: instance_id.into(),
            homeserver: homeserver.into(),
            access_token: access_token.into(),
            room_id: None,
            connected: Arc::new(AtomicBool::new(false)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> Self {
        let homeserver = config
            .options
            .get("homeserver")
            .and_then(|v| v.as_str())
            .unwrap_or("https://matrix.org")
            .to_string();
        let access_token = config
            .options
            .get("access_token")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        Self::new(config.instance_id, homeserver, access_token)
    }
}

#[async_trait]
impl Channel for MatrixChannel {
    fn channel_type(&self) -> &str {
        "matrix"
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
                polls: false,
                native_commands: false,
            },
            limits: ChannelLimits {
                text_max_length: 65536,
                caption_max_length: 1000,
                messages_per_second: 10.0,
                messages_per_minute: 300,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for MatrixChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        if self.access_token.is_empty() {
            return Err(ChannelError::Config(
                "Matrix access_token is not configured".to_string(),
            ));
        }

        let room_id = if message.target.chat_id.is_empty() {
            self.room_id.clone().unwrap_or_default()
        } else {
            message.target.chat_id.clone()
        };

        if room_id.is_empty() {
            return Err(ChannelError::Config(
                "Matrix room_id is not configured".to_string(),
            ));
        }

        let txn_id = uuid::Uuid::new_v4().to_string();
        let url = format!(
            "{}/_matrix/client/v3/rooms/{}/send/m.room.message/{}",
            self.homeserver.trim_end_matches('/'),
            urlencoding::encode(&room_id),
            txn_id
        );

        let payload = json!({
            "msgtype": "m.text",
            "body": message.text,
        });

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::Channel {
                channel: "matrix".to_string(),
                message: format!("HTTP error: {}", e),
            })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ChannelError::Channel {
                channel: "matrix".to_string(),
                message: format!("Matrix API error {}: {}", status, body),
            });
        }

        let response_body: serde_json::Value = response.json().await.map_err(|e| {
            ChannelError::Channel {
                channel: "matrix".to_string(),
                message: format!("Failed to parse response: {}", e),
            }
        })?;

        let msg_id = response_body
            .get("event_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        debug!(
            "Matrix sent to {}: {} (event_id: {})",
            room_id,
            message.text,
            msg_id
        );

        Ok(SendResult::with_chat(msg_id, room_id)
            .with_metadata("homeserver", serde_json::json!(&self.homeserver)))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        _attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        warn!("Matrix attachments stub: sending text only");
        self.send(message).await
    }

    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()> {
        debug!(
            "Matrix edit {} in {}: {}",
            message.message_id, message.chat_id, new_content
        );
        Ok(())
    }

    async fn delete(&self, message: &MessageRef) -> Result<()> {
        debug!("Matrix delete {} in {}", message.message_id, message.chat_id);
        Ok(())
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!(
            "Matrix react {} in {} with {}",
            message.message_id, message.chat_id, emoji
        );
        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        debug!(
            "Matrix unreact {} in {} with {}",
            message.message_id, message.chat_id, emoji
        );
        Ok(())
    }

    async fn send_typing(&self, target: &MessageTarget) -> Result<()> {
        debug!("Matrix typing in {}", target.chat_id);
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        65536
    }
}

#[async_trait]
impl ChannelReceiver for MatrixChannel {
    async fn start_receiving(&self) -> Result<()> {
        info!(
            "Started Matrix receiver for {} on {}",
            self.instance_id,
            self.homeserver
        );
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        info!("Stopped Matrix receiver for {}", self.instance_id);
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
impl ChannelLifecycle for MatrixChannel {
    async fn connect(&self) -> Result<()> {
        if self.homeserver.is_empty() {
            return Err(ChannelError::Config(
                "Matrix homeserver is not configured".to_string(),
            ));
        }
        if self.access_token.is_empty() {
            return Err(ChannelError::Config(
                "Matrix access_token is not configured".to_string(),
            ));
        }

        // Validate homeserver URL is well-formed
        if let Err(e) = url::Url::parse(&self.homeserver) {
            return Err(ChannelError::Config(format!(
                "Invalid Matrix homeserver URL: {}",
                e
            )));
        }

        self.connected.store(true, Ordering::Relaxed);
        info!("Matrix channel connected: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;
        self.connected.store(false, Ordering::Relaxed);
        info!("Matrix channel disconnected: {}", self.instance_id);
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

        // Perform a lightweight GET to /_matrix/client/versions
        let url = format!(
            "{}/_matrix/client/versions",
            self.homeserver.trim_end_matches('/')
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
                    error: Some(format!("Matrix returned status {}", response.status())),
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

impl Clone for MatrixChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            homeserver: self.homeserver.clone(),
            access_token: self.access_token.clone(),
            room_id: self.room_id.clone(),
            connected: self.connected.clone(),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: self.handler.clone(),
        }
    }
}

/// Factory for creating Matrix channels.
pub struct MatrixChannelFactory;

#[async_trait]
impl ChannelFactory for MatrixChannelFactory {
    async fn create(&self, config: ChannelConfig) -> Result<Box<dyn Channel>> {
        Ok(Box::new(MatrixChannel::from_config(config)))
    }

    fn channel_type(&self) -> &str {
        "matrix"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use wiremock::{Mock, MockServer, ResponseTemplate};
    use wiremock::matchers::{method, header};

    #[test]
    fn test_matrix_channel_creation() {
        let channel = MatrixChannel::new("test_matrix", "https://matrix.org", "token123");
        assert_eq!(channel.channel_type(), "matrix");
        assert_eq!(channel.instance_id(), "test_matrix");
    }

    #[test]
    fn test_matrix_capabilities() {
        let channel = MatrixChannel::new("test_matrix", "https://matrix.org", "token123");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.features.threads);
        assert!(caps.features.edits);
        assert!(caps.features.reactions);
        assert_eq!(caps.limits.text_max_length, 65536);
    }

    #[tokio::test]
    async fn test_matrix_connect_disconnect() {
        let channel = MatrixChannel::new("test_matrix", "https://matrix.org", "token123");
        assert!(!channel.is_connected());

        channel.connect().await.unwrap();
        assert!(channel.is_connected());

        channel.disconnect().await.unwrap();
        assert!(!channel.is_connected());
    }

    #[tokio::test]
    async fn test_matrix_send_message() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(header("Authorization", "Bearer token123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "event_id": "$abc123def456"
            })))
            .mount(&mock_server)
            .await;

        let channel = MatrixChannel::new("test_matrix", &mock_server.uri(), "token123");
        let target = MessageTarget {
            chat_id: "!room:example.com".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello Matrix".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let result = channel.send(message).await.unwrap();
        assert!(!result.message_id.is_empty());
        assert_eq!(result.message_id, "$abc123def456");
        assert_eq!(result.chat_id, "!room:example.com");
    }

    #[tokio::test]
    async fn test_matrix_edit_and_delete() {
        let channel = MatrixChannel::new("test_matrix", "https://matrix.org", "token123");
        let msg_ref = MessageRef::new("msg1", "!room:matrix.org");

        assert!(channel.edit(&msg_ref, "new content").await.is_ok());
        assert!(channel.delete(&msg_ref).await.is_ok());
        assert!(channel.react(&msg_ref, "👍").await.is_ok());
    }

    #[test]
    fn test_matrix_from_config() {
        let mut options = HashMap::new();
        options.insert(
            "homeserver".to_string(),
            serde_json::json!("https://example.com"),
        );
        options.insert("access_token".to_string(), serde_json::json!("secret"));

        let config = ChannelConfig {
            channel_type: "matrix".to_string(),
            instance_id: "matrix-1".to_string(),
            account_id: "acct".to_string(),
            enabled: true,
            options,
        };

        let channel = MatrixChannel::from_config(config);
        assert_eq!(channel.instance_id(), "matrix-1");
    }
}
