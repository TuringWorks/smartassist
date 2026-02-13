//! LINE channel implementation via LINE Messaging API.
//!
//! This module provides a LINE channel adapter that supports:
//! - Webhook-based message reception
//! - Push and reply message sending
//! - Rich message templates (buttons, carousel)
//! - Sticker and media support

#![cfg(feature = "line")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver, ChannelSender, MessageHandler,
    MessageRef, SendResult,
};
use crate::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use smartassist_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatInfo, ChatType,
    HealthStatus, InboundMessage, MediaAttachment, MediaCapabilities, MediaType, MessageId,
    MessageTarget, OutboundMessage, SenderInfo,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

const LINE_API_BASE: &str = "https://api.line.me/v2/bot";
const LINE_DATA_API_BASE: &str = "https://api-data.line.me/v2/bot";

/// LINE channel implementation.
pub struct LineChannel {
    /// Channel access token.
    access_token: String,

    /// Channel secret for webhook signature verification.
    channel_secret: String,

    /// Channel ID.
    channel_id: String,

    /// Channel instance ID.
    instance_id: String,

    /// HTTP client.
    client: Client,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Incoming message channel.
    #[allow(dead_code)]
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for LineChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LineChannel")
            .field("instance_id", &self.instance_id)
            .field("channel_id", &self.channel_id)
            .finish()
    }
}

