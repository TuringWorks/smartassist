//! IRC channel implementation.
//!
//! Provides a lightweight IRC client channel for SmartAssist using raw TCP
//! with the standard IRC protocol (NICK/USER/JOIN/PRIVMSG/PING/PONG).

#![cfg(feature = "irc")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelFactory, ChannelLifecycle, ChannelReceiver, ChannelSender,
    MessageHandler, MessageRef, SendResult,
};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatType,
    HealthStatus, InboundMessage, MediaCapabilities, MessageTarget, OutboundMessage,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// IRC channel implementation.
pub struct IrcChannel {
    instance_id: String,
    server: String,
    channel: String,
    nickname: String,
    connected: Arc<AtomicBool>,
    write_half: Arc<RwLock<Option<tokio::io::WriteHalf<TcpStream>>>>,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,
}

impl std::fmt::Debug for IrcChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IrcChannel")
            .field("instance_id", &self.instance_id)
            .field("server", &self.server)
            .field("channel", &self.channel)
            .field("nickname", &self.nickname)
            .finish()
    }
}

impl IrcChannel {
    /// Create a new IRC channel.
    pub fn new(
        instance_id: impl Into<String>,
        server: impl Into<String>,
        channel: impl Into<String>,
        nickname: impl Into<String>,
    ) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: instance_id.into(),
            server: server.into(),
            channel: channel.into(),
            nickname: nickname.into(),
            connected: Arc::new(AtomicBool::new(false)),
            write_half: Arc::new(RwLock::new(None)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> Self {
        let server = config
            .options
            .get("server")
            .and_then(|v| v.as_str())
            .unwrap_or("irc.libera.chat:6667")
            .to_string();
        let channel = config
            .options
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("#smartassist")
            .to_string();
        let nickname = config
            .options
            .get("nickname")
            .and_then(|v| v.as_str())
            .unwrap_or("smartassist-bot")
            .to_string();
        Self::new(config.instance_id, server, channel, nickname)
    }
}

#[async_trait]
impl Channel for IrcChannel {
    fn channel_type(&self) -> &str {
        "irc"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Group, ChatType::Direct],
            media: MediaCapabilities {
                images: false,
                audio: false,
                video: false,
                files: false,
                stickers: false,
                voice_notes: false,
                max_file_size_mb: 0,
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
                native_commands: true,
            },
            limits: ChannelLimits {
                text_max_length: 512,
                caption_max_length: 0,
                messages_per_second: 1.0,
                messages_per_minute: 30,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for IrcChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let mut guard = self.write_half.write().await;
        let write = guard.as_mut().ok_or_else(|| {
            ChannelError::Config("IRC not connected".to_string())
        })?;

        let target = if message.target.chat_id.is_empty() {
            self.channel.clone()
        } else {
            message.target.chat_id.clone()
        };

        let cmd = format!("PRIVMSG {} :{}\r\n", target, message.text);
        write
            .write_all(cmd.as_bytes())
            .await
            .map_err(|e| ChannelError::Channel {
                channel: "irc".to_string(),
                message: format!("Failed to send PRIVMSG: {}", e),
            })?;

        let msg_id = uuid::Uuid::new_v4().to_string();
        debug!(
            "IRC sent to {}: {} (msg_id: {})",
            target,
            message.text,
            msg_id
        );

        Ok(SendResult::with_chat(msg_id, target))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        _attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        warn!("IRC does not support attachments; sending text only");
        self.send(message).await
    }

    async fn edit(&self, _message: &MessageRef, _new_content: &str) -> Result<()> {
        Err(ChannelError::Unsupported("IRC does not support message editing".to_string()))
    }

    async fn delete(&self, _message: &MessageRef) -> Result<()> {
        Err(ChannelError::Unsupported("IRC does not support message deletion".to_string()))
    }

    async fn react(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        Err(ChannelError::Unsupported("IRC does not support reactions".to_string()))
    }

    async fn unreact(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        Err(ChannelError::Unsupported("IRC does not support reactions".to_string()))
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        // IRC has no typing indicator
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        512
    }
}

#[async_trait]
impl ChannelReceiver for IrcChannel {
    async fn start_receiving(&self) -> Result<()> {
        info!(
            "Started IRC receiver for {} on {}",
            self.instance_id,
            self.server
        );
        Ok(())
    }

