# Telegram Channel Specification

## Overview

Full Telegram Bot API implementation using the `teloxide` crate for Rust.

## Dependencies

```toml
[dependencies]
teloxide = { version = "0.12", features = ["macros", "auto-send"] }
tokio = { version = "1", features = ["full"] }
```

## Authentication

```rust
pub struct TelegramConfig {
    /// Bot token from @BotFather
    pub token: SecretString,

    /// Webhook URL (optional, for webhook mode)
    pub webhook_url: Option<String>,

    /// Allowed updates to receive
    pub allowed_updates: Vec<UpdateType>,

    /// Drop pending updates on start
    pub drop_pending_updates: bool,

    /// Parse mode default
    pub default_parse_mode: ParseMode,
}

#[derive(Debug, Clone, Copy)]
pub enum UpdateType {
    Message,
    EditedMessage,
    ChannelPost,
    EditedChannelPost,
    InlineQuery,
    CallbackQuery,
    ShippingQuery,
    PreCheckoutQuery,
    Poll,
    PollAnswer,
    MyChatMember,
    ChatMember,
    ChatJoinRequest,
}
```

## Channel Implementation

```rust
pub struct TelegramChannel {
    config: TelegramConfig,
    bot: Option<Bot>,
    dispatcher: Option<Dispatcher<Bot, Error>>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: mpsc::Receiver<InboundMessage>,
    connected: AtomicBool,
}

#[async_trait]
impl Channel for TelegramChannel {
    fn id(&self) -> &str { "telegram" }
    fn name(&self) -> &str { "Telegram" }

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
                buttons: true,
                inline_queries: true,
                commands: true,
                markdown: true,
                html: true,
            },
            limits: ChannelLimits {
                max_message_length: 4096,
                max_caption_length: 1024,
                max_buttons_per_row: 8,
                max_button_rows: 100,
            },
        }
    }

    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    fn is_connected(&self) -> bool;
    async fn health(&self) -> Result<ChannelHealth>;
    async fn send(&self, message: OutboundMessage) -> Result<SendResult>;
    async fn edit(&self, target: MessageTarget, new_content: &str) -> Result<()>;
    async fn delete(&self, target: MessageTarget) -> Result<()>;
    async fn react(&self, target: MessageTarget, emoji: &str) -> Result<()>;
    async fn unreact(&self, target: MessageTarget, emoji: &str) -> Result<()>;
    async fn typing(&self, chat_id: &str) -> Result<()>;
    fn messages(&self) -> Pin<Box<dyn Stream<Item = InboundMessage> + Send>>;
}
```

## Message Handling

### Inbound Message Conversion

