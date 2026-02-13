//! Telegram channel implementation.

#![cfg(feature = "telegram")]

use crate::attachment::{Attachment, AttachmentType};
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver, ChannelSender, MessageHandler,
    MessageRef, SendResult,
};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatInfo, ChatType,
    HealthStatus, InboundMessage, MediaAttachment, MediaCapabilities, MediaType, MessageId,
    MessageTarget, OutboundMessage, ParseMode as CoreParseMode, QuotedMessage, SenderInfo,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatId, InputFile, MediaKind, MessageKind, ParseMode};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Telegram channel implementation.
pub struct TelegramChannel {
    /// Bot instance.
    bot: Bot,

    /// Channel instance ID.
    instance_id: String,

    /// Bot username.
    username: Option<String>,

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

impl std::fmt::Debug for TelegramChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelegramChannel")
            .field("instance_id", &self.instance_id)
            .field("username", &self.username)
            .finish()
    }
}

impl TelegramChannel {
    /// Create a new Telegram channel.
    pub fn new(bot_token: impl Into<String>, instance_id: impl Into<String>) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        Self {
            bot: Bot::new(bot_token),
            instance_id: instance_id.into(),
            username: None,
            connected: Arc::new(RwLock::new(false)),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig, bot_token: String) -> Self {
        Self::new(bot_token, config.instance_id)
    }

    /// Convert Telegram message to InboundMessage.
    async fn convert_message(&self, msg: &teloxide::types::Message) -> Option<InboundMessage> {
        let from = msg.from()?;

        let sender = SenderInfo {
            id: from.id.to_string(),
            username: from.username.clone(),
            display_name: Some(
                from.last_name
                    .as_ref()
                    .map(|ln| format!("{} {}", from.first_name, ln))
                    .unwrap_or_else(|| from.first_name.clone()),
            ),
            phone_number: None,
            is_bot: from.is_bot,
        };

        let chat_type = match &msg.chat.kind {
            teloxide::types::ChatKind::Private(_) => ChatType::Direct,
            teloxide::types::ChatKind::Public(public) => match &public.kind {
                teloxide::types::PublicChatKind::Group(_) => ChatType::Group,
                teloxide::types::PublicChatKind::Supergroup(_) => ChatType::Group,
                teloxide::types::PublicChatKind::Channel(_) => ChatType::Channel,
            },
        };

        let chat = ChatInfo {
            id: msg.chat.id.to_string(),
            chat_type,
            title: msg.chat.title().map(|t| t.to_string()),
            guild_id: None,
        };

        let text = msg.text().unwrap_or_default().to_string();
        let media = self.extract_attachments(msg).await;

        let quote = msg.reply_to_message().map(|reply| QuotedMessage {
            id: reply.id.to_string(),
            text: reply.text().map(|t| t.to_string()),
            sender_id: reply.from().map(|f| f.id.to_string()),
        });

        Some(InboundMessage {
            id: MessageId::new(msg.id.to_string()),
            timestamp: chrono::Utc::now(),
            channel: "telegram".to_string(),
            account_id: self.instance_id.clone(),
            sender,
            chat,
            text,
            media,
            quote,
            thread: None,
            metadata: serde_json::to_value(msg).unwrap_or_default(),
        })
    }