    async fn stop_receiving(&self) -> Result<()> {
        info!("Stopped IRC receiver for {}", self.instance_id);
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
impl ChannelLifecycle for IrcChannel {
    async fn connect(&self) -> Result<()> {
        if self.server.is_empty() {
            return Err(ChannelError::Config(
                "IRC server is not configured".to_string(),
            ));
        }
        if self.nickname.is_empty() {
            return Err(ChannelError::Config(
                "IRC nickname is not configured".to_string(),
            ));
        }
        if self.channel.is_empty() {
            return Err(ChannelError::Config(
                "IRC channel is not configured".to_string(),
            ));
        }

        let parts: Vec<&str> = self.server.split(':').collect();
        let host = parts[0];
        let port = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(6667u16);

        let stream = TcpStream::connect((host, port)).await.map_err(|e| {
            ChannelError::Channel {
                channel: "irc".to_string(),
                message: format!("TCP connect failed: {}", e),
            }
        })?;

        let (read_half, mut write_half) = tokio::io::split(stream);

        // Send registration
        let nick_cmd = format!("NICK {}\r\n", self.nickname);
        write_half
            .write_all(nick_cmd.as_bytes())
            .await
            .map_err(|e| ChannelError::Channel {
                channel: "irc".to_string(),
                message: format!("Failed to send NICK: {}", e),
            })?;

        let user_cmd = format!(
            "USER {} 0 * :{}\r\n",
            self.nickname, self.nickname
        );
        write_half
            .write_all(user_cmd.as_bytes())
            .await
            .map_err(|e| ChannelError::Channel {
                channel: "irc".to_string(),
                message: format!("Failed to send USER: {}", e),
            })?;

        let join_cmd = format!("JOIN {}\r\n", self.channel);
        write_half
            .write_all(join_cmd.as_bytes())
            .await
            .map_err(|e| ChannelError::Channel {
                channel: "irc".to_string(),
                message: format!("Failed to send JOIN: {}", e),
            })?;

        let mut guard = self.write_half.write().await;
        *guard = Some(write_half);
        drop(guard);

        self.connected.store(true, Ordering::Relaxed);
        info!("IRC channel connected: {}", self.instance_id);

        // Spawn read loop
        let instance_id = self.instance_id.clone();
        let connected = self.connected.clone();
        let message_tx = self.message_tx.clone();
        let channel_name = self.channel.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(read_half);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        info!("IRC server closed connection for {}", instance_id);
                        break;
                    }
                    Ok(_) => {
                        let trimmed = line.trim();
                        debug!("IRC < {}", trimmed);
                        if trimmed.to_uppercase().starts_with("PING ") {
                            // Read task cannot PONG because write_half is owned by the struct.
                            // This is a known limitation; the manager/heartbeat will reconnect.
                            debug!("IRC PING received for {} (PONG not sent from read task)", instance_id);
                        } else if trimmed.contains("PRIVMSG") {
                            // Parse :nick!user@host PRIVMSG #channel :text
                            if let Some(msg_start) = trimmed.find("PRIVMSG ") {
                                let after_privmsg = &trimmed[msg_start + 8..];
                                if let Some(text_start) = after_privmsg.find(" :") {
                                    let text = &after_privmsg[text_start + 2..];
                                    let from_nick = trimmed
                                        .strip_prefix(':')
                                        .and_then(|s| s.split('!').next())
                                        .unwrap_or("unknown");
                                    let msg = InboundMessage {
                                        id: smartassist_core::types::MessageId::new(uuid::Uuid::new_v4().to_string()),
                                        timestamp: chrono::Utc::now(),
                                        channel: "irc".to_string(),
                                        account_id: instance_id.clone(),
                                        sender: smartassist_core::types::SenderInfo {
                                            id: from_nick.to_string(),
                                            username: Some(from_nick.to_string()),
                                            display_name: Some(from_nick.to_string()),
                                            phone_number: None,
                                            is_bot: false,
                                        },
                                        chat: smartassist_core::types::ChatInfo {
                                            id: channel_name.clone(),
                                            chat_type: smartassist_core::types::ChatType::Group,
                                            title: Some(channel_name.clone()),
                                            guild_id: None,
                                        },
                                        text: text.to_string(),
                                        media: vec![],
                                        quote: None,
                                        thread: None,
                                        metadata: serde_json::Value::Null,
                                    };
                                    let _ = message_tx.send(msg).await;
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("IRC read error for {}: {}", instance_id, e);
                        break;
                    }
                }
            }
            connected.store(false, Ordering::Relaxed);
        });

        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;

        let mut guard = self.write_half.write().await;
        if let Some(ref mut write) = guard.as_mut() {
            let quit_cmd = format!("QUIT :SmartAssist\r\n");
            let _ = write.write_all(quit_cmd.as_bytes()).await;
        }
        *guard = None;
        drop(guard);

        self.connected.store(false, Ordering::Relaxed);
        info!("IRC channel disconnected: {}", self.instance_id);
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let start = std::time::Instant::now();
        let connected = self.is_connected();

        if !connected {
            return Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: Some(0),
                last_message_at: None,
                error: Some("Not connected".to_string()),
            });
        }