```rust
impl TelegramChannel {
    fn convert_message(&self, msg: teloxide::types::Message) -> InboundMessage {
        InboundMessage {
            id: MessageId::new(msg.id.to_string()),
            timestamp: DateTime::from_timestamp(msg.date.timestamp(), 0).unwrap_or_default(),
            channel: "telegram".to_string(),
            account_id: self.bot_id(),
            sender: SenderInfo {
                id: msg.from().map(|u| u.id.to_string()).unwrap_or_default(),
                username: msg.from().and_then(|u| u.username.clone()),
                display_name: msg.from().map(|u| u.full_name()),
                phone_number: None,
                is_bot: msg.from().map(|u| u.is_bot).unwrap_or(false),
            },
            chat: ChatInfo {
                id: msg.chat.id.to_string(),
                chat_type: match msg.chat.kind {
                    ChatKind::Private(_) => ChatType::Direct,
                    ChatKind::Group(_) | ChatKind::Supergroup(_) => ChatType::Group,
                    ChatKind::Channel(_) => ChatType::Channel,
                },
                title: msg.chat.title().map(|s| s.to_string()),
                guild_id: None,
            },
            text: msg.text().unwrap_or_default().to_string(),
            media: self.extract_media(&msg),
            quote: msg.reply_to_message().map(|r| QuotedMessage {
                id: r.id.to_string(),
                text: r.text().map(|s| s.to_string()),
                sender_id: r.from().map(|u| u.id.to_string()),
            }),
            thread: msg.thread_id.map(|id| ThreadInfo {
                id: id.to_string(),
                name: None,
            }),
            metadata: serde_json::json!({
                "message_thread_id": msg.thread_id,
                "is_topic_message": msg.is_topic_message,
                "forward_origin": msg.forward_origin().map(|f| format!("{:?}", f)),
            }),
        }
    }

    fn extract_media(&self, msg: &teloxide::types::Message) -> Vec<MediaAttachment> {
        let mut media = Vec::new();

        if let Some(photo) = msg.photo() {
            if let Some(largest) = photo.last() {
                media.push(MediaAttachment {
                    attachment_type: MediaType::Image,
                    url: None,
                    file_id: Some(largest.file.id.clone()),
                    file_path: None,
                    mime_type: Some("image/jpeg".to_string()),
                    file_name: None,
                    file_size: largest.file.size.map(|s| s as u64),
                    caption: msg.caption().map(|s| s.to_string()),
                });
            }
        }

        if let Some(doc) = msg.document() {
            media.push(MediaAttachment {
                attachment_type: MediaType::Document,
                url: None,
                file_id: Some(doc.file.id.clone()),
                file_path: None,
                mime_type: doc.mime_type.as_ref().map(|m| m.to_string()),
                file_name: doc.file_name.clone(),
                file_size: doc.file.size.map(|s| s as u64),
                caption: msg.caption().map(|s| s.to_string()),
            });
        }

        if let Some(audio) = msg.audio() {
            media.push(MediaAttachment {
                attachment_type: MediaType::Audio,
                url: None,
                file_id: Some(audio.file.id.clone()),
                file_path: None,
                mime_type: audio.mime_type.as_ref().map(|m| m.to_string()),
                file_name: audio.file_name.clone(),
                file_size: audio.file.size.map(|s| s as u64),
                caption: msg.caption().map(|s| s.to_string()),
            });
        }

        if let Some(video) = msg.video() {
            media.push(MediaAttachment {
                attachment_type: MediaType::Video,
                url: None,
                file_id: Some(video.file.id.clone()),
                file_path: None,
                mime_type: video.mime_type.as_ref().map(|m| m.to_string()),
                file_name: video.file_name.clone(),
                file_size: video.file.size.map(|s| s as u64),
                caption: msg.caption().map(|s| s.to_string()),
            });
        }

        if let Some(voice) = msg.voice() {
            media.push(MediaAttachment {
                attachment_type: MediaType::Voice,
                url: None,
                file_id: Some(voice.file.id.clone()),
                file_path: None,
                mime_type: voice.mime_type.as_ref().map(|m| m.to_string()),
                file_name: None,
                file_size: voice.file.size.map(|s| s as u64),
                caption: msg.caption().map(|s| s.to_string()),
            });
        }

        if let Some(sticker) = msg.sticker() {
            media.push(MediaAttachment {
                attachment_type: MediaType::Sticker,
                url: None,
                file_id: Some(sticker.file.id.clone()),
                file_path: None,
                mime_type: Some("image/webp".to_string()),
                file_name: None,
                file_size: sticker.file.size.map(|s| s as u64),
                caption: None,
            });
        }

        media
    }
}
```

### Outbound Message Sending

```rust
impl TelegramChannel {
    async fn send_text(&self, chat_id: ChatId, message: &OutboundMessage) -> Result<Message> {
        let mut request = self.bot.as_ref().unwrap()
            .send_message(chat_id, &message.text);

        if let Some(parse_mode) = &message.parse_mode {
            request = request.parse_mode(match parse_mode {
                ParseMode::Markdown => teloxide::types::ParseMode::MarkdownV2,
                ParseMode::Html => teloxide::types::ParseMode::Html,
                ParseMode::None => teloxide::types::ParseMode::Html,
            });
        }

        if let Some(reply_to) = &message.reply_to {
            if let Ok(msg_id) = reply_to.parse::<i32>() {
                request = request.reply_to_message_id(MessageId(msg_id));
            }
        }

        if let Some(thread_id) = &message.thread_id {
            if let Ok(tid) = thread_id.parse::<i32>() {
                request = request.message_thread_id(ThreadId(tid));
            }
        }

        if message.disable_preview {
            request = request.disable_web_page_preview(true);
        }

        if message.silent {
            request = request.disable_notification(true);
        }

        if let Some(buttons) = &message.buttons {
            let keyboard = self.build_inline_keyboard(buttons);
            request = request.reply_markup(keyboard);
        }

        request.await.map_err(|e| ChannelError::SendFailed(e.to_string()))
    }

    async fn send_media(&self, chat_id: ChatId, message: &OutboundMessage) -> Result<Message> {
        // Handle single media
        if message.media.len() == 1 {
            let media = &message.media[0];
            return self.send_single_media(chat_id, media, message).await;
        }

        // Handle media group
        if message.media.len() > 1 {
            return self.send_media_group(chat_id, &message.media, message).await;
        }

        Err(ChannelError::InvalidMessage("No media to send".to_string()))
    }

    fn build_inline_keyboard(&self, buttons: &[Vec<Button>]) -> InlineKeyboardMarkup {
        let rows: Vec<Vec<InlineKeyboardButton>> = buttons
            .iter()
            .map(|row| {
                row.iter()
                    .map(|btn| match &btn.action {
                        ButtonAction::Callback(data) => {
                            InlineKeyboardButton::callback(&btn.text, data)
                        }
                        ButtonAction::Url(url) => {
                            InlineKeyboardButton::url(&btn.text, url.parse().unwrap())
                        }
                    })
                    .collect()
            })
            .collect();

        InlineKeyboardMarkup::new(rows)
    }
}
```

