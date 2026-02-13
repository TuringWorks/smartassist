# LINE Channel Specification

## Overview

LINE channel implementation via LINE Messaging API.

## Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
hmac = "0.12"
sha2 = "0.10"
base64 = "0.21"
```

## Authentication

```rust
pub struct LineConfig {
    /// Channel access token
    pub channel_access_token: SecretString,

    /// Channel secret for webhook signature verification
    pub channel_secret: SecretString,

    /// Channel ID
    pub channel_id: String,

    /// Webhook URL
    pub webhook_url: Option<String>,
}
```

## Channel Implementation

```rust
pub struct LineChannel {
    config: LineConfig,
    client: reqwest::Client,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: mpsc::Receiver<InboundMessage>,
    connected: AtomicBool,
}

#[async_trait]
impl Channel for LineChannel {
    fn id(&self) -> &str { "line" }
    fn name(&self) -> &str { "LINE" }

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
                buttons: true,
                inline_queries: false,
                commands: false,
                markdown: false,
                html: false,
            },
            limits: ChannelLimits {
                max_message_length: 5000,
                max_caption_length: 2000,
                max_buttons_per_row: 3,
                max_button_rows: 4,
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

## Webhook Handler

```rust
#[derive(Debug, Deserialize)]
pub struct LineWebhook {
    pub destination: String,
    pub events: Vec<LineEvent>,
}

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

#[derive(Debug, Deserialize)]
pub struct MessageEvent {
    #[serde(rename = "replyToken")]
    pub reply_token: String,
    pub source: EventSource,
    pub timestamp: u64,
    pub message: LineMessage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum EventSource {
    #[serde(rename = "user")]
    User { #[serde(rename = "userId")] user_id: String },
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

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum LineMessage {
    #[serde(rename = "text")]
    Text { id: String, text: String },
    #[serde(rename = "image")]
    Image { id: String, #[serde(rename = "contentProvider")] content_provider: ContentProvider },
    #[serde(rename = "video")]
    Video { id: String, duration: u64, #[serde(rename = "contentProvider")] content_provider: ContentProvider },
    #[serde(rename = "audio")]
    Audio { id: String, duration: u64, #[serde(rename = "contentProvider")] content_provider: ContentProvider },
    #[serde(rename = "file")]
    File { id: String, #[serde(rename = "fileName")] file_name: String, #[serde(rename = "fileSize")] file_size: u64 },
    #[serde(rename = "location")]
    Location { id: String, title: String, address: String, latitude: f64, longitude: f64 },
    #[serde(rename = "sticker")]
    Sticker { id: String, #[serde(rename = "packageId")] package_id: String, #[serde(rename = "stickerId")] sticker_id: String },
}

impl LineChannel {
    pub fn verify_signature(&self, body: &[u8], signature: &str) -> bool {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let mut mac = Hmac::<Sha256>::new_from_slice(
            self.config.channel_secret.expose_secret().as_bytes()
        ).expect("HMAC can take key of any size");

        mac.update(body);
        let expected = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, mac.finalize().into_bytes());
        expected == signature
    }

    fn convert_message(&self, event: MessageEvent) -> InboundMessage {
        let (sender_id, chat_id, chat_type) = match &event.source {
            EventSource::User { user_id } => (
                user_id.clone(),
                user_id.clone(),
                ChatType::Direct,
            ),
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

        let (text, media) = match &event.message {
            LineMessage::Text { text, .. } => (text.clone(), vec![]),
            LineMessage::Image { id, .. } => (String::new(), vec![MediaAttachment {
                attachment_type: MediaType::Image,
                url: None,
                file_id: Some(id.clone()),
                file_path: None,
                mime_type: Some("image/jpeg".into()),
                file_name: None,
                file_size: None,
                caption: None,
            }]),
            LineMessage::Video { id, .. } => (String::new(), vec![MediaAttachment {
                attachment_type: MediaType::Video,
                url: None,
                file_id: Some(id.clone()),
                file_path: None,
                mime_type: Some("video/mp4".into()),
                file_name: None,
                file_size: None,
                caption: None,
            }]),
            LineMessage::Audio { id, .. } => (String::new(), vec![MediaAttachment {
                attachment_type: MediaType::Audio,
                url: None,
                file_id: Some(id.clone()),
                file_path: None,
                mime_type: Some("audio/m4a".into()),
                file_name: None,
                file_size: None,
                caption: None,
            }]),
            LineMessage::File { id, file_name, file_size } => (String::new(), vec![MediaAttachment {
                attachment_type: MediaType::Document,
                url: None,
                file_id: Some(id.clone()),
                file_path: None,
                mime_type: None,
                file_name: Some(file_name.clone()),
                file_size: Some(*file_size),
                caption: None,
            }]),
            LineMessage::Location { title, address, .. } => {
                (format!("{}\n{}", title, address), vec![])
            }
            LineMessage::Sticker { package_id, sticker_id, .. } => {
                (format!("[Sticker: {}/{}]", package_id, sticker_id), vec![])
            }
        };

        InboundMessage {
            id: MessageId::new(match &event.message {
                LineMessage::Text { id, .. } => id.clone(),
                LineMessage::Image { id, .. } => id.clone(),
                LineMessage::Video { id, .. } => id.clone(),
                LineMessage::Audio { id, .. } => id.clone(),
                LineMessage::File { id, .. } => id.clone(),
                LineMessage::Location { id, .. } => id.clone(),
                LineMessage::Sticker { id, .. } => id.clone(),
            }),
            timestamp: DateTime::from_timestamp_millis(event.timestamp as i64).unwrap_or_default(),
            channel: "line".to_string(),
            account_id: self.config.channel_id.clone(),
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
}
```

## Sending Messages

```rust
impl LineChannel {
    async fn reply(&self, reply_token: &str, messages: Vec<LineOutboundMessage>) -> Result<()> {
        let body = serde_json::json!({
            "replyToken": reply_token,
            "messages": messages,
        });

        self.client
            .post("https://api.line.me/v2/bot/message/reply")
            .bearer_auth(self.config.channel_access_token.expose_secret())
            .json(&body)
            .send()
            .await?;

        Ok(())
    }

    async fn push(&self, to: &str, messages: Vec<LineOutboundMessage>) -> Result<()> {
        let body = serde_json::json!({
            "to": to,
            "messages": messages,
        });

        self.client
            .post("https://api.line.me/v2/bot/message/push")
            .bearer_auth(self.config.channel_access_token.expose_secret())
            .json(&body)
            .send()
            .await?;

        Ok(())
    }

    fn build_text_message(&self, text: &str) -> LineOutboundMessage {
        LineOutboundMessage::Text { text: text.to_string() }
    }

    fn build_image_message(&self, original_url: &str, preview_url: &str) -> LineOutboundMessage {
        LineOutboundMessage::Image {
            original_content_url: original_url.to_string(),
            preview_image_url: preview_url.to_string(),
        }
    }

    fn build_buttons_template(
        &self,
        title: &str,
        text: &str,
        actions: Vec<LineAction>,
    ) -> LineOutboundMessage {
        LineOutboundMessage::Template {
            alt_text: text.to_string(),
            template: LineTemplate::Buttons {
                title: Some(title.to_string()),
                text: text.to_string(),
                actions,
            },
        }
    }
}

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
    #[serde(rename = "flex")]
    Flex {
        #[serde(rename = "altText")]
        alt_text: String,
        contents: serde_json::Value,
    },
}

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
    Confirm {
        text: String,
        actions: Vec<LineAction>,
    },
    #[serde(rename = "carousel")]
    Carousel {
        columns: Vec<CarouselColumn>,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum LineAction {
    #[serde(rename = "message")]
    Message { label: String, text: String },
    #[serde(rename = "uri")]
    Uri { label: String, uri: String },
    #[serde(rename = "postback")]
    Postback { label: String, data: String, #[serde(rename = "displayText")] display_text: Option<String> },
}
```

## LINE-Specific Actions

```rust
impl LineChannel {
    /// Get user profile
    pub async fn get_profile(&self, user_id: &str) -> Result<LineProfile> {
        let response = self.client
            .get(format!("https://api.line.me/v2/bot/profile/{}", user_id))
            .bearer_auth(self.config.channel_access_token.expose_secret())
            .send()
            .await?;

        response.json().await.map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Get group summary
    pub async fn get_group_summary(&self, group_id: &str) -> Result<LineGroupSummary> {
        let response = self.client
            .get(format!("https://api.line.me/v2/bot/group/{}/summary", group_id))
            .bearer_auth(self.config.channel_access_token.expose_secret())
            .send()
            .await?;

        response.json().await.map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Get message content
    pub async fn get_content(&self, message_id: &str) -> Result<Vec<u8>> {
        let response = self.client
            .get(format!("https://api-data.line.me/v2/bot/message/{}/content", message_id))
            .bearer_auth(self.config.channel_access_token.expose_secret())
            .send()
            .await?;

        response.bytes().await.map(|b| b.to_vec()).map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }

    /// Leave group
    pub async fn leave_group(&self, group_id: &str) -> Result<()> {
        self.client
            .post(format!("https://api.line.me/v2/bot/group/{}/leave", group_id))
            .bearer_auth(self.config.channel_access_token.expose_secret())
            .send()
            .await?;

        Ok(())
    }

    /// Get quota
    pub async fn get_quota(&self) -> Result<LineQuota> {
        let response = self.client
            .get("https://api.line.me/v2/bot/message/quota")
            .bearer_auth(self.config.channel_access_token.expose_secret())
            .send()
            .await?;

        response.json().await.map_err(|e| ChannelError::ActionFailed(e.to_string()))
    }
}

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

#[derive(Debug, Deserialize)]
pub struct LineGroupSummary {
    #[serde(rename = "groupId")]
    pub group_id: String,
    #[serde(rename = "groupName")]
    pub group_name: String,
    #[serde(rename = "pictureUrl")]
    pub picture_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LineQuota {
    #[serde(rename = "type")]
    pub quota_type: String,
    pub value: i64,
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum LineError {
    #[error("Not connected")]
    NotConnected,

    #[error("Invalid signature")]
    InvalidSignature,

    #[error("Invalid user ID: {0}")]
    InvalidUserId(String),

    #[error("Reply token expired")]
    ReplyTokenExpired,

    #[error("Rate limited")]
    RateLimited,

    #[error("Quota exceeded")]
    QuotaExceeded,

    #[error("API error: {0}")]
    ApiError(String),
}
```