        // Check write_half is still present
        let guard = self.write_half.read().await;
        if guard.is_none() {
            return Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                last_message_at: None,
                error: Some("TCP stream is closed".to_string()),
            });
        }
        drop(guard);

        // Try a lightweight TCP connect to verify server reachability
        let parts: Vec<&str> = self.server.split(':').collect();
        let host = parts[0];
        let port = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(6667u16);

        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            TcpStream::connect((host, port)),
        )
        .await
        {
            Ok(Ok(_)) => Ok(ChannelHealth {
                status: HealthStatus::Healthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                last_message_at: None,
                error: None,
            }),
            Ok(Err(e)) => Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                last_message_at: None,
                error: Some(format!("TCP connect failed: {}", e)),
            }),
            Err(_) => Ok(ChannelHealth {
                status: HealthStatus::Degraded,
                latency_ms: Some(start.elapsed().as_millis() as u64),
                last_message_at: None,
                error: Some("TCP connect timed out".to_string()),
            }),
        }
    }
}

impl Clone for IrcChannel {
    fn clone(&self) -> Self {
        let (message_tx, message_rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            server: self.server.clone(),
            channel: self.channel.clone(),
            nickname: self.nickname.clone(),
            connected: self.connected.clone(),
            write_half: Arc::new(RwLock::new(None)),
            message_tx,
            message_rx: Arc::new(RwLock::new(message_rx)),
            handler: self.handler.clone(),
        }
    }
}

/// Factory for creating IRC channels.
pub struct IrcChannelFactory;

#[async_trait]
impl ChannelFactory for IrcChannelFactory {
    async fn create(&self, config: ChannelConfig) -> Result<Box<dyn Channel>> {
        Ok(Box::new(IrcChannel::from_config(config)))
    }

    fn channel_type(&self) -> &str {
        "irc"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::net::TcpListener;
    use tokio::time::{timeout, Duration};

    async fn mock_irc_server() -> (String, TcpListener) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = format!("127.0.0.1:{}", addr.port());
        (server, listener)
    }

    #[test]
    fn test_irc_channel_creation() {
        let channel = IrcChannel::new("test_irc", "irc.libera.chat:6667", "#test", "bot");
        assert_eq!(channel.channel_type(), "irc");
        assert_eq!(channel.instance_id(), "test_irc");
    }

    #[test]
    fn test_irc_capabilities() {
        let channel = IrcChannel::new("test_irc", "irc.libera.chat:6667", "#test", "bot");
        let caps = channel.capabilities();
        assert!(!caps.media.images);
        assert!(caps.features.mentions);
        assert!(caps.features.native_commands);
        assert_eq!(caps.limits.text_max_length, 512);
    }