impl LineChannel {
    /// Create a new LINE channel.
    pub fn new(
        access_token: impl Into<String>,
        channel_secret: impl Into<String>,
        channel_id: impl Into<String>,
        instance_id: impl Into<String>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        Self {
            access_token: access_token.into(),
            channel_secret: channel_secret.into(),
            channel_id: channel_id.into(),
            instance_id: instance_id.into(),
            client: Client::new(),
            connected: Arc::new(RwLock::new(false)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(
        config: ChannelConfig,
        access_token: String,
        channel_secret: String,
    ) -> Self {
        let channel_id = config
            .options
            .get("channel_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&config.account_id)
            .to_string();

        Self::new(access_token, channel_secret, channel_id, config.instance_id)
    }

    /// Verify webhook signature.
    pub fn verify_signature(&self, body: &[u8], signature: &str) -> bool {
        type HmacSha256 = Hmac<Sha256>;

        let Ok(mut mac) = HmacSha256::new_from_slice(self.channel_secret.as_bytes()) else {
            return false;
        };

        mac.update(body);
        let expected = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            mac.finalize().into_bytes(),
        );

        expected == signature
    }

    /// Handle incoming webhook payload.
    pub async fn handle_webhook(&self, payload: LineWebhook) -> Result<()> {
        for event in payload.events {
            match event {
                LineEvent::Message(msg_event) => {
                    let inbound = self.convert_message(msg_event);

                    // Call handler if set
                    {
                        let handler_guard = self.handler.read().await;
                        if let Some(ref h) = *handler_guard {
                            if let Err(e) = h.handle(inbound.clone()).await {
                                warn!("Handler error: {}", e);
                            }
                        }
                    }

                    // Send through channel
                    if let Err(e) = self.message_tx.send(inbound).await {
                        warn!("Failed to send message to channel: {}", e);
                    }
                }
                LineEvent::Follow(_) => {
                    debug!("User followed the LINE bot");
                }
                LineEvent::Unfollow(_) => {
                    debug!("User unfollowed the LINE bot");
                }
                LineEvent::Join(_) => {
                    debug!("Bot joined a group");
                }
                LineEvent::Leave(_) => {
                    debug!("Bot left a group");
                }
                LineEvent::Postback(postback) => {
                    debug!("Received postback: {:?}", postback.postback.data);
                }
            }
        }

        Ok(())
    }

    /// Convert LINE message event to InboundMessage.
    fn convert_message(&self, event: MessageEvent) -> InboundMessage {
        let (sender_id, chat_id, chat_type) = match &event.source {
            EventSource::User { user_id } => (user_id.clone(), user_id.clone(), ChatType::Direct),
            EventSource::Group { group_id, user_id } => (
                user_id.clone().unwrap_or_default(),
                group_id.clone(),
                ChatType::Group,
            ),
            EventSource::Room { room_id, user_id } => (
                user_id.clone().unwrap_or_default(),
                room_id.clone(),
                ChatType::Group,
            ),
        };

        let (text, media, message_id) = match &event.message {
            LineMessage::Text { id, text } => (text.clone(), vec![], id.clone()),
            LineMessage::Image { id, .. } => (
                String::new(),
                vec![MediaAttachment {
                    id: id.clone(),
                    media_type: MediaType::Image,
                    url: None,
                    data: None,
                    filename: None,
                    size_bytes: None,
                    mime_type: Some("image/jpeg".to_string()),
                }],
                id.clone(),
            ),
            LineMessage::Video { id, .. } => (
                String::new(),
                vec![MediaAttachment {
                    id: id.clone(),
                    media_type: MediaType::Video,
                    url: None,
                    data: None,
                    filename: None,
                    size_bytes: None,
                    mime_type: Some("video/mp4".to_string()),
                }],
                id.clone(),
            ),
            LineMessage::Audio { id, .. } => (
                String::new(),
                vec![MediaAttachment {
                    id: id.clone(),
                    media_type: MediaType::Audio,
                    url: None,
                    data: None,
                    filename: None,
                    size_bytes: None,
                    mime_type: Some("audio/m4a".to_string()),
                }],
                id.clone(),
            ),
            LineMessage::File {
                id,
                file_name,
                file_size,
            } => (
                String::new(),
                vec![MediaAttachment {
                    id: id.clone(),
                    media_type: MediaType::Document,
                    url: None,
                    data: None,
                    filename: Some(file_name.clone()),
                    size_bytes: Some(*file_size),
                    mime_type: None,
                }],
                id.clone(),
            ),
            LineMessage::Location {
                id,
                title,
                address,
                ..
            } => (format!("{}\n{}", title, address), vec![], id.clone()),
            LineMessage::Sticker {
                id,
                package_id,
                sticker_id,
            } => (
                format!("[Sticker: {}/{}]", package_id, sticker_id),
                vec![],
                id.clone(),
            ),
        };

        let timestamp = DateTime::from_timestamp_millis(event.timestamp as i64)
            .unwrap_or_else(Utc::now);

        InboundMessage {
            id: MessageId::new(message_id),
            timestamp,
            channel: "line".to_string(),
            account_id: self.instance_id.clone(),
            sender: SenderInfo {
                id: sender_id,
                username: None,
                display_name: None,
                phone_number: None,
                is_bot: false,
            },
            chat: ChatInfo {
                id: chat_id,
                chat_type,
                title: None,
                guild_id: None,
            },
            text,
            media,
            quote: None,
            thread: None,
            metadata: serde_json::json!({
                "reply_token": event.reply_token,
            }),
        }
    }

    /// Send a reply message using reply token.
    pub async fn reply(
        &self,
        reply_token: &str,
        messages: Vec<LineOutboundMessage>,
    ) -> Result<()> {
        let body = serde_json::json!({
            "replyToken": reply_token,
            "messages": messages,
        });

        self.client
            .post(format!("{}/message/reply", LINE_API_BASE))
            .bearer_auth(&self.access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| ChannelError::channel("line", e.to_string()))?
            .error_for_status()
            .map_err(|e| ChannelError::channel("line", e.to_string()))?;

        Ok(())
    }

    /// Send a push message to a user/group.
    pub async fn push(&self, to: &str, messages: Vec<LineOutboundMessage>) -> Result<String> {
        let body = serde_json::json!({
            "to": to,
            "messages": messages,
        });

        let response = self
            .client
            .post(format!("{}/message/push", LINE_API_BASE))
            .bearer_auth(&self.access_token)
            .json(&body)
            .send()
            .await
            .map_err(|e| ChannelError::channel("line", e.to_string()))?
            .error_for_status()
            .map_err(|e| ChannelError::channel("line", e.to_string()))?;

        // LINE API returns request ID in x-line-request-id header
        let request_id = response
            .headers()
            .get("x-line-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        Ok(request_id)
    }

    /// Get user profile.
    pub async fn get_profile(&self, user_id: &str) -> Result<LineProfile> {
        let response = self
            .client
            .get(format!("{}/profile/{}", LINE_API_BASE, user_id))
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| ChannelError::channel("line", e.to_string()))?
            .error_for_status()
            .map_err(|e| ChannelError::channel("line", e.to_string()))?;

        response
            .json()
            .await
            .map_err(|e| ChannelError::channel("line", e.to_string()))
    }

    /// Get message content (for media messages).
    pub async fn get_content(&self, message_id: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(format!(
                "{}/message/{}/content",
                LINE_DATA_API_BASE, message_id
            ))
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| ChannelError::channel("line", e.to_string()))?
            .error_for_status()
            .map_err(|e| ChannelError::channel("line", e.to_string()))?;

        response
            .bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| ChannelError::channel("line", e.to_string()))
    }

    /// Build outbound messages from OutboundMessage.
    fn build_messages(&self, message: &OutboundMessage) -> Vec<LineOutboundMessage> {
        use smartassist_core::types::MediaSource;

        let mut messages = vec![];

        // Add text message if present
        if !message.text.is_empty() {
            messages.push(LineOutboundMessage::Text {
                text: message.text.clone(),
            });
        }

        // Add media messages
        for media in &message.media {
            // Extract URL from MediaSource
            let url = match &media.source {
                MediaSource::Url(u) => Some(u.clone()),
                _ => None,
            };

            if let Some(url) = url {
                match media.media_type {
                    MediaType::Image => {
                        messages.push(LineOutboundMessage::Image {
                            original_content_url: url.clone(),
                            preview_image_url: url.clone(),
                        });
                    }
                    MediaType::Video => {
                        messages.push(LineOutboundMessage::Video {
                            original_content_url: url.clone(),
                            preview_image_url: url.clone(), // Should be a thumbnail
                        });
                    }
                    MediaType::Audio => {
                        messages.push(LineOutboundMessage::Audio {
                            original_content_url: url.clone(),
                            duration: 60000, // Default duration
                        });
                    }
                    _ => {
                        // For documents, send as text with URL
                        messages.push(LineOutboundMessage::Text {
                            text: format!(
                                "[File: {}]",
                                media.filename.as_deref().unwrap_or(&url)
                            ),
                        });
                    }
                }
            }
        }

        messages
    }
}

#[async_trait]
impl Channel for LineChannel {
    fn channel_type(&self) -> &str {
        "line"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct, ChatType::Group],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: true,
                voice_notes: false,
                max_file_size_mb: 300,
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
                native_commands: false,
            },
            limits: ChannelLimits {
                text_max_length: 5000,
                caption_max_length: 2000,
                messages_per_second: 10.0,
                messages_per_minute: 100,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for LineChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to LINE".to_string()));
        }

