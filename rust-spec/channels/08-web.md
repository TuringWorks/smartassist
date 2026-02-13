# Web Channel Specification

## Overview

Web channel for browser-based chat interface via WebSocket.

## Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = "0.20"
axum = { version = "0.7", features = ["ws"] }
tower = "0.4"
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4"] }
```

## Authentication

```rust
pub struct WebConfig {
    /// Server bind address
    pub bind_address: SocketAddr,

    /// TLS configuration
    pub tls: Option<TlsConfig>,

    /// CORS origins
    pub cors_origins: Vec<String>,

    /// Session timeout in seconds
    pub session_timeout_secs: u64,

    /// Maximum message size
    pub max_message_size: usize,

    /// Authentication mode
    pub auth: WebAuthConfig,
}

#[derive(Debug, Clone)]
pub struct TlsConfig {
    pub cert_path: PathBuf,
    pub key_path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum WebAuthConfig {
    /// No authentication required
    None,
    /// Bearer token authentication
    Token { tokens: Vec<SecretString> },
    /// Custom authentication handler
    Custom,
}
```

## Channel Implementation

```rust
pub struct WebChannel {
    config: WebConfig,
    sessions: Arc<RwLock<HashMap<String, WebSession>>>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: mpsc::Receiver<InboundMessage>,
    server_handle: Option<JoinHandle<()>>,
    connected: AtomicBool,
}

#[async_trait]
impl Channel for WebChannel {
    fn id(&self) -> &str { "web" }
    fn name(&self) -> &str { "Web" }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: false,
                voice_notes: false,
                max_file_size_mb: 100,
            },
            features: ChannelFeatures {
                reactions: true,
                threads: false,
                edits: true,
                deletes: true,
                typing_indicators: true,
                read_receipts: true,
                mentions: false,
                polls: false,
                buttons: true,
                inline_queries: false,
                commands: true,
                markdown: true,
                html: true,
            },
            limits: ChannelLimits {
                max_message_length: 65536,
                max_caption_length: 4096,
                max_buttons_per_row: 10,
                max_button_rows: 20,
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

## WebSocket Session

```rust
pub struct WebSession {
    pub id: String,
    pub user_id: String,
    pub user_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub ws_tx: mpsc::Sender<WebSocketMessage>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    /// Client -> Server: User message
    #[serde(rename = "message")]
    Message {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        media: Option<Vec<WebMedia>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reply_to: Option<String>,
    },

    /// Server -> Client: Assistant message
    #[serde(rename = "response")]
    Response {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        media: Option<Vec<WebMedia>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        buttons: Option<Vec<Vec<WebButton>>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        metadata: Option<serde_json::Value>,
    },

    /// Server -> Client: Streaming chunk
    #[serde(rename = "chunk")]
    Chunk {
        id: String,
        text: String,
        #[serde(rename = "done")]
        is_done: bool,
    },

    /// Client -> Server: Typing indicator
    #[serde(rename = "typing")]
    Typing {
        #[serde(rename = "isTyping")]
        is_typing: bool,
    },

    /// Server -> Client: Bot typing indicator
    #[serde(rename = "bot_typing")]
    BotTyping {
        #[serde(rename = "isTyping")]
        is_typing: bool,
    },

    /// Client -> Server: Read receipt
    #[serde(rename = "read")]
    Read {
        #[serde(rename = "messageId")]
        message_id: String,
    },

    /// Client -> Server: Button click
    #[serde(rename = "button_click")]
    ButtonClick {
        #[serde(rename = "messageId")]
        message_id: String,
        #[serde(rename = "buttonId")]
        button_id: String,
        data: String,
    },

    /// Server -> Client: Message edit
    #[serde(rename = "edit")]
    Edit {
        id: String,
        #[serde(rename = "newText")]
        new_text: String,
    },

    /// Server -> Client: Message delete
    #[serde(rename = "delete")]
    Delete {
        id: String,
    },

    /// Server -> Client: Error
    #[serde(rename = "error")]
    Error {
        code: String,
        message: String,
    },

    /// Ping/Pong for keep-alive
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "pong")]
    Pong,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebMedia {
    #[serde(rename = "type")]
    pub media_type: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebButton {
    pub id: String,
    pub text: String,
    #[serde(rename = "type")]
    pub button_type: String, // "callback" | "url"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}
```

## HTTP Server

```rust
impl WebChannel {
    async fn start_server(&mut self) -> Result<()> {
        let sessions = self.sessions.clone();
        let message_tx = self.message_tx.clone();
        let config = self.config.clone();

        let app = Router::new()
            .route("/ws", get(Self::websocket_handler))
            .route("/health", get(Self::health_handler))
            .route("/upload", post(Self::upload_handler))
            .layer(CorsLayer::new()
                .allow_origin(config.cors_origins.iter()
                    .map(|o| o.parse().unwrap())
                    .collect::<Vec<_>>())
                .allow_methods([Method::GET, Method::POST])
                .allow_headers(Any))
            .with_state(WebState {
                sessions,
                message_tx,
                config,
            });

        let listener = TcpListener::bind(&self.config.bind_address).await?;

        self.server_handle = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        }));

        self.connected.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn websocket_handler(
        ws: WebSocketUpgrade,
        State(state): State<WebState>,
        headers: HeaderMap,
    ) -> impl IntoResponse {
        // Authenticate
        if let Err(e) = Self::authenticate(&state.config.auth, &headers) {
            return (StatusCode::UNAUTHORIZED, e.to_string()).into_response();
        }

        ws.on_upgrade(move |socket| Self::handle_socket(socket, state))
    }

    async fn handle_socket(socket: WebSocket, state: WebState) {
        let session_id = Uuid::new_v4().to_string();
        let (ws_tx, mut ws_rx) = socket.split();
        let (tx, rx) = mpsc::channel(32);

        // Create session
        let session = WebSession {
            id: session_id.clone(),
            user_id: session_id.clone(), // Could be extracted from auth
            user_name: None,
            created_at: Utc::now(),
            last_activity: Utc::now(),
            ws_tx: tx,
            metadata: HashMap::new(),
        };

        state.sessions.write().await.insert(session_id.clone(), session);

        // Spawn sender task
        let sender = tokio::spawn(async move {
            let mut rx = ReceiverStream::new(rx);
            let mut ws_tx = ws_tx;
            while let Some(msg) = rx.next().await {
                let json = serde_json::to_string(&msg).unwrap();
                if ws_tx.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
        });

        // Receive messages
        while let Some(msg) = ws_rx.next().await {
            match msg {
                Ok(Message::Text(text)) => {
                    if let Ok(ws_msg) = serde_json::from_str::<WebSocketMessage>(&text) {
                        Self::handle_message(ws_msg, &session_id, &state).await;
                    }
                }
                Ok(Message::Close(_)) => break,
                Err(_) => break,
                _ => {}
            }
        }

        // Cleanup
        state.sessions.write().await.remove(&session_id);
        sender.abort();
    }

    async fn handle_message(msg: WebSocketMessage, session_id: &str, state: &WebState) {
        match msg {
            WebSocketMessage::Message { id, text, media, reply_to } => {
                let inbound = InboundMessage {
                    id: MessageId::new(id),
                    timestamp: Utc::now(),
                    channel: "web".to_string(),
                    account_id: "web".to_string(),
                    sender: SenderInfo {
                        id: session_id.to_string(),
                        username: None,
                        display_name: None,
                        phone_number: None,
                        is_bot: false,
                    },
                    chat: ChatInfo {
                        id: session_id.to_string(),
                        chat_type: ChatType::Direct,
                        title: None,
                        guild_id: None,
                    },
                    text,
                    media: media.map(|m| m.into_iter().map(|wm| MediaAttachment {
                        attachment_type: match wm.media_type.as_str() {
                            "image" => MediaType::Image,
                            "video" => MediaType::Video,
                            "audio" => MediaType::Audio,
                            _ => MediaType::Document,
                        },
                        url: Some(wm.url),
                        file_id: None,
                        file_path: None,
                        mime_type: wm.mime_type,
                        file_name: wm.name,
                        file_size: wm.size,
                        caption: None,
                    }).collect()).unwrap_or_default(),
                    quote: reply_to.map(|r| QuotedMessage {
                        id: r,
                        text: None,
                        sender_id: None,
                    }),
                    thread: None,
                    metadata: serde_json::json!({
                        "session_id": session_id,
                    }),
                };

                state.message_tx.send(inbound).await.ok();
            }
            WebSocketMessage::Typing { is_typing } => {
                // Handle typing indicator
            }
            WebSocketMessage::Read { message_id } => {
                // Handle read receipt
            }
            WebSocketMessage::ButtonClick { message_id, button_id, data } => {
                // Handle button click as message
            }
            WebSocketMessage::Ping => {
                // Send pong
            }
            _ => {}
        }
    }
}
```

## Sending Messages

```rust
impl WebChannel {
    async fn send_to_session(&self, session_id: &str, msg: WebSocketMessage) -> Result<()> {
        let sessions = self.sessions.read().await;
        let session = sessions.get(session_id)
            .ok_or(ChannelError::NotConnected)?;

        session.ws_tx.send(msg).await
            .map_err(|e| ChannelError::SendFailed(e.to_string()))
    }

    async fn send_response(
        &self,
        session_id: &str,
        message: &OutboundMessage,
    ) -> Result<SendResult> {
        let msg_id = Uuid::new_v4().to_string();

        let buttons = message.buttons.as_ref().map(|rows| {
            rows.iter().map(|row| {
                row.iter().map(|btn| WebButton {
                    id: Uuid::new_v4().to_string(),
                    text: btn.text.clone(),
                    button_type: match &btn.action {
                        ButtonAction::Callback(_) => "callback".to_string(),
                        ButtonAction::Url(_) => "url".to_string(),
                    },
                    data: match &btn.action {
                        ButtonAction::Callback(d) => Some(d.clone()),
                        _ => None,
                    },
                    url: match &btn.action {
                        ButtonAction::Url(u) => Some(u.clone()),
                        _ => None,
                    },
                }).collect()
            }).collect()
        });

        let media = if message.media.is_empty() {
            None
        } else {
            Some(message.media.iter().map(|m| WebMedia {
                media_type: match m.attachment_type {
                    MediaType::Image => "image".to_string(),
                    MediaType::Video => "video".to_string(),
                    MediaType::Audio => "audio".to_string(),
                    _ => "file".to_string(),
                },
                url: m.url.clone().unwrap_or_default(),
                name: m.file_name.clone(),
                size: m.file_size,
                mime_type: m.mime_type.clone(),
            }).collect())
        };

        let ws_msg = WebSocketMessage::Response {
            id: msg_id.clone(),
            text: message.text.clone(),
            media,
            buttons,
            metadata: None,
        };

        self.send_to_session(session_id, ws_msg).await?;

        Ok(SendResult {
            message_id: msg_id,
            timestamp: Utc::now(),
        })
    }

    async fn send_streaming_chunk(
        &self,
        session_id: &str,
        message_id: &str,
        text: &str,
        is_done: bool,
    ) -> Result<()> {
        let ws_msg = WebSocketMessage::Chunk {
            id: message_id.to_string(),
            text: text.to_string(),
            is_done,
        };

        self.send_to_session(session_id, ws_msg).await
    }
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum WebError {
    #[error("Server not running")]
    NotRunning,

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Authentication failed")]
    AuthenticationFailed,

    #[error("Invalid message format")]
    InvalidMessage,

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Rate limited")]
    RateLimited,

    #[error("Server error: {0}")]
    ServerError(String),
}
```