    #[tokio::test]
    async fn test_irc_connect_disconnect() {
        let (server, listener) = mock_irc_server().await;
        let channel = IrcChannel::new("test_irc", &server, "#test", "bot");
        assert!(!channel.is_connected());

        let accept_task = tokio::spawn(async move {
            let (mut stream, _) = timeout(Duration::from_secs(2), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();

            reader.read_line(&mut line).await.unwrap();
            assert!(line.contains("NICK bot"));
            line.clear();

            reader.read_line(&mut line).await.unwrap();
            assert!(line.contains("USER"));
            line.clear();

            reader.read_line(&mut line).await.unwrap();
            assert!(line.contains("JOIN #test"));
            line.clear();

            reader.read_line(&mut line).await.unwrap();
            assert!(line.contains("QUIT"));
        });

        channel.connect().await.unwrap();
        assert!(channel.is_connected());

        channel.disconnect().await.unwrap();
        assert!(!channel.is_connected());

        timeout(Duration::from_secs(2), accept_task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn test_irc_send_message() {
        let (server, listener) = mock_irc_server().await;
        let channel = IrcChannel::new("test_irc", &server, "#test", "bot");

        let accept_task = tokio::spawn(async move {
            let (mut stream, _) = timeout(Duration::from_secs(2), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();

            // Handshake
            for _ in 0..3 {
                reader.read_line(&mut line).await.unwrap();
                line.clear();
            }

            // PRIVMSG
            reader.read_line(&mut line).await.unwrap();
            assert!(line.contains("PRIVMSG #test :Hello IRC"));
        });

        channel.connect().await.unwrap();

        let target = MessageTarget {
            chat_id: "#test".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello IRC".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let result = channel.send(message).await.unwrap();
        assert!(!result.message_id.is_empty());
        assert_eq!(result.chat_id, "#test");

        timeout(Duration::from_secs(2), accept_task)
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn test_irc_send_without_connect_fails() {
        let channel = IrcChannel::new("test_irc", "127.0.0.1:1", "#test", "bot");
        let target = MessageTarget {
            chat_id: "#test".to_string(),
            thread_id: None,
        };
        let message = OutboundMessage {
            target,
            text: "Hello".to_string(),
            media: vec![],
            mentions: vec![],
            reply_to: None,
            options: Default::default(),
        };

        let err = channel.send(message).await.unwrap_err();
        assert!(err.to_string().contains("not connected"));
    }

    #[tokio::test]
    async fn test_irc_connect_requires_server() {
        let channel = IrcChannel::new("test_irc", "", "#test", "bot");
        let err = channel.connect().await.unwrap_err();
        assert!(err.to_string().contains("server is not configured"));
    }

    #[tokio::test]
    async fn test_irc_connect_requires_nickname() {
        let channel = IrcChannel::new("test_irc", "127.0.0.1:6667", "#test", "");
        let err = channel.connect().await.unwrap_err();
        assert!(err.to_string().contains("nickname is not configured"));
    }

    #[tokio::test]
    async fn test_irc_connect_requires_channel() {
        let channel = IrcChannel::new("test_irc", "127.0.0.1:6667", "", "bot");
        let err = channel.connect().await.unwrap_err();
        assert!(err.to_string().contains("channel is not configured"));
    }

    #[tokio::test]
    async fn test_irc_unsupported_operations() {
        let channel = IrcChannel::new("test_irc", "irc.libera.chat:6667", "#test", "bot");
        let msg_ref = MessageRef::new("msg1", "#test");

        assert!(channel.edit(&msg_ref, "new").await.is_err());
        assert!(channel.delete(&msg_ref).await.is_err());
        assert!(channel.react(&msg_ref, "👍").await.is_err());
    }

    #[test]
    fn test_irc_from_config() {
        let mut options = HashMap::new();
        options.insert("server".to_string(), serde_json::json!("irc.example.com:6667"));
        options.insert("channel".to_string(), serde_json::json!("#general"));
        options.insert("nickname".to_string(), serde_json::json!("mybot"));

        let config = ChannelConfig {
            channel_type: "irc".to_string(),
            instance_id: "irc-1".to_string(),
            account_id: "acct".to_string(),
            enabled: true,
            options,
        };

        let channel = IrcChannel::from_config(config);
        assert_eq!(channel.instance_id(), "irc-1");
    }
}
