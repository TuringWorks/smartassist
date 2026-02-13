# WhatsApp Channel Specification

## Overview

WhatsApp channel implementation via WhatsApp Business API or whatsapp-web.js bridge.

## Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.11", features = ["json"] }
serde_json = "1.0"
```

## Authentication

```rust
pub struct WhatsAppConfig {
    /// WhatsApp Business API mode or Web bridge
    pub mode: WhatsAppMode,

    /// Business API configuration
    pub business: Option<WhatsAppBusinessConfig>,

    /// Web bridge configuration
    pub web: Option<WhatsAppWebConfig>,
}

#[derive(Debug, Clone)]
pub enum WhatsAppMode {
    /// Official WhatsApp Business API
    Business,
    /// whatsapp-web.js bridge
    Web,
}

#[derive(Debug, Clone)]
pub struct WhatsAppBusinessConfig {
    /// Phone number ID
    pub phone_number_id: String,

    /// WhatsApp Business Account ID
    pub waba_id: String,

    /// Access token
    pub access_token: SecretString,

    /// API version
    pub api_version: String,

    /// Webhook verify token
    pub verify_token: SecretString,
}

#[derive(Debug, Clone)]
pub struct WhatsAppWebConfig {
    /// Bridge server URL
    pub bridge_url: String,

    /// Session name
    pub session_name: String,

    /// Data directory for session files
    pub data_dir: PathBuf,
}
```

## Channel Implementation

```rust
pub struct WhatsAppChannel {
    config: WhatsAppConfig,
    client: reqwest::Client,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: mpsc::Receiver<InboundMessage>,
    connected: AtomicBool,
}