        let messages = self.build_messages(&message);
        if messages.is_empty() {
            return Err(ChannelError::InvalidMessage(
                "No content to send".to_string(),
            ));
        }

        // LINE uses push API by default
        // Reply tokens must be used via the reply() method directly
        let request_id = self.push(&message.target.chat_id, messages).await?;
        Ok(SendResult::with_chat(request_id, &message.target.chat_id))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        _attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        // LINE requires URLs for media, so attachments would need to be uploaded first
        warn!("LINE attachment uploads require external hosting - using message media URLs");
        self.send(message).await
    }

    async fn edit(&self, _message: &MessageRef, _new_content: &str) -> Result<()> {
        // LINE doesn't support message editing
        warn!("LINE does not support message editing");
        Err(ChannelError::Internal(
            "LINE does not support message editing".to_string(),
        ))
    }

    async fn delete(&self, _message: &MessageRef) -> Result<()> {
        // LINE doesn't support message deletion
        warn!("LINE does not support message deletion");
        Err(ChannelError::Internal(
            "LINE does not support message deletion".to_string(),
        ))
    }

    async fn react(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        // LINE doesn't support reactions
        warn!("LINE does not support reactions");
        Err(ChannelError::Internal(
            "LINE does not support reactions".to_string(),
        ))
    }

    async fn unreact(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        warn!("LINE does not support reactions");
        Err(ChannelError::Internal(
            "LINE does not support reactions".to_string(),
        ))
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        // LINE doesn't have a typing indicator API
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        5000
    }
}

