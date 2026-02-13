//! WhatsApp channel implementation.
//!
//! This channel integrates with WhatsApp via the WhatsApp Cloud API (Meta).
//! It supports sending text messages, media, and receiving webhooks.

#![cfg(feature = "whatsapp")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver, ChannelSender, MessageHandler,
    MessageRef, SendResult,
};
use crate::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use smartassist_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatInfo, ChatType,
    HealthStatus, InboundMessage, MediaAttachment, MediaCapabilities, MediaType, MessageId,
    MessageTarget, OutboundMessage, QuotedMessage, SenderInfo,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// WhatsApp Cloud API base URL.
const WHATSAPP_API_BASE: &str = "https://graph.facebook.com/v18.0";

/// WhatsApp channel implementation using Cloud API.
pub struct WhatsAppChannel {
    /// Phone number ID from WhatsApp Business.
    phone_number_id: String,

    /// Access token for the API.
    access_token: String,

    /// Business Account ID.
    business_account_id: Option<String>,

    /// Channel instance ID.
    instance_id: String,

    /// HTTP client.
    client: Client,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Webhook verification token.
    webhook_verify_token: Option<String>,

    /// Incoming message channel.
    #[allow(dead_code)]
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl std::fmt::Debug for WhatsAppChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhatsAppChannel")
            .field("instance_id", &self.instance_id)
            .field("phone_number_id", &self.phone_number_id)
            .finish()
    }
}

/// WhatsApp message payload for sending.
#[derive(Debug, Serialize)]
struct WhatsAppMessagePayload {
    messaging_product: &'static str,
    recipient_type: &'static str,
    to: String,
    #[serde(rename = "type")]
    message_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<WhatsAppText>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<WhatsAppMedia>,
    #[serde(skip_serializing_if = "Option::is_none")]
    video: Option<WhatsAppMedia>,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<WhatsAppMedia>,
    #[serde(skip_serializing_if = "Option::is_none")]
    document: Option<WhatsAppDocument>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<WhatsAppContext>,
}

#[derive(Debug, Serialize)]
struct WhatsAppText {
    body: String,
    preview_url: bool,
}

#[derive(Debug, Serialize)]
struct WhatsAppMedia {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
}

