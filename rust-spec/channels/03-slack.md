# Slack Channel Specification

## Overview

Full Slack API implementation using Slack Web API and Socket Mode.

## Dependencies

```toml
[dependencies]
slack-morphism = { version = "2.0", features = ["hyper", "socket-mode"] }
tokio = { version = "1", features = ["full"] }
```

## Authentication

```rust
pub struct SlackConfig {
    /// Bot token (xoxb-...)
    pub bot_token: SecretString,

    /// App token for Socket Mode (xapp-...)
    pub app_token: SecretString,

    /// Signing secret for webhook verification
    pub signing_secret: SecretString,

    /// Workspace ID (optional, for multi-workspace)
    pub workspace_id: Option<String>,

    /// Use Socket Mode (vs webhook)
    pub socket_mode: bool,

    /// Webhook URL (if not using socket mode)
    pub webhook_url: Option<String>,
}
```

## Channel Implementation

```rust
pub struct SlackChannel {
    config: SlackConfig,
    client: Option<SlackClient<SlackClientHyperConnector>>,
    socket: Option<SlackClientSocketModeListener>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: mpsc::Receiver<InboundMessage>,
    connected: AtomicBool,
}

#[async_trait]
impl Channel for SlackChannel {
    fn id(&self) -> &str { "slack" }
    fn name(&self) -> &str { "Slack" }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct, ChatType::Group, ChatType::Channel, ChatType::Thread],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: false,
                voice_notes: false,
                max_file_size_mb: 1000, // 1GB for paid workspaces
            },
            features: ChannelFeatures {
                reactions: true,
                threads: true,
                edits: true,
                deletes: true,
                typing_indicators: false,
                read_receipts: false,
                mentions: true,
                polls: false,
                buttons: true,
                inline_queries: false,
                commands: true,
                markdown: true, // mrkdwn
                html: false,
            },
            limits: ChannelLimits {
                max_message_length: 40000,
                max_caption_length: 40000,
                max_buttons_per_row: 5,
                max_button_rows: 10,
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
impl SlackChannel {
    fn convert_message(&self, event: SlackMessageEvent) -> InboundMessage {
        InboundMessage {
            id: MessageId::new(event.ts.clone()),
            timestamp: self.parse_slack_ts(&event.ts),
            channel: "slack".to_string(),
            account_id: self.config.workspace_id.clone().unwrap_or_default(),
            sender: SenderInfo {
                id: event.user.clone().unwrap_or_default(),
                username: None, // Fetched separately
                display_name: None,
                phone_number: None,
                is_bot: event.bot_id.is_some(),
            },
            chat: ChatInfo {
                id: event.channel.clone(),
                chat_type: self.determine_chat_type(&event.channel_type),
                title: None, // Fetched separately
                guild_id: self.config.workspace_id.clone(),
            },
            text: event.text.clone().unwrap_or_default(),
            media: self.extract_media(&event.files),
            quote: None, // Slack uses threads differently
            thread: event.thread_ts.map(|ts| ThreadInfo {
                id: ts.clone(),
                name: None,
            }),
            metadata: serde_json::json!({
                "ts": event.ts,
                "thread_ts": event.thread_ts,
                "channel_type": event.channel_type,
                "subtype": event.subtype,
                "blocks": event.blocks,
            }),
        }
    }

    fn determine_chat_type(&self, channel_type: &Option<String>) -> ChatType {
        match channel_type.as_deref() {
            Some("im") => ChatType::Direct,
            Some("mpim") => ChatType::Group,
            Some("channel") | Some("group") => ChatType::Channel,
            _ => ChatType::Channel,
        }
    }

    fn extract_media(&self, files: &Option<Vec<SlackFile>>) -> Vec<MediaAttachment> {
        files.as_ref().map(|fs| {
            fs.iter().map(|f| MediaAttachment {
                attachment_type: self.guess_media_type(&f.mimetype),
                url: f.url_private.clone(),
                file_id: Some(f.id.clone()),
                file_path: None,
                mime_type: f.mimetype.clone(),
                file_name: f.name.clone(),
                file_size: f.size.map(|s| s as u64),
                caption: f.title.clone(),
            }).collect()
        }).unwrap_or_default()
    }

    fn parse_slack_ts(&self, ts: &str) -> DateTime<Utc> {
        let secs: f64 = ts.parse().unwrap_or(0.0);
        DateTime::from_timestamp(secs as i64, ((secs.fract() * 1_000_000_000.0) as u32))
            .unwrap_or_default()
    }
}
```