#[async_trait]
impl ChannelReceiver for LineChannel {
    async fn start_receiving(&self) -> Result<()> {
        // LINE uses webhooks, so receiving is handled by handle_webhook()
        info!(
            "LINE channel {} ready to receive webhooks",
            self.instance_id
        );
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
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
impl ChannelLifecycle for LineChannel {
    async fn connect(&self) -> Result<()> {
        // Verify token by getting bot info
        let response = self
            .client
            .get(format!("{}/info", LINE_API_BASE))
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| ChannelError::Auth(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ChannelError::Auth("Invalid LINE access token".to_string()));
        }

        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Connected to LINE channel: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        let mut connected = self.connected.write().await;
        *connected = false;

        info!("Disconnected from LINE channel: {}", self.instance_id);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let start = std::time::Instant::now();

        let response = self
            .client
            .get(format!("{}/info", LINE_API_BASE))
            .bearer_auth(&self.access_token)
            .send()
            .await;

        match response {
            Ok(r) if r.status().is_success() => Ok(ChannelHealth {
                status: HealthStatus::Healthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                last_message_at: None,
                error: None,
            }),
            Ok(r) => Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                last_message_at: None,
                error: Some(format!("HTTP {}", r.status())),
            }),
            Err(e) => Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: None,
                last_message_at: None,
                error: Some(e.to_string()),
            }),
        }
    }
}

impl Clone for LineChannel {
    fn clone(&self) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            access_token: self.access_token.clone(),
            channel_secret: self.channel_secret.clone(),
            channel_id: self.channel_id.clone(),
            instance_id: self.instance_id.clone(),
            client: self.client.clone(),
            connected: self.connected.clone(),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: self.handler.clone(),
        }
    }
}

// --- LINE API Types ---

/// LINE webhook payload.
#[derive(Debug, Deserialize)]
pub struct LineWebhook {
    /// Destination user ID (bot's user ID).
    pub destination: String,
    /// List of events.
    pub events: Vec<LineEvent>,
}

/// LINE event types.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum LineEvent {
    #[serde(rename = "message")]
    Message(MessageEvent),
    #[serde(rename = "follow")]
    Follow(FollowEvent),
    #[serde(rename = "unfollow")]
    Unfollow(UnfollowEvent),
    #[serde(rename = "join")]
    Join(JoinEvent),
    #[serde(rename = "leave")]
    Leave(LeaveEvent),
    #[serde(rename = "postback")]
    Postback(PostbackEvent),
}

/// Message event.
#[derive(Debug, Deserialize)]
pub struct MessageEvent {
    #[serde(rename = "replyToken")]
    pub reply_token: String,
    pub source: EventSource,
    pub timestamp: u64,
    pub message: LineMessage,
}

/// Follow event.
#[derive(Debug, Deserialize)]
pub struct FollowEvent {
    #[serde(rename = "replyToken")]
    pub reply_token: String,
    pub source: EventSource,
    pub timestamp: u64,
}

/// Unfollow event.
#[derive(Debug, Deserialize)]
pub struct UnfollowEvent {
    pub source: EventSource,
    pub timestamp: u64,
}

/// Join event.
#[derive(Debug, Deserialize)]
pub struct JoinEvent {
    #[serde(rename = "replyToken")]
    pub reply_token: String,
    pub source: EventSource,
    pub timestamp: u64,
}

/// Leave event.
#[derive(Debug, Deserialize)]
pub struct LeaveEvent {
    pub source: EventSource,
    pub timestamp: u64,
}

/// Postback event.
#[derive(Debug, Deserialize)]
pub struct PostbackEvent {
    #[serde(rename = "replyToken")]
    pub reply_token: String,
    pub source: EventSource,
    pub timestamp: u64,
    pub postback: Postback,
}

/// Postback data.
#[derive(Debug, Deserialize)]
pub struct Postback {
    pub data: String,
}

/// Event source.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum EventSource {
    #[serde(rename = "user")]
    User {
        #[serde(rename = "userId")]
        user_id: String,
    },
    #[serde(rename = "group")]
    Group {
        #[serde(rename = "groupId")]
        group_id: String,
        #[serde(rename = "userId")]
        user_id: Option<String>,
    },
    #[serde(rename = "room")]
    Room {
        #[serde(rename = "roomId")]
        room_id: String,
        #[serde(rename = "userId")]
        user_id: Option<String>,
    },
}