#[derive(Debug, Serialize)]
struct WhatsAppDocument {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    caption: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

#[derive(Debug, Serialize)]
struct WhatsAppContext {
    message_id: String,
}

/// WhatsApp API response for sent messages.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WhatsAppSendResponse {
    messaging_product: String,
    contacts: Vec<WhatsAppContact>,
    messages: Vec<WhatsAppMessageRef>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct WhatsAppContact {
    input: String,
    wa_id: String,
}

#[derive(Debug, Deserialize)]
struct WhatsAppMessageRef {
    id: String,
}

/// WhatsApp webhook payload for incoming messages.
#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhookPayload {
    pub object: String,
    pub entry: Vec<WhatsAppWebhookEntry>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhookEntry {
    pub id: String,
    pub changes: Vec<WhatsAppWebhookChange>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhookChange {
    pub value: WhatsAppWebhookValue,
    pub field: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhookValue {
    pub messaging_product: String,
    pub metadata: WhatsAppMetadata,
    #[serde(default)]
    pub contacts: Vec<WhatsAppWebhookContact>,
    #[serde(default)]
    pub messages: Vec<WhatsAppWebhookMessage>,
    #[serde(default)]
    pub statuses: Vec<WhatsAppWebhookStatus>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppMetadata {
    pub display_phone_number: String,
    pub phone_number_id: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhookContact {
    pub profile: WhatsAppProfile,
    pub wa_id: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppProfile {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhookMessage {
    pub from: String,
    pub id: String,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub message_type: String,
    #[serde(default)]
    pub text: Option<WhatsAppTextContent>,
    #[serde(default)]
    pub image: Option<WhatsAppMediaContent>,
    #[serde(default)]
    pub video: Option<WhatsAppMediaContent>,
    #[serde(default)]
    pub audio: Option<WhatsAppMediaContent>,
    #[serde(default)]
    pub document: Option<WhatsAppDocumentContent>,
    #[serde(default)]
    pub context: Option<WhatsAppMessageContext>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppTextContent {
    pub body: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppMediaContent {
    pub id: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppDocumentContent {
    pub id: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub filename: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppMessageContext {
    pub from: String,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhookStatus {
    pub id: String,
    pub status: String,
    pub timestamp: String,
    pub recipient_id: String,
}

/// Media upload response from WhatsApp.
#[derive(Debug, Deserialize)]
struct WhatsAppMediaUploadResponse {
    id: String,
}

impl WhatsAppChannel {
    /// Create a new WhatsApp channel.
    pub fn new(
        phone_number_id: impl Into<String>,
        access_token: impl Into<String>,
        instance_id: impl Into<String>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        Self {
            phone_number_id: phone_number_id.into(),
            access_token: access_token.into(),
            business_account_id: None,
            instance_id: instance_id.into(),
            client: Client::new(),
            connected: Arc::new(RwLock::new(false)),
            webhook_verify_token: None,
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> std::result::Result<Self, ChannelError> {
        let phone_number_id = config
            .options
            .get("phone_number_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::Config("Missing phone_number_id".to_string()))?
            .to_string();

        let access_token = config
            .options
            .get("access_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::Config("Missing access_token".to_string()))?
            .to_string();

        let mut channel = Self::new(phone_number_id, access_token, config.instance_id);

        channel.business_account_id = config
            .options
            .get("business_account_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        channel.webhook_verify_token = config
            .options
            .get("webhook_verify_token")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(channel)
    }

    /// Set webhook verification token.
    pub fn with_webhook_verify_token(mut self, token: impl Into<String>) -> Self {
        self.webhook_verify_token = Some(token.into());
        self
    }

    /// Get the API URL for messages.
    fn messages_url(&self) -> String {
        format!(
            "{}/{}/messages",
            WHATSAPP_API_BASE, self.phone_number_id
        )
    }

    /// Get the API URL for media upload.
    fn media_url(&self) -> String {
        format!("{}/{}/media", WHATSAPP_API_BASE, self.phone_number_id)
    }

    /// Upload media and return the media ID.
    async fn upload_media(
        &self,
        data: &[u8],
        mime_type: &str,
        filename: Option<&str>,
    ) -> Result<String> {
        let form = reqwest::multipart::Form::new()
            .text("messaging_product", "whatsapp")
            .text("type", mime_type.to_string())
            .part(
                "file",
                reqwest::multipart::Part::bytes(data.to_vec())
                    .file_name(filename.unwrap_or("file").to_string())
                    .mime_str(mime_type)
                    .map_err(|e| ChannelError::Internal(e.to_string()))?,
            );

        let response = self
            .client
            .post(&self.media_url())
            .bearer_auth(&self.access_token)
            .multipart(form)
            .send()
            .await
            .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ChannelError::channel(
                "whatsapp",
                format!("Media upload failed ({}): {}", status, body),
            ));
        }

        let upload_response: WhatsAppMediaUploadResponse = response
            .json()
            .await
            .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?;

        Ok(upload_response.id)
    }

    /// Convert a webhook message to InboundMessage.
    #[allow(dead_code)]
    fn convert_webhook_message(
        &self,
        msg: &WhatsAppWebhookMessage,
        contact: Option<&WhatsAppWebhookContact>,
    ) -> InboundMessage {
        let sender = SenderInfo {
            id: msg.from.clone(),
            username: None,
            display_name: contact.map(|c| c.profile.name.clone()),
            phone_number: Some(format!("+{}", msg.from)),
            is_bot: false,
        };

        let chat = ChatInfo {
            id: msg.from.clone(),
            chat_type: ChatType::Direct, // WhatsApp DMs are always direct
            title: None,
            guild_id: None,
        };

        let text = msg
            .text
            .as_ref()
            .map(|t| t.body.clone())
            .unwrap_or_default();

        let media = self.extract_media(msg);

        let quote = msg.context.as_ref().map(|ctx| QuotedMessage {
            id: ctx.id.clone(),
            text: None,
            sender_id: Some(ctx.from.clone()),
        });

        // Parse timestamp (Unix timestamp string)
        let timestamp = msg
            .timestamp
            .parse::<i64>()
            .ok()
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
            .unwrap_or_else(Utc::now);

        InboundMessage {
            id: MessageId::new(msg.id.clone()),
            timestamp,
            channel: "whatsapp".to_string(),
            account_id: self.phone_number_id.clone(),
            sender,
            chat,
            text,
            media,
            quote,
            thread: None,
            metadata: serde_json::json!({
                "message_type": msg.message_type,
            }),
        }
    }

    /// Extract media attachments from a webhook message.
    #[allow(dead_code)]
    fn extract_media(&self, msg: &WhatsAppWebhookMessage) -> Vec<MediaAttachment> {
        let mut media = Vec::new();

        if let Some(ref image) = msg.image {
            media.push(MediaAttachment {
                id: image.id.clone(),
                media_type: MediaType::Image,
                url: None, // Would need to fetch via media API
                data: None,
                filename: None,
                size_bytes: None,
                mime_type: image.mime_type.clone(),
            });
        }

        if let Some(ref video) = msg.video {
            media.push(MediaAttachment {
                id: video.id.clone(),
                media_type: MediaType::Video,
                url: None,
                data: None,
                filename: None,
                size_bytes: None,
                mime_type: video.mime_type.clone(),
            });
        }

        if let Some(ref audio) = msg.audio {
            media.push(MediaAttachment {
                id: audio.id.clone(),
                media_type: MediaType::Audio,
                url: None,
                data: None,
                filename: None,
                size_bytes: None,
                mime_type: audio.mime_type.clone(),
            });
        }

        if let Some(ref doc) = msg.document {
            media.push(MediaAttachment {
                id: doc.id.clone(),
                media_type: MediaType::Document,
                url: None,
                data: None,
                filename: doc.filename.clone(),
                size_bytes: None,
                mime_type: doc.mime_type.clone(),
            });
        }

        media
    }

    /// Process an incoming webhook payload.
    /// This should be called from your webhook handler.
    pub async fn process_webhook(&self, payload: WhatsAppWebhookPayload) -> Result<()> {
        for entry in payload.entry {
            for change in entry.changes {
                if change.field == "messages" {
                    let contacts = &change.value.contacts;

                    for msg in &change.value.messages {
                        let contact = contacts.iter().find(|c| c.wa_id == msg.from);
                        let inbound = self.convert_webhook_message(msg, contact);

                        // If we have a handler, call it
                        let handler = self.handler.read().await;
                        if let Some(ref h) = *handler {
                            if let Err(e) = h.handle(inbound).await {
                                warn!("Handler error: {}", e);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Verify webhook subscription (called during webhook setup).
    pub fn verify_webhook(
        &self,
        mode: &str,
        token: &str,
        challenge: &str,
    ) -> std::result::Result<String, ChannelError> {
        if mode != "subscribe" {
            return Err(ChannelError::Auth("Invalid mode".to_string()));
        }

        match &self.webhook_verify_token {
            Some(expected) if expected == token => Ok(challenge.to_string()),
            Some(_) => Err(ChannelError::Auth("Invalid verify token".to_string())),
            None => Err(ChannelError::Config(
                "Webhook verify token not configured".to_string(),
            )),
        }
    }

    /// Normalize phone number to WhatsApp format (digits only, no +).
    fn normalize_phone(&self, phone: &str) -> String {
        phone
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect()
    }
}

#[async_trait]
impl Channel for WhatsAppChannel {
    fn channel_type(&self) -> &str {
        "whatsapp"
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
                voice_notes: true,
                max_file_size_mb: 100, // 100MB for most media, 16MB for audio
            },
            features: ChannelFeatures {
                reactions: true,
                threads: false,
                edits: false, // WhatsApp doesn't support message editing
                deletes: false,
                typing_indicators: false, // Not available via Cloud API
                read_receipts: true,
                mentions: true,
                polls: false,
                native_commands: false,
            },
            limits: ChannelLimits {
                text_max_length: 4096,
                caption_max_length: 1024,
                messages_per_second: 80.0, // Varies by tier
                messages_per_minute: 1000,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for WhatsAppChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to WhatsApp".to_string()));
        }

        let recipient = self.normalize_phone(&message.target.chat_id);
        debug!("Sending WhatsApp message to {}", recipient);

        let payload = WhatsAppMessagePayload {
            messaging_product: "whatsapp",
            recipient_type: "individual",
            to: recipient,
            message_type: "text".to_string(),
            text: Some(WhatsAppText {
                body: message.text.clone(),
                preview_url: !message.options.disable_preview,
            }),
            image: None,
            video: None,
            audio: None,
            document: None,
            context: message.reply_to.map(|id| WhatsAppContext { message_id: id }),
        };

        let response = self
            .client
            .post(&self.messages_url())
            .bearer_auth(&self.access_token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ChannelError::channel(
                "whatsapp",
                format!("Send failed ({}): {}", status, body),
            ));
        }

        let send_response: WhatsAppSendResponse = response
            .json()
            .await
            .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?;

        let msg_id = send_response
            .messages
            .first()
            .map(|m| m.id.clone())
            .unwrap_or_default();

        Ok(SendResult::new(msg_id))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to WhatsApp".to_string()));
        }

        let recipient = self.normalize_phone(&message.target.chat_id);
        let mut last_msg_id = String::new();

        for attachment in attachments {
            // Upload media first
            let media_id = match &attachment.source {
                crate::attachment::AttachmentSource::Bytes(bytes) => {
                    let mime = if attachment.mime_type.is_empty() {
                        "application/octet-stream"
                    } else {
                        &attachment.mime_type
                    };
                    self.upload_media(bytes, mime, Some(&attachment.filename))
                        .await?
                }
                crate::attachment::AttachmentSource::Path(path) => {
                    let bytes = tokio::fs::read(path)
                        .await
                        .map_err(|e| ChannelError::Internal(e.to_string()))?;
                    let mime = if attachment.mime_type.is_empty() {
                        "application/octet-stream"
                    } else {
                        &attachment.mime_type
                    };
                    self.upload_media(&bytes, mime, Some(&attachment.filename))
                        .await?
                }
                crate::attachment::AttachmentSource::Url(url) => {
                    // Can use link directly for some media types
                    let (message_type, media_field) = match attachment.attachment_type {
                        crate::attachment::AttachmentType::Image => ("image", "image"),
                        crate::attachment::AttachmentType::Video => ("video", "video"),
                        crate::attachment::AttachmentType::Audio => ("audio", "audio"),
                        _ => ("document", "document"),
                    };

                    let mut payload = WhatsAppMessagePayload {
                        messaging_product: "whatsapp",
                        recipient_type: "individual",
                        to: recipient.clone(),
                        message_type: message_type.to_string(),
                        text: None,
                        image: None,
                        video: None,
                        audio: None,
                        document: None,
                        context: None,
                    };

                    match media_field {
                        "image" => {
                            payload.image = Some(WhatsAppMedia {
                                id: None,
                                link: Some(url.clone()),
                                caption: attachment.caption.clone(),
                            });
                        }
                        "video" => {
                            payload.video = Some(WhatsAppMedia {
                                id: None,
                                link: Some(url.clone()),
                                caption: attachment.caption.clone(),
                            });
                        }
                        "audio" => {
                            payload.audio = Some(WhatsAppMedia {
                                id: None,
                                link: Some(url.clone()),
                                caption: None,
                            });
                        }
                        _ => {
                            payload.document = Some(WhatsAppDocument {
                                id: None,
                                link: Some(url.clone()),
                                caption: attachment.caption.clone(),
                                filename: Some(attachment.filename.clone()),
                            });
                        }
                    }

                    let response = self
                        .client
                        .post(&self.messages_url())
                        .bearer_auth(&self.access_token)
                        .json(&payload)
                        .send()
                        .await
                        .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?;

                    if response.status().is_success() {
                        let send_response: WhatsAppSendResponse = response.json().await.ok().unwrap_or(WhatsAppSendResponse {
                            messaging_product: String::new(),
                            contacts: vec![],
                            messages: vec![],
                        });
                        last_msg_id = send_response
                            .messages
                            .first()
                            .map(|m| m.id.clone())
                            .unwrap_or_default();
                    }

                    continue;
                }
                _ => {
                    warn!("Unsupported attachment source for WhatsApp");
                    continue;
                }
            };

            // Send with uploaded media ID
            let (message_type, media_field) = match attachment.attachment_type {
                crate::attachment::AttachmentType::Image => ("image", "image"),
                crate::attachment::AttachmentType::Video => ("video", "video"),
                crate::attachment::AttachmentType::Audio
                | crate::attachment::AttachmentType::Voice => ("audio", "audio"),
                _ => ("document", "document"),
            };

            let mut payload = WhatsAppMessagePayload {
                messaging_product: "whatsapp",
                recipient_type: "individual",
                to: recipient.clone(),
                message_type: message_type.to_string(),
                text: None,
                image: None,
                video: None,
                audio: None,
                document: None,
                context: None,
            };

            match media_field {
                "image" => {
                    payload.image = Some(WhatsAppMedia {
                        id: Some(media_id),
                        link: None,
                        caption: attachment.caption.clone(),
                    });
                }
                "video" => {
                    payload.video = Some(WhatsAppMedia {
                        id: Some(media_id),
                        link: None,
                        caption: attachment.caption.clone(),
                    });
                }
                "audio" => {
                    payload.audio = Some(WhatsAppMedia {
                        id: Some(media_id),
                        link: None,
                        caption: None,
                    });
                }
                _ => {
                    payload.document = Some(WhatsAppDocument {
                        id: Some(media_id),
                        link: None,
                        caption: attachment.caption.clone(),
                        filename: Some(attachment.filename.clone()),
                    });
                }
            }

            let response = self
                .client
                .post(&self.messages_url())
                .bearer_auth(&self.access_token)
                .json(&payload)
                .send()
                .await
                .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?;

            if response.status().is_success() {
                let send_response: WhatsAppSendResponse = response.json().await.ok().unwrap_or(WhatsAppSendResponse {
                    messaging_product: String::new(),
                    contacts: vec![],
                    messages: vec![],
                });
                last_msg_id = send_response
                    .messages
                    .first()
                    .map(|m| m.id.clone())
                    .unwrap_or_default();
            }
        }

        // Send text message if present
        if !message.text.is_empty() {
            return self.send(message).await;
        }

        Ok(SendResult::new(last_msg_id))
    }

    async fn edit(&self, _message: &MessageRef, _new_content: &str) -> Result<()> {
        // WhatsApp doesn't support message editing
        warn!("WhatsApp does not support message editing");
        Err(ChannelError::Internal(
            "WhatsApp does not support message editing".to_string(),
        ))
    }

    async fn delete(&self, _message: &MessageRef) -> Result<()> {
        // WhatsApp doesn't support message deletion via API
        warn!("WhatsApp does not support message deletion via API");
        Err(ChannelError::Internal(
            "WhatsApp does not support message deletion".to_string(),
        ))
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to WhatsApp".to_string()));
        }

        // WhatsApp reaction payload - now we have recipient from MessageRef
        let payload = serde_json::json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": message.chat_id,
            "type": "reaction",
            "reaction": {
                "message_id": message.message_id,
                "emoji": emoji
            }
        });

        let url = format!(
            "https://graph.facebook.com/v18.0/{}/messages",
            self.phone_number_id
        );

        self.client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?
            .error_for_status()
            .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?;

        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, _emoji: &str) -> Result<()> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to WhatsApp".to_string()));
        }

        // To remove reaction, send empty emoji
        let payload = serde_json::json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": message.chat_id,
            "type": "reaction",
            "reaction": {
                "message_id": message.message_id,
                "emoji": ""
            }
        });

        let url = format!(
            "https://graph.facebook.com/v18.0/{}/messages",
            self.phone_number_id
        );

        self.client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?
            .error_for_status()
            .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?;

        Ok(())
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        // WhatsApp Cloud API doesn't support typing indicators
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        4096
    }
}

#[async_trait]
impl ChannelReceiver for WhatsAppChannel {
    async fn start_receiving(&self) -> Result<()> {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        // WhatsApp uses webhooks for receiving messages
        // The webhook endpoint should call process_webhook() when messages arrive
        info!(
            "WhatsApp channel ready for webhooks (instance: {})",
            self.instance_id
        );

        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        let mut shutdown = self.shutdown.write().await;
        if let Some(tx) = shutdown.take() {
            let _ = tx.send(());
        }

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
impl ChannelLifecycle for WhatsAppChannel {
    async fn connect(&self) -> Result<()> {
        // Verify credentials by checking phone number info
        let url = format!(
            "{}/{}",
            WHATSAPP_API_BASE, self.phone_number_id
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
            .map_err(|e| ChannelError::channel("whatsapp", e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ChannelError::Auth(format!(
                "WhatsApp auth failed ({}): {}",
                status, body
            )));
        }

        info!(
            "Connected to WhatsApp Cloud API (phone_number_id: {})",
            self.phone_number_id
        );

        let mut connected = self.connected.write().await;
        *connected = true;

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;

        let mut connected = self.connected.write().await;
        *connected = false;

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let start = std::time::Instant::now();

        let url = format!(
            "{}/{}",
            WHATSAPP_API_BASE, self.phone_number_id
        );

        match self
            .client
            .get(&url)
            .bearer_auth(&self.access_token)
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => Ok(ChannelHealth {
                status: HealthStatus::Healthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                last_message_at: None,
                error: None,
            }),
            Ok(response) => Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                last_message_at: None,
                error: Some(format!("API returned {}", response.status())),
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

impl Clone for WhatsAppChannel {
    fn clone(&self) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            phone_number_id: self.phone_number_id.clone(),
            access_token: self.access_token.clone(),
            business_account_id: self.business_account_id.clone(),
            instance_id: self.instance_id.clone(),
            client: self.client.clone(),
            connected: self.connected.clone(),
            webhook_verify_token: self.webhook_verify_token.clone(),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: self.handler.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whatsapp_channel_creation() {
        let channel = WhatsAppChannel::new("123456789", "access_token", "test_whatsapp");
        assert_eq!(channel.channel_type(), "whatsapp");
        assert_eq!(channel.instance_id(), "test_whatsapp");
    }

    #[test]
    fn test_capabilities() {
        let channel = WhatsAppChannel::new("123456789", "access_token", "test_whatsapp");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.media.voice_notes);
        assert!(caps.features.reactions);
        assert!(!caps.features.edits); // WhatsApp doesn't support edits
        assert!(caps.chat_types.contains(&ChatType::Direct));
    }

    #[test]
    fn test_normalize_phone() {
        let channel = WhatsAppChannel::new("123456789", "access_token", "test");

        assert_eq!(channel.normalize_phone("+1 555 123 4567"), "15551234567");
        assert_eq!(channel.normalize_phone("1-555-123-4567"), "15551234567");
        assert_eq!(channel.normalize_phone("15551234567"), "15551234567");
    }

    #[test]
    fn test_verify_webhook_success() {
        let channel = WhatsAppChannel::new("123456789", "access_token", "test")
            .with_webhook_verify_token("my_verify_token");

        let result = channel.verify_webhook("subscribe", "my_verify_token", "challenge123");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "challenge123");
    }

    #[test]
    fn test_verify_webhook_wrong_token() {
        let channel = WhatsAppChannel::new("123456789", "access_token", "test")
            .with_webhook_verify_token("my_verify_token");

        let result = channel.verify_webhook("subscribe", "wrong_token", "challenge123");
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_webhook_wrong_mode() {
        let channel = WhatsAppChannel::new("123456789", "access_token", "test")
            .with_webhook_verify_token("my_verify_token");

        let result = channel.verify_webhook("unsubscribe", "my_verify_token", "challenge123");
        assert!(result.is_err());
    }
}
