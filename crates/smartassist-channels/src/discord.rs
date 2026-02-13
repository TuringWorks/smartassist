//! Discord channel implementation.

#![cfg(feature = "discord")]

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
use serenity::all::{
    ChannelId, CreateAttachment, CreateMessage, GatewayIntents, Http, Message,
};
use serenity::async_trait as serenity_async_trait;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

/// Discord channel implementation.
pub struct DiscordChannel {
    /// Bot token.
    token: String,

    /// Application ID.
    application_id: u64,

    /// Channel instance ID.
    instance_id: String,

    /// HTTP client for API calls.
    http: Arc<RwLock<Option<Arc<Http>>>>,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Incoming message channel.
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl std::fmt::Debug for DiscordChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscordChannel")
            .field("instance_id", &self.instance_id)
            .field("application_id", &self.application_id)
            .finish()
    }
}

impl DiscordChannel {
    /// Create a new Discord channel.
    pub fn new(
        token: impl Into<String>,
        application_id: u64,
        instance_id: impl Into<String>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        Self {
            token: token.into(),
            application_id,
            instance_id: instance_id.into(),
            http: Arc::new(RwLock::new(None)),
            connected: Arc::new(RwLock::new(false)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig, token: String, application_id: u64) -> Self {
        Self::new(token, application_id, config.instance_id)
    }

    /// Convert Discord message to InboundMessage.
    fn convert_message(&self, msg: &Message) -> InboundMessage {
        let sender = SenderInfo {
            id: msg.author.id.to_string(),
            username: Some(msg.author.name.clone()),
            display_name: msg.author.global_name.clone(),
            phone_number: None,
            is_bot: msg.author.bot,
        };

        let chat_type = if msg.guild_id.is_some() {
            ChatType::Group
        } else {
            ChatType::Direct
        };

        let chat = ChatInfo {
            id: msg.channel_id.to_string(),
            chat_type,
            title: None,
            guild_id: msg.guild_id.map(|id| id.to_string()),
        };

        let media = self.extract_media(msg);

        let quote = msg.referenced_message.as_ref().map(|r| QuotedMessage {
            id: r.id.to_string(),
            text: Some(r.content.clone()),
            sender_id: Some(r.author.id.to_string()),
        });

        // Convert time::OffsetDateTime to chrono::DateTime<Utc>
        let timestamp = DateTime::<Utc>::from_timestamp(
            msg.timestamp.unix_timestamp(),
            msg.timestamp.nanosecond(),
        )
        .unwrap_or_else(Utc::now);

        InboundMessage {
            id: MessageId::new(msg.id.to_string()),
            timestamp,
            channel: "discord".to_string(),
            account_id: self.application_id.to_string(),
            sender,
            chat,
            text: msg.content.clone(),
            media,
            quote,
            thread: None,
            metadata: serde_json::json!({
                "guild_id": msg.guild_id,
                "webhook_id": msg.webhook_id,
            }),
        }
    }

    /// Extract media attachments from a Discord message.
    fn extract_media(&self, msg: &Message) -> Vec<MediaAttachment> {
        let mut media = Vec::new();

        for attachment in &msg.attachments {
            let media_type = self.guess_media_type(&attachment.content_type);
            media.push(MediaAttachment {
                id: attachment.id.to_string(),
                media_type,
                url: Some(attachment.url.clone()),
                data: None,
                filename: Some(attachment.filename.clone()),
                size_bytes: Some(attachment.size as u64),
                mime_type: attachment.content_type.clone(),
            });
        }

        for embed in &msg.embeds {
            if let Some(ref image) = embed.image {
                media.push(MediaAttachment {
                    id: format!("embed_{}", media.len()),
                    media_type: MediaType::Image,
                    url: Some(image.url.clone()),
                    data: None,
                    filename: None,
                    size_bytes: None,
                    mime_type: None,
                });
            }
        }

        media
    }

    /// Guess media type from content type.
    fn guess_media_type(&self, content_type: &Option<String>) -> MediaType {
        match content_type.as_deref() {
            Some(ct) if ct.starts_with("image/") => MediaType::Image,
            Some(ct) if ct.starts_with("video/") => MediaType::Video,
            Some(ct) if ct.starts_with("audio/") => MediaType::Audio,
            _ => MediaType::Document,
        }
    }
}

/// Event handler for Discord gateway events.
struct Handler {
    message_tx: mpsc::Sender<InboundMessage>,
    channel: Arc<DiscordChannel>,
}

#[serenity_async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, msg: Message) {
        // Ignore bot messages
        if msg.author.bot {
            return;
        }

        let inbound = self.channel.convert_message(&msg);
        let _ = self.message_tx.send(inbound).await;
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("Discord bot connected as {}", ready.user.name);
    }
}