/// LINE message types.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum LineMessage {
    #[serde(rename = "text")]
    Text { id: String, text: String },
    #[serde(rename = "image")]
    Image {
        id: String,
        #[serde(rename = "contentProvider")]
        content_provider: ContentProvider,
    },
    #[serde(rename = "video")]
    Video {
        id: String,
        duration: u64,
        #[serde(rename = "contentProvider")]
        content_provider: ContentProvider,
    },
    #[serde(rename = "audio")]
    Audio {
        id: String,
        duration: u64,
        #[serde(rename = "contentProvider")]
        content_provider: ContentProvider,
    },
    #[serde(rename = "file")]
    File {
        id: String,
        #[serde(rename = "fileName")]
        file_name: String,
        #[serde(rename = "fileSize")]
        file_size: u64,
    },
    #[serde(rename = "location")]
    Location {
        id: String,
        title: String,
        address: String,
        latitude: f64,
        longitude: f64,
    },
    #[serde(rename = "sticker")]
    Sticker {
        id: String,
        #[serde(rename = "packageId")]
        package_id: String,
        #[serde(rename = "stickerId")]
        sticker_id: String,
    },
}

/// Content provider info.
#[derive(Debug, Deserialize)]
pub struct ContentProvider {
    #[serde(rename = "type")]
    pub provider_type: String,
    #[serde(rename = "originalContentUrl")]
    pub original_content_url: Option<String>,
    #[serde(rename = "previewImageUrl")]
    pub preview_image_url: Option<String>,
}

/// Outbound message types.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum LineOutboundMessage {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image {
        #[serde(rename = "originalContentUrl")]
        original_content_url: String,
        #[serde(rename = "previewImageUrl")]
        preview_image_url: String,
    },
    #[serde(rename = "video")]
    Video {
        #[serde(rename = "originalContentUrl")]
        original_content_url: String,
        #[serde(rename = "previewImageUrl")]
        preview_image_url: String,
    },
    #[serde(rename = "audio")]
    Audio {
        #[serde(rename = "originalContentUrl")]
        original_content_url: String,
        duration: u64,
    },
    #[serde(rename = "sticker")]
    Sticker {
        #[serde(rename = "packageId")]
        package_id: String,
        #[serde(rename = "stickerId")]
        sticker_id: String,
    },
    #[serde(rename = "template")]
    Template {
        #[serde(rename = "altText")]
        alt_text: String,
        template: LineTemplate,
    },
}

/// Template message types.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum LineTemplate {
    #[serde(rename = "buttons")]
    Buttons {
        title: Option<String>,
        text: String,
        actions: Vec<LineAction>,
    },
    #[serde(rename = "confirm")]
    Confirm { text: String, actions: Vec<LineAction> },
    #[serde(rename = "carousel")]
    Carousel { columns: Vec<CarouselColumn> },
}

/// Carousel column.
#[derive(Debug, Serialize)]
pub struct CarouselColumn {
    #[serde(rename = "thumbnailImageUrl")]
    pub thumbnail_image_url: Option<String>,
    pub title: Option<String>,
    pub text: String,
    pub actions: Vec<LineAction>,
}

/// Template actions.
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum LineAction {
    #[serde(rename = "message")]
    Message { label: String, text: String },
    #[serde(rename = "uri")]
    Uri { label: String, uri: String },
    #[serde(rename = "postback")]
    Postback {
        label: String,
        data: String,
        #[serde(rename = "displayText")]
        display_text: Option<String>,
    },
}

/// User profile.
#[derive(Debug, Deserialize)]
pub struct LineProfile {
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "pictureUrl")]
    pub picture_url: Option<String>,
    #[serde(rename = "statusMessage")]
    pub status_message: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_channel_creation() {
        let channel = LineChannel::new("token", "secret", "channel123", "test_instance");
        assert_eq!(channel.channel_type(), "line");
        assert_eq!(channel.instance_id(), "test_instance");
    }

    #[test]
    fn test_capabilities() {
        let channel = LineChannel::new("token", "secret", "channel123", "test_instance");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.media.stickers);
        assert!(!caps.features.reactions);
        assert!(!caps.features.edits);
        assert_eq!(caps.limits.text_max_length, 5000);
        assert!(caps.chat_types.contains(&ChatType::Direct));
        assert!(caps.chat_types.contains(&ChatType::Group));
    }

    #[test]
    fn test_signature_verification() {
        let channel = LineChannel::new("token", "test_secret", "channel123", "test_instance");
        // This would need actual HMAC calculation for a real test
        // For now, just verify the method exists and doesn't panic
        let result = channel.verify_signature(b"test body", "invalid_signature");
        assert!(!result); // Should fail with invalid signature
    }
}
