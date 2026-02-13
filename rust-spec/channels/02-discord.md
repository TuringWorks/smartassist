# Discord Channel Specification

## Overview

Full Discord Bot API implementation using `serenity` crate for Rust.

## Dependencies

```toml
[dependencies]
serenity = { version = "0.12", features = ["client", "gateway", "cache"] }
tokio = { version = "1", features = ["full"] }
```

## Authentication

```rust
pub struct DiscordConfig {
    /// Bot token from Discord Developer Portal
    pub token: SecretString,

    /// Application ID
    pub application_id: u64,

    /// Gateway intents
    pub intents: GatewayIntents,

    /// Guild IDs to listen to (empty = all)
    pub guild_ids: Vec<u64>,

    /// Whether to enable slash commands
    pub slash_commands: bool,
}

impl Default for DiscordConfig {
    fn default() -> Self {
        Self {
            token: SecretString::new(String::new()),
            application_id: 0,
            intents: GatewayIntents::GUILDS
                | GatewayIntents::GUILD_MESSAGES
                | GatewayIntents::DIRECT_MESSAGES
                | GatewayIntents::MESSAGE_CONTENT,
            guild_ids: vec![],
            slash_commands: true,
        }
    }
}
```

## Channel Implementation

```rust
pub struct DiscordChannel {
    config: DiscordConfig,
    client: Option<Client>,
    http: Option<Arc<Http>>,
    cache: Option<Arc<Cache>>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: mpsc::Receiver<InboundMessage>,
    connected: AtomicBool,
}

#[async_trait]
impl Channel for DiscordChannel {
    fn id(&self) -> &str { "discord" }
    fn name(&self) -> &str { "Discord" }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct, ChatType::Group, ChatType::Channel, ChatType::Thread],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: true,
                voice_notes: false,
                max_file_size_mb: 25, // 100MB for Nitro
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
                inline_queries: false,
                commands: true,
                markdown: true,
                html: false,
            },
            limits: ChannelLimits {
                max_message_length: 2000,
                max_caption_length: 2000,
                max_buttons_per_row: 5,
                max_button_rows: 5,
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
impl DiscordChannel {
    fn convert_message(&self, msg: serenity::model::channel::Message) -> InboundMessage {
        InboundMessage {
            id: MessageId::new(msg.id.to_string()),
            timestamp: msg.timestamp.to_utc(),
            channel: "discord".to_string(),
            account_id: self.config.application_id.to_string(),
            sender: SenderInfo {
                id: msg.author.id.to_string(),
                username: Some(msg.author.name.clone()),
                display_name: msg.author.global_name.clone(),
                phone_number: None,
                is_bot: msg.author.bot,
            },
            chat: ChatInfo {
                id: msg.channel_id.to_string(),
                chat_type: self.determine_chat_type(&msg),
                title: None, // Fetched separately for guilds
                guild_id: msg.guild_id.map(|id| id.to_string()),
            },
            text: msg.content.clone(),
            media: self.extract_media(&msg),
            quote: msg.referenced_message.as_ref().map(|r| QuotedMessage {
                id: r.id.to_string(),
                text: Some(r.content.clone()),
                sender_id: Some(r.author.id.to_string()),
            }),
            thread: msg.thread.as_ref().map(|t| ThreadInfo {
                id: t.id.to_string(),
                name: Some(t.name.clone()),
            }),
            metadata: serde_json::json!({
                "guild_id": msg.guild_id,
                "channel_type": format!("{:?}", msg.kind),
                "webhook_id": msg.webhook_id,
                "components": msg.components,
            }),
        }
    }

    fn determine_chat_type(&self, msg: &serenity::model::channel::Message) -> ChatType {
        match msg.guild_id {
            Some(_) => match msg.channel(&self.cache) {
                Some(Channel::Guild(gc)) => match gc.kind {
                    ChannelType::Text => ChatType::Channel,
                    ChannelType::PublicThread | ChannelType::PrivateThread => ChatType::Thread,
                    _ => ChatType::Group,
                },
                _ => ChatType::Group,
            },
            None => ChatType::Direct,
        }
    }

    fn extract_media(&self, msg: &serenity::model::channel::Message) -> Vec<MediaAttachment> {
        let mut media = Vec::new();

        for attachment in &msg.attachments {
            let media_type = self.guess_media_type(&attachment.content_type);
            media.push(MediaAttachment {
                attachment_type: media_type,
                url: Some(attachment.url.clone()),
                file_id: Some(attachment.id.to_string()),
                file_path: None,
                mime_type: attachment.content_type.clone(),
                file_name: Some(attachment.filename.clone()),
                file_size: Some(attachment.size as u64),
                caption: None,
            });
        }

        for embed in &msg.embeds {
            if let Some(ref image) = embed.image {
                media.push(MediaAttachment {
                    attachment_type: MediaType::Image,
                    url: Some(image.url.clone()),
                    file_id: None,
                    file_path: None,
                    mime_type: None,
                    file_name: None,
                    file_size: None,
                    caption: embed.description.clone(),
                });
            }
        }

        media
    }

    fn guess_media_type(&self, content_type: &Option<String>) -> MediaType {
        match content_type.as_deref() {
            Some(ct) if ct.starts_with("image/") => MediaType::Image,
            Some(ct) if ct.starts_with("video/") => MediaType::Video,
            Some(ct) if ct.starts_with("audio/") => MediaType::Audio,
            _ => MediaType::Document,
        }
    }
}
```

### Outbound Message Sending