#[async_trait]
impl Channel for DiscordChannel {
    fn channel_type(&self) -> &str {
        "discord"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![
                ChatType::Direct,
                ChatType::Group,
                ChatType::Channel,
                ChatType::Thread,
            ],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: true,
                voice_notes: false,
                max_file_size_mb: 25,
            },
            features: ChannelFeatures {
                reactions: true,
                threads: true,
                edits: true,
                deletes: true,
                typing_indicators: true,
                read_receipts: false,
                mentions: true,
                polls: true,
                native_commands: true,
            },
            limits: ChannelLimits {
                text_max_length: 2000,
                caption_max_length: 2000,
                messages_per_second: 5.0,
                messages_per_minute: 300,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for DiscordChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let channel_id = ChannelId::new(
            message
                .target
                .chat_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        let mut builder = CreateMessage::new().content(&message.text);

        // Set reply reference
        if let Some(ref reply_to) = message.reply_to {
            if let Ok(msg_id) = reply_to.parse::<u64>() {
                builder = builder.reference_message((channel_id, serenity::all::MessageId::new(msg_id)));
            }
        }

        let sent = channel_id
            .send_message(http, builder)
            .await
            .map_err(|e| ChannelError::channel("discord", e.to_string()))?;

        Ok(SendResult::new(sent.id.to_string()))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let channel_id = ChannelId::new(
            message
                .target
                .chat_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        let mut files = Vec::new();
        for attachment in attachments {
            match &attachment.source {
                crate::attachment::AttachmentSource::Bytes(bytes) => {
                    files.push(CreateAttachment::bytes(
                        bytes.to_vec(),
                        attachment.filename.clone(),
                    ));
                }
                crate::attachment::AttachmentSource::Path(path) => {
                    if let Ok(file) = CreateAttachment::path(path).await {
                        files.push(file);
                    }
                }
                _ => {
                    warn!("Unsupported attachment source for Discord");
                }
            }
        }

        let mut builder = CreateMessage::new();
        if !message.text.is_empty() {
            builder = builder.content(&message.text);
        }

        for file in files {
            builder = builder.add_file(file);
        }

        let sent = channel_id
            .send_message(http, builder)
            .await
            .map_err(|e| ChannelError::channel("discord", e.to_string()))?;

        Ok(SendResult::new(sent.id.to_string()))
    }

    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()> {
        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let channel_id = ChannelId::new(
            message
                .chat_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );
        let message_id = serenity::all::MessageId::new(
            message
                .message_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        channel_id
            .edit_message(http, message_id, serenity::all::EditMessage::new().content(new_content))
            .await
            .map_err(|e| ChannelError::channel("discord", e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, message: &MessageRef) -> Result<()> {
        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let channel_id = ChannelId::new(
            message
                .chat_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );
        let message_id = serenity::all::MessageId::new(
            message
                .message_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        channel_id
            .delete_message(http, message_id)
            .await
            .map_err(|e| ChannelError::channel("discord", e.to_string()))?;

        Ok(())
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let channel_id = ChannelId::new(
            message
                .chat_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );
        let message_id = serenity::all::MessageId::new(
            message
                .message_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        // Parse emoji - could be unicode or custom emoji format
        let reaction = serenity::all::ReactionType::Unicode(emoji.to_string());

        http.create_reaction(channel_id, message_id, &reaction)
            .await
            .map_err(|e| ChannelError::channel("discord", e.to_string()))?;

        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let channel_id = ChannelId::new(
            message
                .chat_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );
        let message_id = serenity::all::MessageId::new(
            message
                .message_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        let reaction = serenity::all::ReactionType::Unicode(emoji.to_string());

        http.delete_reaction_me(channel_id, message_id, &reaction)
            .await
            .map_err(|e| ChannelError::channel("discord", e.to_string()))?;

        Ok(())
    }

    async fn send_typing(&self, target: &MessageTarget) -> Result<()> {
        let http_guard = self.http.read().await;
        let http = http_guard
            .as_ref()
            .ok_or_else(|| ChannelError::Internal("Not connected".to_string()))?;

        let channel_id = ChannelId::new(
            target
                .chat_id
                .parse::<u64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        channel_id
            .broadcast_typing(http)
            .await
            .map_err(|e| ChannelError::channel("discord", e.to_string()))?;

        Ok(())
    }

    fn max_message_length(&self) -> usize {
        2000
    }
}

#[async_trait]
impl ChannelReceiver for DiscordChannel {
    async fn start_receiving(&self) -> Result<()> {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        let intents = GatewayIntents::GUILDS
            | GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

        let tx = self.message_tx.clone();
        let channel = Arc::new(DiscordChannel {
            token: self.token.clone(),
            application_id: self.application_id,
            instance_id: self.instance_id.clone(),
            http: self.http.clone(),
            connected: self.connected.clone(),
            message_tx: self.message_tx.clone(),
            message_rx: Arc::new(RwLock::new(mpsc::channel(1).1)), // Dummy receiver
            handler: self.handler.clone(),
            shutdown: self.shutdown.clone(),
        });

        let handler = Handler {
            message_tx: tx,
            channel,
        };

        let token = self.token.clone();
        let http_arc = self.http.clone();
        let connected = self.connected.clone();

        tokio::spawn(async move {
            let mut client = Client::builder(&token, intents)
                .event_handler(handler)
                .await
                .expect("Failed to create Discord client");

            // Store HTTP client
            {
                let mut http = http_arc.write().await;
                *http = Some(client.http.clone());
            }

            {
                let mut conn = connected.write().await;
                *conn = true;
            }

            if let Err(e) = client.start().await {
                warn!("Discord client error: {}", e);
            }
        });

        info!(
            "Started receiving messages for Discord bot: {}",
            self.instance_id
        );
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        let mut shutdown = self.shutdown.write().await;
        if let Some(tx) = shutdown.take() {
            let _ = tx.send(());
        }

        let mut connected = self.connected.write().await;
        *connected = false;

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
impl ChannelLifecycle for DiscordChannel {
    async fn connect(&self) -> Result<()> {
        // Connection is handled in start_receiving
        // Just verify token by attempting to get current user
        let http = Http::new(&self.token);

        let me = http
            .get_current_user()
            .await
            .map_err(|e| ChannelError::Auth(e.to_string()))?;

        info!("Connected to Discord as {}", me.name);

        {
            let mut http_guard = self.http.write().await;
            *http_guard = Some(Arc::new(http));
        }

        let mut connected = self.connected.write().await;
        *connected = true;

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;

        let mut http_guard = self.http.write().await;
        *http_guard = None;

        let mut connected = self.connected.write().await;
        *connected = false;

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let start = std::time::Instant::now();

        let http_guard = self.http.read().await;
        match http_guard.as_ref() {
            Some(http) => match http.get_current_user().await {
                Ok(_) => Ok(ChannelHealth {
                    status: HealthStatus::Healthy,
                    latency_ms: Some(start.elapsed().as_millis() as u64),
                    last_message_at: None,
                    error: None,
                }),
                Err(e) => Ok(ChannelHealth {
                    status: HealthStatus::Unhealthy,
                    latency_ms: None,
                    last_message_at: None,
                    error: Some(e.to_string()),
                }),
            },
            None => Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: None,
                last_message_at: None,
                error: Some("Not connected".to_string()),
            }),
        }
    }
}

impl Clone for DiscordChannel {
    fn clone(&self) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            token: self.token.clone(),
            application_id: self.application_id,
            instance_id: self.instance_id.clone(),
            http: self.http.clone(),
            connected: self.connected.clone(),
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
    fn test_discord_channel_creation() {
        let channel = DiscordChannel::new("test_token", 123456789, "test_bot");
        assert_eq!(channel.channel_type(), "discord");
        assert_eq!(channel.instance_id(), "test_bot");
    }

    #[test]
    fn test_capabilities() {
        let channel = DiscordChannel::new("test_token", 123456789, "test_bot");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.features.threads);
        assert!(caps.features.reactions);
        assert_eq!(caps.limits.text_max_length, 2000);
        assert!(caps.chat_types.contains(&ChatType::Direct));
        assert!(caps.chat_types.contains(&ChatType::Group));
    }
}
