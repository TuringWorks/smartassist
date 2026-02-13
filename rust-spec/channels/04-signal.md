# Signal Channel Specification

## Overview

Signal channel implementation via signal-cli JSON-RPC interface.

## Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full", "process"] }
serde_json = "1.0"
```

## Authentication

```rust
pub struct SignalConfig {
    /// Phone number with country code (+1234567890)
    pub phone_number: String,

    /// signal-cli data directory
    pub data_dir: PathBuf,

    /// signal-cli binary path
    pub signal_cli_path: PathBuf,

    /// JSON-RPC socket path
    pub socket_path: Option<PathBuf>,

    /// Trust mode for safety numbers
    pub trust_mode: TrustMode,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum TrustMode {
    /// Trust on first use
    #[default]
    Tofu,
    /// Always trust
    Always,
    /// Never trust new keys automatically
    Never,
}
```

## Channel Implementation

```rust
pub struct SignalChannel {
    config: SignalConfig,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: mpsc::Receiver<InboundMessage>,
    connected: AtomicBool,
    request_id: AtomicU64,
}

#[async_trait]
impl Channel for SignalChannel {
    fn id(&self) -> &str { "signal" }
    fn name(&self) -> &str { "Signal" }

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
                max_file_size_mb: 100,
            },
            features: ChannelFeatures {
                reactions: true,
                threads: false,
                edits: true,
                deletes: true,
                typing_indicators: true,
                read_receipts: true,
                mentions: true,
                polls: false,
                buttons: false,
                inline_queries: false,
                commands: false,
                markdown: false,
                html: false,
            },
            limits: ChannelLimits {
                max_message_length: 65536,
                max_caption_length: 2048,
                max_buttons_per_row: 0,
                max_button_rows: 0,
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

## JSON-RPC Protocol

```rust
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    data: Option<serde_json::Value>,
}

impl SignalChannel {
    async fn call_rpc<T: DeserializeOwned>(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        };

        let stdin = self.stdin.as_ref().ok_or(ChannelError::NotConnected)?;
        let request_line = serde_json::to_string(&request)? + "\n";
        stdin.write_all(request_line.as_bytes()).await?;

        // Response handling via stdout reader task
        // ...

        Ok(result)
    }
}
```

## Message Handling

### Inbound Message Conversion

```rust
#[derive(Debug, Deserialize)]
struct SignalEnvelope {
    source: String,
    source_uuid: Option<String>,
    source_device: u32,
    timestamp: u64,
    #[serde(rename = "dataMessage")]
    data_message: Option<SignalDataMessage>,
    #[serde(rename = "syncMessage")]
    sync_message: Option<SignalSyncMessage>,
    #[serde(rename = "receiptMessage")]
    receipt_message: Option<SignalReceiptMessage>,
}

#[derive(Debug, Deserialize)]
struct SignalDataMessage {
    timestamp: u64,
    message: Option<String>,
    #[serde(rename = "groupInfo")]
    group_info: Option<SignalGroupInfo>,
    attachments: Option<Vec<SignalAttachment>>,
    quote: Option<SignalQuote>,
    reaction: Option<SignalReaction>,
    mentions: Option<Vec<SignalMention>>,
}

impl SignalChannel {
    fn convert_message(&self, envelope: SignalEnvelope) -> Option<InboundMessage> {
        let data = envelope.data_message.as_ref()?;

        Some(InboundMessage {
            id: MessageId::new(data.timestamp.to_string()),
            timestamp: DateTime::from_timestamp_millis(data.timestamp as i64)?,
            channel: "signal".to_string(),
            account_id: self.config.phone_number.clone(),
            sender: SenderInfo {
                id: envelope.source_uuid.clone().unwrap_or(envelope.source.clone()),
                username: None,
                display_name: None,
                phone_number: Some(envelope.source.clone()),
                is_bot: false,
            },
            chat: ChatInfo {
                id: data.group_info.as_ref()
                    .map(|g| g.group_id.clone())
                    .unwrap_or_else(|| envelope.source.clone()),
                chat_type: if data.group_info.is_some() {
                    ChatType::Group
                } else {
                    ChatType::Direct
                },
                title: data.group_info.as_ref().and_then(|g| g.name.clone()),
                guild_id: None,
            },
            text: data.message.clone().unwrap_or_default(),
            media: self.extract_media(&data.attachments),
            quote: data.quote.as_ref().map(|q| QuotedMessage {
                id: q.id.to_string(),
                text: q.text.clone(),
                sender_id: q.author_uuid.clone().or(q.author.clone()),
            }),
            thread: None,
            metadata: serde_json::json!({
                "source_device": envelope.source_device,
                "mentions": data.mentions,
            }),
        })
    }

    fn extract_media(&self, attachments: &Option<Vec<SignalAttachment>>) -> Vec<MediaAttachment> {
        attachments.as_ref().map(|atts| {
            atts.iter().map(|a| MediaAttachment {
                attachment_type: self.guess_media_type(&a.content_type),
                url: None,
                file_id: a.id.clone(),
                file_path: a.filename.as_ref().map(PathBuf::from),
                mime_type: Some(a.content_type.clone()),
                file_name: a.filename.clone(),
                file_size: a.size.map(|s| s as u64),
                caption: None,
            }).collect()
        }).unwrap_or_default()
    }
}
```

### Outbound Message Sending

```rust
impl SignalChannel {
    async fn send_message(
        &self,
        recipient: &str,
        message: &OutboundMessage,
    ) -> Result<SendResult> {
        let mut params = serde_json::json!({
            "message": message.text,
        });

        // Determine if group or individual
        if recipient.starts_with("group.") || recipient.len() == 44 {
            params["groupId"] = serde_json::Value::String(recipient.to_string());
        } else {
            params["recipient"] = serde_json::Value::String(recipient.to_string());
        }

        // Handle quote
        if let Some(reply_to) = &message.reply_to {
            if let Ok(timestamp) = reply_to.parse::<u64>() {
                params["quoteTimestamp"] = timestamp.into();
            }
        }

        // Handle attachments
        if !message.media.is_empty() {
            let attachments: Vec<_> = message.media.iter()
                .filter_map(|m| m.file_path.as_ref())
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            params["attachments"] = serde_json::to_value(attachments)?;
        }

        let result: serde_json::Value = self.call_rpc("send", params).await?;

        Ok(SendResult {
            message_id: result["timestamp"].as_u64()
                .map(|t| t.to_string())
                .unwrap_or_default(),
            timestamp: Utc::now(),
        })
    }
}
```

## Signal-Specific Actions

```rust
impl SignalChannel {
    /// Send a reaction
    pub async fn send_reaction(
        &self,
        recipient: &str,
        target_author: &str,
        target_timestamp: u64,
        emoji: &str,
        remove: bool,
    ) -> Result<()> {
        let params = serde_json::json!({
            "recipient": recipient,
            "targetAuthor": target_author,
            "targetTimestamp": target_timestamp,
            "emoji": emoji,
            "remove": remove,
        });

        self.call_rpc::<serde_json::Value>("sendReaction", params).await?;
        Ok(())
    }

    /// Send typing indicator
    pub async fn send_typing(&self, recipient: &str, stop: bool) -> Result<()> {
        let params = serde_json::json!({
            "recipient": recipient,
            "stop": stop,
        });

        self.call_rpc::<serde_json::Value>("sendTyping", params).await?;
        Ok(())
    }

    /// Send read receipt
    pub async fn send_read_receipt(
        &self,
        recipient: &str,
        timestamps: Vec<u64>,
    ) -> Result<()> {
        let params = serde_json::json!({
            "recipient": recipient,
            "targetTimestamp": timestamps,
        });

        self.call_rpc::<serde_json::Value>("sendReceipt", params).await?;
        Ok(())
    }

    /// Get group info
    pub async fn get_group(&self, group_id: &str) -> Result<SignalGroupInfo> {
        let params = serde_json::json!({
            "groupId": group_id,
        });

        self.call_rpc("getGroup", params).await
    }

    /// List groups
    pub async fn list_groups(&self) -> Result<Vec<SignalGroupInfo>> {
        self.call_rpc("listGroups", serde_json::json!({})).await
    }

    /// Get contact name
    pub async fn get_contact(&self, number: &str) -> Result<SignalContact> {
        let params = serde_json::json!({
            "recipient": number,
        });

        self.call_rpc("getContact", params).await
    }

    /// Update profile
    pub async fn update_profile(
        &self,
        name: Option<&str>,
        about: Option<&str>,
        avatar: Option<&Path>,
    ) -> Result<()> {
        let mut params = serde_json::json!({});

        if let Some(n) = name {
            params["name"] = n.into();
        }
        if let Some(a) = about {
            params["about"] = a.into();
        }
        if let Some(av) = avatar {
            params["avatar"] = av.to_string_lossy().into_owned().into();
        }

        self.call_rpc::<serde_json::Value>("updateProfile", params).await?;
        Ok(())
    }

    /// Set disappearing messages timer
    pub async fn set_expiration(
        &self,
        recipient: &str,
        expiration_seconds: u32,
    ) -> Result<()> {
        let params = serde_json::json!({
            "recipient": recipient,
            "expiration": expiration_seconds,
        });

        self.call_rpc::<serde_json::Value>("setExpiration", params).await?;
        Ok(())
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum SignalError {
    #[error("signal-cli not connected")]
    NotConnected,

    #[error("Invalid phone number: {0}")]
    InvalidPhoneNumber(String),

    #[error("Recipient not found: {0}")]
    RecipientNotFound(String),

    #[error("Group not found: {0}")]
    GroupNotFound(String),

    #[error("Untrusted identity for {0}")]
    UntrustedIdentity(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Attachment too large")]
    AttachmentTooLarge,

    #[error("signal-cli error: {0}")]
    CliError(String),

    #[error("JSON-RPC error {code}: {message}")]
    RpcError { code: i32, message: String },
}

impl From<JsonRpcError> for SignalError {
    fn from(err: JsonRpcError) -> Self {
        match err.code {
            -1 => SignalError::CliError(err.message),
            -2 => SignalError::UntrustedIdentity(err.message),
            -3 => SignalError::RateLimited,
            _ => SignalError::RpcError {
                code: err.code,
                message: err.message,
            },
        }
    }
}
```