    /// Extract attachments from a message.
    async fn extract_attachments(&self, msg: &teloxide::types::Message) -> Vec<MediaAttachment> {
        let mut attachments = Vec::new();

        if let MessageKind::Common(common) = &msg.kind {
            match &common.media_kind {
                MediaKind::Photo(photo) => {
                    if let Some(largest) = photo.photo.last() {
                        attachments.push(MediaAttachment {
                            id: largest.file.id.clone(),
                            media_type: MediaType::Image,
                            url: None,
                            data: None,
                            filename: None,
                            size_bytes: Some(largest.file.size as u64),
                            mime_type: Some("image/jpeg".to_string()),
                        });
                    }
                }
                MediaKind::Document(doc) => {
                    attachments.push(MediaAttachment {
                        id: doc.document.file.id.clone(),
                        media_type: MediaType::Document,
                        url: None,
                        data: None,
                        filename: doc.document.file_name.clone(),
                        size_bytes: Some(doc.document.file.size as u64),
                        mime_type: doc.document.mime_type.as_ref().map(|m| m.to_string()),
                    });
                }
                MediaKind::Audio(audio) => {
                    attachments.push(MediaAttachment {
                        id: audio.audio.file.id.clone(),
                        media_type: MediaType::Audio,
                        url: None,
                        data: None,
                        filename: audio.audio.file_name.clone(),
                        size_bytes: Some(audio.audio.file.size as u64),
                        mime_type: audio.audio.mime_type.as_ref().map(|m| m.to_string()),
                    });
                }
                MediaKind::Voice(voice) => {
                    attachments.push(MediaAttachment {
                        id: voice.voice.file.id.clone(),
                        media_type: MediaType::Voice,
                        url: None,
                        data: None,
                        filename: None,
                        size_bytes: Some(voice.voice.file.size as u64),
                        mime_type: voice.voice.mime_type.as_ref().map(|m| m.to_string()),
                    });
                }
                MediaKind::Video(video) => {
                    attachments.push(MediaAttachment {
                        id: video.video.file.id.clone(),
                        media_type: MediaType::Video,
                        url: None,
                        data: None,
                        filename: video.video.file_name.clone(),
                        size_bytes: Some(video.video.file.size as u64),
                        mime_type: video.video.mime_type.as_ref().map(|m| m.to_string()),
                    });
                }
                MediaKind::Sticker(sticker) => {
                    attachments.push(MediaAttachment {
                        id: sticker.sticker.file.id.clone(),
                        media_type: MediaType::Sticker,
                        url: None,
                        data: None,
                        filename: None,
                        size_bytes: Some(sticker.sticker.file.size as u64),
                        mime_type: Some("image/webp".to_string()),
                    });
                }
                _ => {}
            }
        }

        attachments
    }

    /// Call the Telegram Bot API setMessageReaction endpoint directly.
    /// This bypasses teloxide since it doesn't yet support Bot API 7.0+ reactions.
    async fn set_message_reaction(
        &self,
        chat_id: i64,
        message_id: i32,
        reactions: Vec<ReactionType>,
    ) -> Result<()> {
        let url = format!(
            "https://api.telegram.org/bot{}/setMessageReaction",
            self.bot.token()
        );

        let request = SetMessageReactionRequest {
            chat_id,
            message_id,
            reaction: reactions,
            is_big: None,
        };

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .json(&request)
            .send()
            .await
            .map_err(|e| ChannelError::channel("telegram", format!("HTTP request failed: {}", e)))?;

        let status = response.status();
        let body: TelegramApiResponse = response
            .json()
            .await
            .map_err(|e| ChannelError::channel("telegram", format!("Failed to parse response: {}", e)))?;

        if !body.ok {
            let error_msg = body.description.unwrap_or_else(|| format!("HTTP {}", status));
            return Err(ChannelError::channel("telegram", error_msg));
        }

        Ok(())
    }
}

/// Request body for setMessageReaction API call.
#[derive(Debug, Serialize)]
struct SetMessageReactionRequest {
    chat_id: i64,
    message_id: i32,
    reaction: Vec<ReactionType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    is_big: Option<bool>,
}

/// Telegram API response wrapper.
#[derive(Debug, Deserialize)]
struct TelegramApiResponse {
    ok: bool,
    #[serde(default)]
    description: Option<String>,
}

/// Telegram reaction type for Bot API 7.0+.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ReactionType {
    /// Standard emoji reaction.
    Emoji {
        emoji: String,
    },
    /// Custom emoji reaction (Telegram Premium).
    CustomEmoji {
        custom_emoji_id: String,
    },
}

impl ReactionType {
    /// Create an emoji reaction.
    pub fn emoji(emoji: impl Into<String>) -> Self {
        Self::Emoji { emoji: emoji.into() }
    }