```rust
impl DiscordChannel {
    async fn send_text(&self, channel_id: ChannelId, message: &OutboundMessage) -> Result<Message> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;

        channel_id.send_message(http, |m| {
            m.content(&message.text);

            if let Some(reply_to) = &message.reply_to {
                if let Ok(msg_id) = reply_to.parse::<u64>() {
                    m.reference_message(MessageId::new(msg_id));
                }
            }

            if let Some(buttons) = &message.buttons {
                m.components(|c| {
                    for row in buttons {
                        c.create_action_row(|r| {
                            for btn in row {
                                r.create_button(|b| {
                                    b.label(&btn.text);
                                    match &btn.action {
                                        ButtonAction::Callback(data) => {
                                            b.custom_id(data);
                                            b.style(ButtonStyle::Primary);
                                        }
                                        ButtonAction::Url(url) => {
                                            b.url(url);
                                            b.style(ButtonStyle::Link);
                                        }
                                    }
                                    b
                                });
                            }
                            r
                        });
                    }
                    c
                });
            }

            m
        }).await.map_err(|e| ChannelError::SendFailed(e.to_string()))
    }

    async fn send_media(&self, channel_id: ChannelId, message: &OutboundMessage) -> Result<Message> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;

        let files: Vec<_> = message.media.iter()
            .filter_map(|m| {
                m.file_path.as_ref().map(|path| {
                    CreateAttachment::path(path)
                })
            })
            .collect();

        channel_id.send_files(http, files, |m| {
            if !message.text.is_empty() {
                m.content(&message.text);
            }
            m
        }).await.map_err(|e| ChannelError::SendFailed(e.to_string()))
    }
}
```

## Discord-Specific Actions

```rust
impl DiscordChannel {
    /// Create a thread from a message
    pub async fn create_thread(
        &self,
        channel_id: ChannelId,
        message_id: MessageId,
        name: &str,
        auto_archive_duration: AutoArchiveDuration,
    ) -> Result<GuildChannel> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;
        channel_id.create_public_thread(http, message_id, |t| {
            t.name(name).auto_archive_duration(auto_archive_duration)
        }).await.map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Get guild member information
    pub async fn get_member(&self, guild_id: GuildId, user_id: UserId) -> Result<Member> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;
        guild_id.member(http, user_id)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Get guild role information
    pub async fn get_roles(&self, guild_id: GuildId) -> Result<Vec<Role>> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;
        guild_id.roles(http)
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Timeout a member
    pub async fn timeout_member(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        duration: Duration,
        reason: Option<&str>,
    ) -> Result<()> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;
        let timeout_until = Utc::now() + duration;
        guild_id.edit_member(http, user_id, |m| {
            m.disable_communication_until(timeout_until.to_rfc3339());
            if let Some(r) = reason {
                m.audit_log_reason(r);
            }
            m
        }).await.map_err(|e| ChannelError::ActionFailed(e.to_string()))?;
        Ok(())
    }

    /// Kick a member
    pub async fn kick_member(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        reason: Option<&str>,
    ) -> Result<()> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;
        guild_id.kick_with_reason(http, user_id, reason.unwrap_or(""))
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Ban a member
    pub async fn ban_member(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        delete_message_days: u8,
        reason: Option<&str>,
    ) -> Result<()> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;
        guild_id.ban_with_reason(http, user_id, delete_message_days, reason.unwrap_or(""))
            .await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// List guild channels
    pub async fn list_channels(&self, guild_id: GuildId) -> Result<Vec<GuildChannel>> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;
        guild_id.channels(http)
            .await
            .map(|m| m.into_values().collect())
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Create a scheduled event
    pub async fn create_event(
        &self,
        guild_id: GuildId,
        name: &str,
        description: Option<&str>,
        start_time: DateTime<Utc>,
        end_time: Option<DateTime<Utc>>,
        location: Option<&str>,
    ) -> Result<ScheduledEvent> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;
        guild_id.create_scheduled_event(http, |e| {
            e.name(name)
                .start_time(start_time)
                .entity_type(ScheduledEventType::External);
            if let Some(desc) = description {
                e.description(desc);
            }
            if let Some(end) = end_time {
                e.end_time(end);
            }
            if let Some(loc) = location {
                e.location(loc);
            }
            e
        }).await.map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }
}
```

## Slash Commands

```rust
impl DiscordChannel {
    pub async fn register_slash_commands(&self, guild_id: Option<GuildId>) -> Result<()> {
        let http = self.http.as_ref().ok_or(ChannelError::NotConnected)?;

        let commands = vec![
            CreateCommand::new("ask")
                .description("Ask the AI a question")
                .add_option(
                    CreateCommandOption::new(CommandOptionType::String, "question", "Your question")
                        .required(true)
                ),
            CreateCommand::new("status")
                .description("Get bot status"),
        ];

        match guild_id {
            Some(id) => {
                id.set_commands(http, commands).await?;
            }
            None => {
                Command::set_global_commands(http, commands).await?;
            }
        }

        Ok(())
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum DiscordError {
    #[error("Bot not connected")]
    NotConnected,

    #[error("Invalid channel ID: {0}")]
    InvalidChannelId(String),

    #[error("Invalid guild ID: {0}")]
    InvalidGuildId(String),

    #[error("Message not found: {0}")]
    MessageNotFound(String),

    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u32),

    #[error("Insufficient permissions: {0}")]
    InsufficientPermissions(String),

    #[error("Guild not found")]
    GuildNotFound,

    #[error("Member not found")]
    MemberNotFound,

    #[error("API error: {0}")]
    ApiError(String),
}

impl From<serenity::Error> for DiscordError {
    fn from(err: serenity::Error) -> Self {
        match err {
            serenity::Error::Http(http_err) => {
                DiscordError::ApiError(http_err.to_string())
            }
            serenity::Error::Model(model_err) => {
                DiscordError::ApiError(model_err.to_string())
            }
            _ => DiscordError::ApiError(err.to_string()),
        }
    }
}
```