## Telegram-Specific Actions

```rust
impl TelegramChannel {
    /// Forward a message
    pub async fn forward(
        &self,
        from_chat_id: ChatId,
        message_id: MessageId,
        to_chat_id: ChatId,
    ) -> Result<Message> {
        self.bot.as_ref().unwrap()
            .forward_message(to_chat_id, from_chat_id, message_id)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Pin a message
    pub async fn pin(&self, chat_id: ChatId, message_id: MessageId) -> Result<()> {
        self.bot.as_ref().unwrap()
            .pin_chat_message(chat_id, message_id)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))?;
        Ok(())
    }

    /// Unpin a message
    pub async fn unpin(&self, chat_id: ChatId, message_id: MessageId) -> Result<()> {
        self.bot.as_ref().unwrap()
            .unpin_chat_message(chat_id)
            .message_id(message_id)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))?;
        Ok(())
    }

    /// Get chat information
    pub async fn get_chat(&self, chat_id: ChatId) -> Result<Chat> {
        self.bot.as_ref().unwrap()
            .get_chat(chat_id)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Get chat member information
    pub async fn get_chat_member(&self, chat_id: ChatId, user_id: UserId) -> Result<ChatMember> {
        self.bot.as_ref().unwrap()
            .get_chat_member(chat_id, user_id)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Answer callback query
    pub async fn answer_callback(
        &self,
        callback_id: &str,
        text: Option<&str>,
        show_alert: bool,
    ) -> Result<()> {
        let mut request = self.bot.as_ref().unwrap()
            .answer_callback_query(callback_id);

        if let Some(text) = text {
            request = request.text(text);
        }

        request = request.show_alert(show_alert);

        request.await.map_err(|e| ChannelError::ActionFailed(e.to_string()))?;
        Ok(())
    }

    /// Set chat action (typing, uploading, etc.)
    pub async fn set_chat_action(&self, chat_id: ChatId, action: ChatAction) -> Result<()> {
        self.bot.as_ref().unwrap()
            .send_chat_action(chat_id, action)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))?;
        Ok(())
    }

    /// Download file by file_id
    pub async fn download_file(&self, file_id: &str) -> Result<Vec<u8>> {
        let file = self.bot.as_ref().unwrap()
            .get_file(file_id)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))?;

        let mut data = Vec::new();
        self.bot.as_ref().unwrap()
            .download_file(&file.path, &mut data)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))?;

        Ok(data)
    }
}
```

## Callback Query Handling

```rust
impl TelegramChannel {
    fn setup_callback_handler(&mut self) {
        // Handle inline button callbacks
        self.dispatcher.as_mut().unwrap().callback_queries_handler(|bot, query| async move {
            // Parse callback data
            if let Some(data) = &query.data {
                // Route to appropriate handler based on data prefix
                match data.split(':').next() {
                    Some("approve") => handle_approval_callback(bot, query).await,
                    Some("deny") => handle_denial_callback(bot, query).await,
                    Some("action") => handle_action_callback(bot, query).await,
                    _ => {
                        bot.answer_callback_query(&query.id).await?;
                    }
                }
            }
            Ok(())
        });
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum TelegramError {
    #[error("Bot not connected")]
    NotConnected,

    #[error("Invalid chat ID: {0}")]
    InvalidChatId(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u32),

    #[error("Bot blocked by user")]
    BotBlocked,

    #[error("Chat not found")]
    ChatNotFound,

    #[error("Insufficient permissions")]
    InsufficientPermissions,

    #[error("API error: {0}")]
    ApiError(String),
}

impl From<teloxide::RequestError> for TelegramError {
    fn from(err: teloxide::RequestError) -> Self {
        match err {
            teloxide::RequestError::Api(api_err) => {
                match api_err {
                    ApiError::BotBlocked => TelegramError::BotBlocked,
                    ApiError::ChatNotFound => TelegramError::ChatNotFound,
                    ApiError::MessageNotModified => TelegramError::MessageNotFound("not modified".to_string()),
                    _ => TelegramError::ApiError(api_err.to_string()),
                }
            }
            _ => TelegramError::ApiError(err.to_string()),
        }
    }
}
```