    /// Create a custom emoji reaction.
    pub fn custom_emoji(id: impl Into<String>) -> Self {
        Self::CustomEmoji { custom_emoji_id: id.into() }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn channel_type(&self) -> &str {
        "telegram"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct, ChatType::Group, ChatType::Channel],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: true,
                voice_notes: true,
                max_file_size_mb: 50,
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
                text_max_length: 4096,
                caption_max_length: 1024,
                messages_per_second: 30.0,
                messages_per_minute: 1800,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for TelegramChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let chat_id = ChatId(
            message
                .target
                .chat_id
                .parse::<i64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        let mut request = self.bot.send_message(chat_id, &message.text);

        // Set parse mode
        if let Some(ref parse_mode) = message.options.parse_mode {
            match parse_mode {
                CoreParseMode::Html => request = request.parse_mode(ParseMode::Html),
                CoreParseMode::Markdown => request = request.parse_mode(ParseMode::MarkdownV2),
                CoreParseMode::Plain => {} // No parse mode for plain text
            }
        }

        // Set reply
        if let Some(ref reply_to) = message.reply_to {
            if let Ok(id) = reply_to.parse::<i32>() {
                request = request.reply_to_message_id(teloxide::types::MessageId(id));
            }
        }

        let sent = request
            .await
            .map_err(|e| ChannelError::channel("telegram", e.to_string()))?;

        Ok(SendResult::new(sent.id.to_string()))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        let chat_id = ChatId(
            message
                .target
                .chat_id
                .parse::<i64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        // Send attachments first
        let mut last_msg_id = None;

        for attachment in attachments {
            let input_file = match &attachment.source {
                crate::attachment::AttachmentSource::FileId(id) => InputFile::file_id(id.clone()),
                crate::attachment::AttachmentSource::Url(url) => InputFile::url(url.parse().unwrap()),
                crate::attachment::AttachmentSource::Bytes(bytes) => {
                    InputFile::memory(bytes.to_vec()).file_name(attachment.filename.clone())
                }
                crate::attachment::AttachmentSource::Path(path) => InputFile::file(path),
            };

            let result = match attachment.attachment_type {
                AttachmentType::Image => {
                    self.bot
                        .send_photo(chat_id, input_file)
                        .caption(attachment.caption.unwrap_or_default())
                        .await
                }
                AttachmentType::Audio => {
                    self.bot
                        .send_audio(chat_id, input_file)
                        .caption(attachment.caption.unwrap_or_default())
                        .await
                }
                AttachmentType::Video => {
                    self.bot
                        .send_video(chat_id, input_file)
                        .caption(attachment.caption.unwrap_or_default())
                        .await
                }
                AttachmentType::Voice => self.bot.send_voice(chat_id, input_file).await,
                _ => {
                    self.bot
                        .send_document(chat_id, input_file)
                        .caption(attachment.caption.unwrap_or_default())
                        .await
                }
            };

            match result {
                Ok(msg) => last_msg_id = Some(msg.id.to_string()),
                Err(e) => warn!("Failed to send attachment: {}", e),
            }
        }

        // Send text if present
        if !message.text.is_empty() {
            return self.send(message).await;
        }

        Ok(SendResult::new(last_msg_id.unwrap_or_default()))
    }

    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()> {
        let chat_id = ChatId(
            message
                .chat_id
                .parse::<i64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );
        let message_id = teloxide::types::MessageId(
            message
                .message_id
                .parse::<i32>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        self.bot
            .edit_message_text(chat_id, message_id, new_content)
            .await
            .map_err(|e| ChannelError::channel("telegram", e.to_string()))?;

        Ok(())
    }

    async fn delete(&self, message: &MessageRef) -> Result<()> {
        let chat_id = ChatId(
            message
                .chat_id
                .parse::<i64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );
        let message_id = teloxide::types::MessageId(
            message
                .message_id
                .parse::<i32>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        self.bot
            .delete_message(chat_id, message_id)
            .await
            .map_err(|e| ChannelError::channel("telegram", e.to_string()))?;

        Ok(())
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let chat_id = message
            .chat_id
            .parse::<i64>()
            .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?;
        let message_id = message
            .message_id
            .parse::<i32>()
            .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?;

        debug!(
            "Setting Telegram reaction {} on {}:{}",
            emoji, chat_id, message_id
        );

        // Use raw API call since teloxide 0.12 doesn't support Bot API 7.0+ reactions
        self.set_message_reaction(chat_id, message_id, vec![ReactionType::emoji(emoji)])
            .await
    }

    async fn unreact(&self, message: &MessageRef, _emoji: &str) -> Result<()> {
        let chat_id = message
            .chat_id
            .parse::<i64>()
            .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?;
        let message_id = message
            .message_id
            .parse::<i32>()
            .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?;

        debug!(
            "Removing Telegram reactions on {}:{}",
            chat_id, message_id
        );

        // To remove reactions, send an empty array
        self.set_message_reaction(chat_id, message_id, vec![])
            .await
    }

    async fn send_typing(&self, target: &MessageTarget) -> Result<()> {
        let chat_id = ChatId(
            target
                .chat_id
                .parse::<i64>()
                .map_err(|e| ChannelError::InvalidMessage(e.to_string()))?,
        );

        self.bot
            .send_chat_action(chat_id, teloxide::types::ChatAction::Typing)
            .await
            .map_err(|e| ChannelError::channel("telegram", e.to_string()))?;

        Ok(())
    }

    fn max_message_length(&self) -> usize {
        4096
    }
}

#[async_trait]
impl ChannelReceiver for TelegramChannel {
    async fn start_receiving(&self) -> Result<()> {
        let (shutdown_tx, _shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        let bot = self.bot.clone();
        let tx = self.message_tx.clone();
        let channel = Arc::new(self.clone());

        tokio::spawn(async move {
            let handler = Update::filter_message().endpoint(
                move |_bot: Bot, msg: teloxide::types::Message| {
                    let tx = tx.clone();
                    let channel = channel.clone();
                    async move {
                        if let Some(inbound) = channel.convert_message(&msg).await {
                            let _ = tx.send(inbound).await;
                        }
                        respond(())
                    }
                },
            );

            Dispatcher::builder(bot, handler).build().dispatch().await;
        });

        info!("Started receiving messages for Telegram bot: {}", self.instance_id);
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
impl ChannelLifecycle for TelegramChannel {
    async fn connect(&self) -> Result<()> {
        // Verify bot token by getting bot info
        let me = self
            .bot
            .get_me()
            .await
            .map_err(|e| ChannelError::Auth(e.to_string()))?;

        info!(
            "Connected to Telegram as @{}",
            me.username.as_deref().unwrap_or("unknown")
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
        // This is a blocking read, should be fine for status check
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let start = std::time::Instant::now();

        match self.bot.get_me().await {
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
        }
    }
}

impl Clone for TelegramChannel {
    fn clone(&self) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            bot: self.bot.clone(),
            instance_id: self.instance_id.clone(),
            username: self.username.clone(),
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
    fn test_telegram_channel_creation() {
        let channel = TelegramChannel::new("test_token", "test_bot");
        assert_eq!(channel.channel_type(), "telegram");
        assert_eq!(channel.instance_id(), "test_bot");
    }

    #[test]
    fn test_capabilities() {
        let channel = TelegramChannel::new("test_token", "test_bot");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.features.edits);
        assert!(caps.features.reactions);
        assert_eq!(caps.limits.text_max_length, 4096);
        assert!(caps.chat_types.contains(&ChatType::Direct));
        assert!(caps.chat_types.contains(&ChatType::Group));
    }

    #[test]
    fn test_reaction_type_serialization() {
        // Test emoji reaction
        let emoji_reaction = ReactionType::emoji("👍");
        let json = serde_json::to_string(&emoji_reaction).unwrap();
        assert!(json.contains("\"type\":\"emoji\""));
        assert!(json.contains("\"emoji\":\"👍\""));

        // Test custom emoji reaction
        let custom_reaction = ReactionType::custom_emoji("5368324170671202286");
        let json = serde_json::to_string(&custom_reaction).unwrap();
        assert!(json.contains("\"type\":\"custom_emoji\""));
        assert!(json.contains("\"custom_emoji_id\":\"5368324170671202286\""));

        // Test request serialization
        let request = SetMessageReactionRequest {
            chat_id: 123456789,
            message_id: 42,
            reaction: vec![ReactionType::emoji("❤️")],
            is_big: Some(true),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"chat_id\":123456789"));
        assert!(json.contains("\"message_id\":42"));
        assert!(json.contains("\"is_big\":true"));
    }
}