#[async_trait]
impl Channel for WhatsAppChannel {
    fn id(&self) -> &str { "whatsapp" }
    fn name(&self) -> &str { "WhatsApp" }

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
                max_file_size_mb: 16, // 100MB for documents
            },
            features: ChannelFeatures {
                reactions: true,
                threads: false,
                edits: false,
                deletes: true,
                typing_indicators: false,
                read_receipts: true,
                mentions: true,
                polls: true,
                buttons: true,
                inline_queries: false,
                commands: false,
                markdown: true,
                html: false,
            },
            limits: ChannelLimits {
                max_message_length: 4096,
                max_caption_length: 1024,
                max_buttons_per_row: 3,
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

## Business API Implementation

### Webhook Handler

```rust
#[derive(Debug, Deserialize)]
pub struct WhatsAppWebhook {
    pub object: String,
    pub entry: Vec<WebhookEntry>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookEntry {
    pub id: String,
    pub changes: Vec<WebhookChange>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookChange {
    pub field: String,
    pub value: WebhookValue,
}

#[derive(Debug, Deserialize)]
pub struct WebhookValue {
    pub messaging_product: String,
    pub metadata: WebhookMetadata,
    pub contacts: Option<Vec<WebhookContact>>,
    pub messages: Option<Vec<WebhookMessage>>,
    pub statuses: Option<Vec<WebhookStatus>>,
}

#[derive(Debug, Deserialize)]
pub struct WebhookMessage {
    pub from: String,
    pub id: String,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub message_type: String,
    pub text: Option<TextMessage>,
    pub image: Option<MediaMessage>,
    pub audio: Option<MediaMessage>,
    pub video: Option<MediaMessage>,
    pub document: Option<DocumentMessage>,
    pub sticker: Option<MediaMessage>,
    pub location: Option<LocationMessage>,
    pub contacts: Option<Vec<ContactMessage>>,
    pub interactive: Option<InteractiveResponse>,
    pub button: Option<ButtonResponse>,
    pub context: Option<MessageContext>,
}

impl WhatsAppChannel {
    fn convert_webhook_message(&self, msg: WebhookMessage) -> InboundMessage {
        let text = msg.text.map(|t| t.body).unwrap_or_default();

        InboundMessage {
            id: MessageId::new(msg.id.clone()),
            timestamp: DateTime::from_timestamp(msg.timestamp.parse().unwrap_or(0), 0)
                .unwrap_or_default(),
            channel: "whatsapp".to_string(),
            account_id: self.phone_number_id(),
            sender: SenderInfo {
                id: msg.from.clone(),
                username: None,
                display_name: None,
                phone_number: Some(msg.from),
                is_bot: false,
            },
            chat: ChatInfo {
                id: msg.from.clone(),
                chat_type: ChatType::Direct, // Group info in context
                title: None,
                guild_id: None,
            },
            text,
            media: self.extract_media(&msg),
            quote: msg.context.map(|c| QuotedMessage {
                id: c.id,
                text: None,
                sender_id: c.from,
            }),
            thread: None,
            metadata: serde_json::json!({
                "type": msg.message_type,
            }),
        }
    }
}
```

### Sending Messages

```rust
impl WhatsAppChannel {
    async fn send_text(&self, to: &str, message: &OutboundMessage) -> Result<SendResult> {
        let config = self.business_config()?;

        let body = serde_json::json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": to,
            "type": "text",
            "text": {
                "preview_url": !message.disable_preview,
                "body": message.text,
            }
        });

        let response = self.client
            .post(format!(
                "https://graph.facebook.com/{}/{}/messages",
                config.api_version,
                config.phone_number_id
            ))
            .bearer_auth(config.access_token.expose_secret())
            .json(&body)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;

        Ok(SendResult {
            message_id: result["messages"][0]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn send_media(&self, to: &str, media: &MediaAttachment) -> Result<SendResult> {
        let config = self.business_config()?;

        // First upload the media
        let media_id = self.upload_media(media).await?;

        let media_type = match media.attachment_type {
            MediaType::Image => "image",
            MediaType::Video => "video",
            MediaType::Audio => "audio",
            MediaType::Document => "document",
            MediaType::Sticker => "sticker",
            _ => "document",
        };

        let body = serde_json::json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": to,
            "type": media_type,
            media_type: {
                "id": media_id,
                "caption": media.caption,
            }
        });

        let response = self.client
            .post(format!(
                "https://graph.facebook.com/{}/{}/messages",
                config.api_version,
                config.phone_number_id
            ))
            .bearer_auth(config.access_token.expose_secret())
            .json(&body)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;

        Ok(SendResult {
            message_id: result["messages"][0]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn send_interactive(&self, to: &str, interactive: InteractiveMessage) -> Result<SendResult> {
        let config = self.business_config()?;

        let body = serde_json::json!({
            "messaging_product": "whatsapp",
            "recipient_type": "individual",
            "to": to,
            "type": "interactive",
            "interactive": interactive,
        });

        let response = self.client
            .post(format!(
                "https://graph.facebook.com/{}/{}/messages",
                config.api_version,
                config.phone_number_id
            ))
            .bearer_auth(config.access_token.expose_secret())
            .json(&body)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;

        Ok(SendResult {
            message_id: result["messages"][0]["id"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            timestamp: Utc::now(),
        })
    }
}
```

## Interactive Messages

```rust
#[derive(Debug, Serialize)]
pub struct InteractiveMessage {
    #[serde(rename = "type")]
    pub interactive_type: String,
    pub header: Option<InteractiveHeader>,
    pub body: InteractiveBody,
    pub footer: Option<InteractiveFooter>,
    pub action: InteractiveAction,
}

#[derive(Debug, Serialize)]
pub struct InteractiveHeader {
    #[serde(rename = "type")]
    pub header_type: String,
    pub text: Option<String>,
    pub image: Option<MediaId>,
    pub video: Option<MediaId>,
    pub document: Option<MediaId>,
}

#[derive(Debug, Serialize)]
pub struct InteractiveBody {
    pub text: String,
}

#[derive(Debug, Serialize)]
pub struct InteractiveFooter {
    pub text: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum InteractiveAction {
    Buttons { buttons: Vec<InteractiveButton> },
    List { button: String, sections: Vec<ListSection> },
    CatalogMessage { /* ... */ },
}

#[derive(Debug, Serialize)]
pub struct InteractiveButton {
    #[serde(rename = "type")]
    pub button_type: String, // "reply"
    pub reply: ButtonReply,
}

#[derive(Debug, Serialize)]
pub struct ButtonReply {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct ListSection {
    pub title: String,
    pub rows: Vec<ListRow>,
}

#[derive(Debug, Serialize)]
pub struct ListRow {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
}
```

## WhatsApp-Specific Actions

```rust
impl WhatsAppChannel {
    /// Send a template message
    pub async fn send_template(
        &self,
        to: &str,
        template_name: &str,
        language_code: &str,
        components: Vec<TemplateComponent>,
    ) -> Result<SendResult> {
        let config = self.business_config()?;

        let body = serde_json::json!({
            "messaging_product": "whatsapp",
            "to": to,
            "type": "template",
            "template": {
                "name": template_name,
                "language": {
                    "code": language_code,
                },
                "components": components,
            }
        });

        // Send request...
        todo!()
    }

    /// Mark message as read
    pub async fn mark_read(&self, message_id: &str) -> Result<()> {
        let config = self.business_config()?;

        let body = serde_json::json!({
            "messaging_product": "whatsapp",
            "status": "read",
            "message_id": message_id,
        });

        self.client
            .post(format!(
                "https://graph.facebook.com/{}/{}/messages",
                config.api_version,
                config.phone_number_id
            ))
            .bearer_auth(config.access_token.expose_secret())
            .json(&body)
            .send()
            .await?;

        Ok(())
    }

    /// Upload media
    pub async fn upload_media(&self, media: &MediaAttachment) -> Result<String> {
        let config = self.business_config()?;

        let file_path = media.file_path.as_ref()
            .ok_or(ChannelError::InvalidMessage("No file path".into()))?;

        let content = tokio::fs::read(file_path).await?;
        let mime_type = media.mime_type.clone().unwrap_or_else(|| "application/octet-stream".into());

        let form = reqwest::multipart::Form::new()
            .text("messaging_product", "whatsapp")
            .text("type", mime_type.clone())
            .part("file", reqwest::multipart::Part::bytes(content)
                .mime_str(&mime_type)?
                .file_name(media.file_name.clone().unwrap_or_else(|| "file".into())));

        let response = self.client
            .post(format!(
                "https://graph.facebook.com/{}/{}/media",
                config.api_version,
                config.phone_number_id
            ))
            .bearer_auth(config.access_token.expose_secret())
            .multipart(form)
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        result["id"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ChannelError::ActionFailed("No media ID returned".into()))
    }

    /// Get media URL
    pub async fn get_media_url(&self, media_id: &str) -> Result<String> {
        let config = self.business_config()?;

        let response = self.client
            .get(format!(
                "https://graph.facebook.com/{}/{}",
                config.api_version,
                media_id
            ))
            .bearer_auth(config.access_token.expose_secret())
            .send()
            .await?;

        let result: serde_json::Value = response.json().await?;
        result["url"].as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| ChannelError::ActionFailed("No media URL".into()))
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum WhatsAppError {
    #[error("Not connected")]
    NotConnected,

    #[error("Invalid phone number: {0}")]
    InvalidPhoneNumber(String),

    #[error("Message failed: {0}")]
    MessageFailed(String),

    #[error("Template not approved: {0}")]
    TemplateNotApproved(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Media upload failed: {0}")]
    MediaUploadFailed(String),

    #[error("Webhook verification failed")]
    WebhookVerificationFailed,

    #[error("API error: {code} - {message}")]
    ApiError { code: i32, message: String },
}
```