### Outbound Message Sending

```rust
impl SlackChannel {
    async fn send_text(&self, channel: &str, message: &OutboundMessage) -> Result<SlackMessageResponse> {
        let client = self.client.as_ref().ok_or(ChannelError::NotConnected)?;
        let session = client.open_session(&self.config.bot_token.expose_secret().into());

        let mut request = SlackApiChatPostMessageRequest::new(
            channel.into(),
            SlackMessageContent::new().with_text(message.text.clone()),
        );

        if let Some(thread_ts) = &message.thread_id {
            request = request.with_thread_ts(thread_ts.into());
        }

        if let Some(buttons) = &message.buttons {
            let blocks = self.build_blocks(buttons);
            request = request.with_blocks(blocks);
        }

        if message.silent {
            // Slack doesn't have a direct silent mode
        }

        session.chat_post_message(&request)
            .await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))
    }

    async fn send_media(&self, channel: &str, message: &OutboundMessage) -> Result<SlackFileResponse> {
        let client = self.client.as_ref().ok_or(ChannelError::NotConnected)?;
        let session = client.open_session(&self.config.bot_token.expose_secret().into());

        for media in &message.media {
            if let Some(path) = &media.file_path {
                let content = tokio::fs::read(path).await?;
                let request = SlackApiFilesUploadRequest::new(content)
                    .with_channels(vec![channel.into()])
                    .with_filename(media.file_name.clone().unwrap_or_else(|| "file".to_string()));

                if let Some(caption) = &media.caption {
                    request = request.with_initial_comment(caption.clone());
                }

                session.files_upload(&request).await?;
            }
        }

        Ok(SlackFileResponse::default())
    }

    fn build_blocks(&self, buttons: &[Vec<Button>]) -> Vec<SlackBlock> {
        buttons.iter().map(|row| {
            let elements: Vec<SlackBlockElement> = row.iter().map(|btn| {
                match &btn.action {
                    ButtonAction::Callback(data) => {
                        SlackBlockElement::Button(SlackBlockButtonElement::new(
                            data.into(),
                            SlackBlockPlainText::new(btn.text.clone()),
                        ))
                    }
                    ButtonAction::Url(url) => {
                        SlackBlockElement::Button(
                            SlackBlockButtonElement::new(
                                "url_button".into(),
                                SlackBlockPlainText::new(btn.text.clone()),
                            ).with_url(url.into())
                        )
                    }
                }
            }).collect();

            SlackBlock::Actions(SlackActionsBlock::new(elements))
        }).collect()
    }
}
```

## Slack-Specific Actions

