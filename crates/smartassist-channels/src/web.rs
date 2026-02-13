//! Web channel implementation using WebSocket.
//!
//! This module provides a WebSocket-based channel for web clients.
//! Features:
//! - JSON-based message protocol
//! - Session/authentication support
//! - Broadcast to all connected clients
//! - Individual client messaging

#![cfg(feature = "web")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver, ChannelSender, MessageHandler,
    MessageRef, SendResult,
};
use crate::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::{SinkExt, StreamExt};
use smartassist_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatInfo, ChatType,
    HealthStatus, InboundMessage, MediaCapabilities, MessageId, MessageTarget, OutboundMessage,
    SenderInfo,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};
use tracing::{debug, error, info, warn};

/// Web channel implementation using WebSocket.
pub struct WebChannel {
    /// Channel instance ID.
    instance_id: String,

    /// Bind address for WebSocket server.
    bind_address: String,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Connected clients.
    clients: Arc<RwLock<HashMap<String, WebClient>>>,

    /// Incoming message channel.
    #[allow(dead_code)]
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Broadcast channel for outgoing messages.
    broadcast_tx: broadcast::Sender<String>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

/// A connected web client.
#[derive(Debug, Clone)]
pub struct WebClient {
    /// Client ID.
    pub id: String,

    /// Display name.
    pub name: Option<String>,

    /// Connected timestamp.
    pub connected_at: DateTime<Utc>,

    /// Client's peer address.
    pub peer_addr: Option<SocketAddr>,
}

/// Message types for the WebSocket protocol.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WebSocketMessage {
    /// Client authentication/identification.
    Auth {
        client_id: String,
        name: Option<String>,
        token: Option<String>,
    },
    /// Text message from client.
    Message {
        text: String,
        #[serde(default)]
        chat_id: Option<String>,
    },
    /// Typing indicator.
    Typing {
        #[serde(default)]
        chat_id: Option<String>,
    },
    /// Ping to keep connection alive.
    Ping,
    /// Pong response.
    Pong,
}

/// Outbound message types sent to clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundWebMessage {
    /// Authentication response.
    AuthResult {
        success: bool,
        client_id: String,
        error: Option<String>,
    },
    /// Text message.
    Message {
        message_id: String,
        text: String,
        target: String,
        timestamp: String,
    },
    /// Edit notification.
    Edit {
        message_id: String,
        chat_id: String,
        text: String,
        timestamp: String,
    },
    /// Delete notification.
    Delete {
        message_id: String,
        chat_id: String,
        timestamp: String,
    },
    /// Reaction added.
    React {
        message_id: String,
        chat_id: String,
        emoji: String,
        timestamp: String,
    },
    /// Reaction removed.
    Unreact {
        message_id: String,
        chat_id: String,
        emoji: String,
        timestamp: String,
    },
    /// Typing indicator.
    Typing {
        target: String,
        timestamp: String,
    },
    /// Pong response.
    Pong,
    /// Error message.
    Error {
        message: String,
    },
}

impl std::fmt::Debug for WebChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebChannel")
            .field("instance_id", &self.instance_id)
            .field("bind_address", &self.bind_address)
            .finish()
    }
}

impl WebChannel {
    /// Create a new Web channel.
    pub fn new(instance_id: impl Into<String>, bind_address: impl Into<String>) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        let (broadcast_tx, _) = broadcast::channel(1000);

        Self {
            instance_id: instance_id.into(),
            bind_address: bind_address.into(),
            connected: Arc::new(RwLock::new(false)),
            clients: Arc::new(RwLock::new(HashMap::new())),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            broadcast_tx,
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> Self {
        let bind_address = config
            .options
            .get("bind_address")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:8080")
            .to_string();

        Self::new(config.instance_id, bind_address)
    }

    /// Get the number of connected clients.
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }

    /// Broadcast a message to all connected clients.
    pub fn broadcast(&self, message: &str) -> std::result::Result<usize, broadcast::error::SendError<String>> {
        self.broadcast_tx.send(message.to_string())
    }
}

#[async_trait]
impl Channel for WebChannel {
    fn channel_type(&self) -> &str {
        "web"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct],
            media: MediaCapabilities {
                images: true,
                audio: true,
                video: true,
                files: true,
                stickers: false,
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
                mentions: false,
                polls: false,
                native_commands: false,
            },
            limits: ChannelLimits {
                text_max_length: 100000,
                caption_max_length: 1000,
                messages_per_second: 100.0,
                messages_per_minute: 6000,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for WebChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let msg_id = uuid::Uuid::new_v4().to_string();

        let payload = OutboundWebMessage::Message {
            message_id: msg_id.clone(),
            text: message.text,
            target: message.target.chat_id,
            timestamp: Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);

        Ok(SendResult::new(msg_id))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        // For web channel, attachments would be sent as base64 or URLs
        debug!(
            "Web channel attachments: {} files (sent as text message)",
            attachments.len()
        );
        self.send(message).await
    }

    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()> {
        let payload = OutboundWebMessage::Edit {
            message_id: message.message_id.clone(),
            chat_id: message.chat_id.clone(),
            text: new_content.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    async fn delete(&self, message: &MessageRef) -> Result<()> {
        let payload = OutboundWebMessage::Delete {
            message_id: message.message_id.clone(),
            chat_id: message.chat_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let payload = OutboundWebMessage::React {
            message_id: message.message_id.clone(),
            chat_id: message.chat_id.clone(),
            emoji: emoji.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let payload = OutboundWebMessage::Unreact {
            message_id: message.message_id.clone(),
            chat_id: message.chat_id.clone(),
            emoji: emoji.to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    async fn send_typing(&self, target: &MessageTarget) -> Result<()> {
        let payload = OutboundWebMessage::Typing {
            target: target.chat_id.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_string(&payload)
            .map_err(|e| ChannelError::Internal(e.to_string()))?;

        let _ = self.broadcast_tx.send(json);
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        100000
    }
}

#[async_trait]
impl ChannelReceiver for WebChannel {
    async fn start_receiving(&self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        // Parse bind address
        let addr: SocketAddr = self.bind_address.parse().map_err(|e| {
            ChannelError::channel("web", format!("Invalid bind address '{}': {}", self.bind_address, e))
        })?;

        // Start TCP listener
        let listener = TcpListener::bind(addr).await.map_err(|e| {
            ChannelError::channel("web", format!("Failed to bind to {}: {}", addr, e))
        })?;

        info!("WebSocket server listening on {}", addr);

        {
            let mut connected = self.connected.write().await;
            *connected = true;
        }

        // Spawn the server task
        let clients = self.clients.clone();
        let message_tx = self.message_tx.clone();
        let broadcast_tx = self.broadcast_tx.clone();
        let handler = self.handler.clone();
        let instance_id = self.instance_id.clone();
        let connected = self.connected.clone();

        tokio::spawn(async move {
            run_websocket_server(
                listener,
                clients,
                message_tx,
                broadcast_tx,
                handler,
                instance_id,
                connected,
                shutdown_rx,
            )
            .await;
        });

        info!(
            "Started Web channel on {} (instance: {})",
            self.bind_address, self.instance_id
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
impl ChannelLifecycle for WebChannel {
    async fn connect(&self) -> Result<()> {
        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Web channel connected: {}", self.instance_id);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;

        let mut connected = self.connected.write().await;
        *connected = false;

        // Clear clients
        let mut clients = self.clients.write().await;
        clients.clear();

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let connected = *self.connected.read().await;
        let _client_count = self.clients.read().await.len();

        Ok(ChannelHealth {
            status: if connected {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            },
            latency_ms: Some(0), // Local connection
            last_message_at: None,
            error: if connected {
                None
            } else {
                Some("Not connected".to_string())
            },
        })
    }
}

impl Clone for WebChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            bind_address: self.bind_address.clone(),
            connected: self.connected.clone(),
            clients: self.clients.clone(),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            broadcast_tx: self.broadcast_tx.clone(),
            handler: self.handler.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

// --- WebSocket Server Implementation ---

/// Run the WebSocket server.
async fn run_websocket_server(
    listener: TcpListener,
    clients: Arc<RwLock<HashMap<String, WebClient>>>,
    message_tx: mpsc::Sender<InboundMessage>,
    broadcast_tx: broadcast::Sender<String>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
    instance_id: String,
    connected: Arc<RwLock<bool>>,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) {
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        debug!("New WebSocket connection from {}", peer_addr);
                        let clients = clients.clone();
                        let message_tx = message_tx.clone();
                        let broadcast_rx = broadcast_tx.subscribe();
                        let handler = handler.clone();
                        let instance_id = instance_id.clone();

                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(
                                stream,
                                peer_addr,
                                clients,
                                message_tx,
                                broadcast_rx,
                                handler,
                                instance_id,
                            ).await {
                                warn!("Connection error from {}: {}", peer_addr, e);
                            }
                        });
                    }
                    Err(e) => {
                        error!("Failed to accept connection: {}", e);
                    }
                }
            }
            _ = &mut shutdown_rx => {
                info!("WebSocket server shutting down");
                break;
            }
        }
    }

    // Mark as disconnected
    let mut connected = connected.write().await;
    *connected = false;
}

/// Handle a single WebSocket connection.
async fn handle_connection(
    stream: TcpStream,
    peer_addr: SocketAddr,
    clients: Arc<RwLock<HashMap<String, WebClient>>>,
    message_tx: mpsc::Sender<InboundMessage>,
    mut broadcast_rx: broadcast::Receiver<String>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
    instance_id: String,
) -> std::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Perform WebSocket handshake
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sink, mut ws_stream) = ws_stream.split();

    // Generate a temporary client ID until auth
    let mut client_id = format!("anon_{}", uuid::Uuid::new_v4());
    let mut client_name: Option<String> = None;
    let mut authenticated = false;

    // Add to clients list
    {
        let mut clients = clients.write().await;
        clients.insert(
            client_id.clone(),
            WebClient {
                id: client_id.clone(),
                name: None,
                connected_at: Utc::now(),
                peer_addr: Some(peer_addr),
            },
        );
    }

    debug!("Client {} connected from {}", client_id, peer_addr);

    loop {
        tokio::select! {
            // Handle incoming messages from client
            msg = ws_stream.next() => {
                match msg {
                    Some(Ok(WsMessage::Text(text))) => {
                        match serde_json::from_str::<WebSocketMessage>(&text) {
                            Ok(ws_msg) => {
                                match ws_msg {
                                    WebSocketMessage::Auth { client_id: new_id, name, token: _ } => {
                                        // Update client ID and name
                                        let old_id = client_id.clone();
                                        client_id = new_id.clone();
                                        client_name = name.clone();
                                        authenticated = true;

                                        // Update clients map
                                        {
                                            let mut clients = clients.write().await;
                                            clients.remove(&old_id);
                                            clients.insert(
                                                client_id.clone(),
                                                WebClient {
                                                    id: client_id.clone(),
                                                    name: name.clone(),
                                                    connected_at: Utc::now(),
                                                    peer_addr: Some(peer_addr),
                                                },
                                            );
                                        }

                                        // Send auth response
                                        let response = OutboundWebMessage::AuthResult {
                                            success: true,
                                            client_id: client_id.clone(),
                                            error: None,
                                        };
                                        let json = serde_json::to_string(&response)?;
                                        ws_sink.send(WsMessage::Text(json)).await?;

                                        info!("Client {} authenticated as '{}'", client_id, name.unwrap_or_default());
                                    }
                                    WebSocketMessage::Message { text, chat_id } => {
                                        // Convert to InboundMessage
                                        let inbound = InboundMessage {
                                            id: MessageId::new(uuid::Uuid::new_v4().to_string()),
                                            timestamp: Utc::now(),
                                            channel: "web".to_string(),
                                            account_id: instance_id.clone(),
                                            sender: SenderInfo {
                                                id: client_id.clone(),
                                                username: client_name.clone(),
                                                display_name: client_name.clone(),
                                                phone_number: None,
                                                is_bot: false,
                                            },
                                            chat: ChatInfo {
                                                id: chat_id.unwrap_or_else(|| client_id.clone()),
                                                chat_type: ChatType::Direct,
                                                title: None,
                                                guild_id: None,
                                            },
                                            text,
                                            media: vec![],
                                            quote: None,
                                            thread: None,
                                            metadata: serde_json::json!({
                                                "peer_addr": peer_addr.to_string(),
                                                "authenticated": authenticated,
                                            }),
                                        };

                                        // Call handler if set
                                        {
                                            let handler_guard = handler.read().await;
                                            if let Some(ref h) = *handler_guard {
                                                if let Err(e) = h.handle(inbound.clone()).await {
                                                    warn!("Message handler error: {}", e);
                                                }
                                            }
                                        }

                                        // Send to message channel
                                        if let Err(e) = message_tx.send(inbound).await {
                                            warn!("Failed to send message to channel: {}", e);
                                        }
                                    }
                                    WebSocketMessage::Ping => {
                                        let response = OutboundWebMessage::Pong;
                                        let json = serde_json::to_string(&response)?;
                                        ws_sink.send(WsMessage::Text(json)).await?;
                                    }
                                    WebSocketMessage::Pong => {
                                        // Client responded to our ping
                                    }
                                    WebSocketMessage::Typing { chat_id: _ } => {
                                        // Could broadcast typing indicator to other clients
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse message from {}: {}", client_id, e);
                                let response = OutboundWebMessage::Error {
                                    message: format!("Invalid message format: {}", e),
                                };
                                let json = serde_json::to_string(&response)?;
                                ws_sink.send(WsMessage::Text(json)).await?;
                            }
                        }
                    }
                    Some(Ok(WsMessage::Binary(_))) => {
                        // Binary messages not supported yet
                        warn!("Binary message from {} - not supported", client_id);
                    }
                    Some(Ok(WsMessage::Ping(data))) => {
                        ws_sink.send(WsMessage::Pong(data)).await?;
                    }
                    Some(Ok(WsMessage::Pong(_))) => {
                        // Pong received
                    }
                    Some(Ok(WsMessage::Close(_))) => {
                        debug!("Client {} sent close frame", client_id);
                        break;
                    }
                    Some(Ok(WsMessage::Frame(_))) => {
                        // Raw frame - ignore
                    }
                    Some(Err(e)) => {
                        warn!("WebSocket error from {}: {}", client_id, e);
                        break;
                    }
                    None => {
                        debug!("Client {} stream ended", client_id);
                        break;
                    }
                }
            }
            // Handle outgoing broadcast messages
            result = broadcast_rx.recv() => {
                match result {
                    Ok(msg) => {
                        if let Err(e) = ws_sink.send(WsMessage::Text(msg)).await {
                            warn!("Failed to send to {}: {}", client_id, e);
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("Client {} lagged {} messages", client_id, n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        debug!("Broadcast channel closed");
                        break;
                    }
                }
            }
        }
    }

    // Remove from clients list
    {
        let mut clients = clients.write().await;
        clients.remove(&client_id);
    }

    info!("Client {} disconnected", client_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_web_channel_creation() {
        let channel = WebChannel::new("test_web", "127.0.0.1:8080");
        assert_eq!(channel.channel_type(), "web");
        assert_eq!(channel.instance_id(), "test_web");
    }

    #[test]
    fn test_capabilities() {
        let channel = WebChannel::new("test_web", "127.0.0.1:8080");
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.features.typing_indicators);
        assert!(caps.features.edits);
        assert_eq!(caps.limits.text_max_length, 100000);
    }

    #[tokio::test]
    async fn test_connect_disconnect() {
        let channel = WebChannel::new("test_web", "127.0.0.1:8080");

        // Check initial state via the internal lock
        assert!(!*channel.connected.read().await);

        channel.connect().await.unwrap();
        assert!(*channel.connected.read().await);

        channel.disconnect().await.unwrap();
        assert!(!*channel.connected.read().await);
    }

    #[test]
    fn test_websocket_message_parsing() {
        // Test auth message
        let auth_json = r#"{"type":"auth","client_id":"user123","name":"Test User","token":null}"#;
        let parsed: WebSocketMessage = serde_json::from_str(auth_json).unwrap();
        match parsed {
            WebSocketMessage::Auth { client_id, name, token } => {
                assert_eq!(client_id, "user123");
                assert_eq!(name, Some("Test User".to_string()));
                assert!(token.is_none());
            }
            _ => panic!("Expected Auth message"),
        }

        // Test message
        let msg_json = r#"{"type":"message","text":"Hello world","chat_id":"chat1"}"#;
        let parsed: WebSocketMessage = serde_json::from_str(msg_json).unwrap();
        match parsed {
            WebSocketMessage::Message { text, chat_id } => {
                assert_eq!(text, "Hello world");
                assert_eq!(chat_id, Some("chat1".to_string()));
            }
            _ => panic!("Expected Message"),
        }

        // Test ping
        let ping_json = r#"{"type":"ping"}"#;
        let parsed: WebSocketMessage = serde_json::from_str(ping_json).unwrap();
        assert!(matches!(parsed, WebSocketMessage::Ping));
    }

    #[test]
    fn test_outbound_message_serialization() {
        // Test message serialization
        let msg = OutboundWebMessage::Message {
            message_id: "msg123".to_string(),
            text: "Hello".to_string(),
            target: "user1".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"message\""));
        assert!(json.contains("\"message_id\":\"msg123\""));

        // Test edit serialization
        let edit = OutboundWebMessage::Edit {
            message_id: "msg123".to_string(),
            chat_id: "chat1".to_string(),
            text: "Edited".to_string(),
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&edit).unwrap();
        assert!(json.contains("\"type\":\"edit\""));
    }
}