```rust
impl SlackChannel {
    /// Pin a message
    pub async fn pin(&self, channel: &str, ts: &str) -> Result<()> {
        let session = self.get_session()?;
        let request = SlackApiPinsAddRequest::new(channel.into(), ts.into());
        session.pins_add(&request).await?;
        Ok(())
    }

    /// Unpin a message
    pub async fn unpin(&self, channel: &str, ts: &str) -> Result<()> {
        let session = self.get_session()?;
        let request = SlackApiPinsRemoveRequest::new(channel.into(), ts.into());
        session.pins_remove(&request).await?;
        Ok(())
    }

    /// Get conversation history
    pub async fn get_history(
        &self,
        channel: &str,
        limit: Option<u32>,
        oldest: Option<&str>,
        latest: Option<&str>,
    ) -> Result<Vec<SlackMessage>> {
        let session = self.get_session()?;
        let mut request = SlackApiConversationsHistoryRequest::new(channel.into());

        if let Some(l) = limit {
            request = request.with_limit(l);
        }
        if let Some(o) = oldest {
            request = request.with_oldest(o.into());
        }
        if let Some(l) = latest {
            request = request.with_latest(l.into());
        }

        let response = session.conversations_history(&request).await?;
        Ok(response.messages)
    }

    /// Get user info
    pub async fn get_user_info(&self, user_id: &str) -> Result<SlackUser> {
        let session = self.get_session()?;
        let request = SlackApiUsersInfoRequest::new(user_id.into());
        let response = session.users_info(&request).await?;
        Ok(response.user)
    }

    /// List channels
    pub async fn list_channels(&self) -> Result<Vec<SlackChannel>> {
        let session = self.get_session()?;
        let request = SlackApiConversationsListRequest::new()
            .with_types(vec!["public_channel".into(), "private_channel".into()]);
        let response = session.conversations_list(&request).await?;
        Ok(response.channels)
    }

    /// Open a DM with a user
    pub async fn open_dm(&self, user_id: &str) -> Result<String> {
        let session = self.get_session()?;
        let request = SlackApiConversationsOpenRequest::new()
            .with_users(vec![user_id.into()]);
        let response = session.conversations_open(&request).await?;
        Ok(response.channel.id)
    }

    /// Reply in thread
    pub async fn reply_in_thread(
        &self,
        channel: &str,
        thread_ts: &str,
        text: &str,
    ) -> Result<SlackMessageResponse> {
        let session = self.get_session()?;
        let request = SlackApiChatPostMessageRequest::new(
            channel.into(),
            SlackMessageContent::new().with_text(text.to_string()),
        ).with_thread_ts(thread_ts.into());

        session.chat_post_message(&request).await
            .map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    fn get_session(&self) -> Result<SlackClientSession> {
        let client = self.client.as_ref().ok_or(ChannelError::NotConnected)?;
        Ok(client.open_session(&self.config.bot_token.expose_secret().into()))
    }
}
```

## Block Kit Builder

```rust
pub struct BlockKitBuilder;

impl BlockKitBuilder {
    pub fn section(text: &str) -> SlackBlock {
        SlackBlock::Section(SlackSectionBlock::new()
            .with_text(SlackBlockMarkDownText::new(text.to_string())))
    }

    pub fn divider() -> SlackBlock {
        SlackBlock::Divider(SlackDividerBlock::new())
    }

    pub fn image(url: &str, alt_text: &str) -> SlackBlock {
        SlackBlock::Image(SlackImageBlock::new(url.into(), alt_text.to_string()))
    }

    pub fn context(elements: Vec<SlackBlockElement>) -> SlackBlock {
        SlackBlock::Context(SlackContextBlock::new(elements))
    }

    pub fn button(text: &str, action_id: &str, value: Option<&str>) -> SlackBlockElement {
        let mut btn = SlackBlockButtonElement::new(
            action_id.into(),
            SlackBlockPlainText::new(text.to_string()),
        );
        if let Some(v) = value {
            btn = btn.with_value(v.to_string());
        }
        SlackBlockElement::Button(btn)
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum SlackError {
    #[error("Client not connected")]
    NotConnected,

    #[error("Invalid channel: {0}")]
    InvalidChannel(String),

    #[error("User not found: {0}")]
    UserNotFound(String),

    #[error("Rate limited, retry after {0} seconds")]
    RateLimited(u32),

    #[error("Missing scope: {0}")]
    MissingScope(String),

    #[error("Channel not found")]
    ChannelNotFound,

    #[error("Message not found")]
    MessageNotFound,

    #[error("API error: {0}")]
    ApiError(String),
}

impl From<SlackClientError> for SlackError {
    fn from(err: SlackClientError) -> Self {
        match err {
            SlackClientError::RateLimitError(info) => {
                SlackError::RateLimited(info.retry_after.unwrap_or(60))
            }
            SlackClientError::ApiError(api_err) => {
                match api_err.code.as_str() {
                    "channel_not_found" => SlackError::ChannelNotFound,
                    "user_not_found" => SlackError::UserNotFound(String::new()),
                    "missing_scope" => SlackError::MissingScope(api_err.message.unwrap_or_default()),
                    _ => SlackError::ApiError(api_err.code),
                }
            }
            _ => SlackError::ApiError(err.to_string()),
        }
    }
}
```
