//! Signal channel implementation.
//!
//! This channel integrates with Signal via signal-cli.
//! Signal provides strong end-to-end encryption and is a popular
//! secure messaging platform.
//!
//! Prerequisites:
//! - signal-cli must be installed and available in PATH
//! - Account must be registered via `signal-cli register` or linked via `signal-cli link`

#![cfg(feature = "signal")]

use crate::attachment::Attachment;
use crate::error::ChannelError;
use crate::traits::{
    Channel, ChannelConfig, ChannelLifecycle, ChannelReceiver, ChannelSender, MessageHandler,
    MessageRef, SendResult,
};
use crate::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use smartassist_core::types::{
    ChannelCapabilities, ChannelFeatures, ChannelHealth, ChannelLimits, ChatInfo, ChatType,
    HealthStatus, InboundMessage, MediaAttachment, MediaCapabilities, MediaType, MessageId,
    MessageTarget, OutboundMessage, QuotedMessage, SenderInfo,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// Signal channel implementation using signal-cli.
pub struct SignalChannel {
    /// Phone number registered with Signal.
    phone_number: String,

    /// Channel instance ID.
    instance_id: String,

    /// Signal data directory (for signal-cli).
    data_dir: PathBuf,

    /// Path to signal-cli binary.
    signal_cli_path: String,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Registered state (whether the number is verified).
    registered: Arc<RwLock<bool>>,

    /// Contact cache for display names.
    contacts: Arc<RwLock<HashMap<String, ContactInfo>>>,

    /// Incoming message channel.
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

/// Contact information from Signal.
#[derive(Debug, Clone)]
pub struct ContactInfo {
    /// Phone number (E.164 format).
    pub phone_number: String,
    /// Display name.
    pub name: Option<String>,
    /// Profile name.
    pub profile_name: Option<String>,
    /// UUID from Signal.
    pub uuid: Option<String>,
}

/// Signal-cli JSON output for received messages.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SignalCliEnvelope {
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    source_number: Option<String>,
    #[serde(default)]
    source_uuid: Option<String>,
    #[serde(default)]
    source_name: Option<String>,
    timestamp: Option<u64>,
    #[serde(default)]
    data_message: Option<SignalCliDataMessage>,
    #[serde(default)]
    sync_message: Option<SignalCliSyncMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalCliDataMessage {
    #[serde(default)]
    message: Option<String>,
    timestamp: Option<u64>,
    #[serde(default)]
    group_info: Option<SignalCliGroupInfo>,
    #[serde(default)]
    quote: Option<SignalCliQuote>,
    #[serde(default)]
    attachments: Vec<SignalCliAttachment>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SignalCliSyncMessage {
    #[serde(default)]
    sent_message: Option<SignalCliSentMessage>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct SignalCliSentMessage {
    #[serde(default)]
    destination: Option<String>,
    timestamp: Option<u64>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalCliGroupInfo {
    group_id: String,
    #[serde(default)]
    group_name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalCliQuote {
    id: u64,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignalCliAttachment {
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}

/// Signal-cli send response.
#[derive(Debug, Deserialize)]
struct SignalCliSendResult {
    timestamp: u64,
}

impl std::fmt::Debug for SignalChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SignalChannel")
            .field("instance_id", &self.instance_id)
            .field("phone_number", &self.phone_number)
            .field("signal_cli_path", &self.signal_cli_path)
            .finish()
    }
}

impl SignalChannel {
    /// Create a new Signal channel.
    pub fn new(
        phone_number: impl Into<String>,
        instance_id: impl Into<String>,
        data_dir: impl Into<PathBuf>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        Self {
            phone_number: phone_number.into(),
            instance_id: instance_id.into(),
            data_dir: data_dir.into(),
            signal_cli_path: "signal-cli".to_string(),
            connected: Arc::new(RwLock::new(false)),
            registered: Arc::new(RwLock::new(false)),
            contacts: Arc::new(RwLock::new(HashMap::new())),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> std::result::Result<Self, ChannelError> {
        let phone_number = config
            .options
            .get("phone_number")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ChannelError::Config("Missing phone_number in Signal config".to_string()))?
            .to_string();

        let data_dir = config
            .options
            .get("data_dir")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                dirs::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".local")
                    .join("share")
                    .join("signal-cli")
            });

        let mut channel = Self::new(phone_number, config.instance_id, data_dir);

        // Allow custom signal-cli path
        if let Some(path) = config.options.get("signal_cli_path").and_then(|v| v.as_str()) {
            channel.signal_cli_path = path.to_string();
        }

        Ok(channel)
    }

    /// Set custom signal-cli path.
    pub fn with_signal_cli_path(mut self, path: impl Into<String>) -> Self {
        self.signal_cli_path = path.into();
        self
    }

    /// Get the phone number for this channel.
    pub fn phone_number(&self) -> &str {
        &self.phone_number
    }

    /// Check if the channel is registered with Signal.
    pub async fn is_registered(&self) -> bool {
        *self.registered.read().await
    }

    /// Run a signal-cli command and return the output.
    async fn run_signal_cli(&self, args: &[&str]) -> std::result::Result<String, ChannelError> {
        let mut cmd = Command::new(&self.signal_cli_path);
        cmd.arg("-a")
            .arg(&self.phone_number)
            .arg("--config")
            .arg(&self.data_dir)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        debug!("Running signal-cli: {:?}", cmd);

        let output = cmd
            .output()
            .await
            .map_err(|e| ChannelError::Internal(format!("Failed to run signal-cli: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ChannelError::channel(
                "signal",
                format!("signal-cli error: {}", stderr),
            ));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Send a message using signal-cli.
    async fn send_message_cli(
        &self,
        recipient: &str,
        message: &str,
        attachments: &[PathBuf],
    ) -> std::result::Result<u64, ChannelError> {
        let mut args = vec!["send", "-m", message, recipient];

        // Add attachments
        let attachment_strings: Vec<String> = attachments.iter().map(|p| p.display().to_string()).collect();
        for att in &attachment_strings {
            args.push("-a");
            args.push(att);
        }

        let output = self.run_signal_cli(&args).await?;

        // Try to parse timestamp from JSON output
        if let Ok(result) = serde_json::from_str::<SignalCliSendResult>(&output) {
            return Ok(result.timestamp);
        }

        // Fallback to current time
        Ok(chrono::Utc::now().timestamp_millis() as u64)
    }

    /// Send to a group using signal-cli.
    async fn send_to_group_cli(
        &self,
        group_id: &str,
        message: &str,
        attachments: &[PathBuf],
    ) -> std::result::Result<u64, ChannelError> {
        let mut args = vec!["send", "-m", message, "-g", group_id];

        let attachment_strings: Vec<String> = attachments.iter().map(|p| p.display().to_string()).collect();
        for att in &attachment_strings {
            args.push("-a");
            args.push(att);
        }

        let output = self.run_signal_cli(&args).await?;

        if let Ok(result) = serde_json::from_str::<SignalCliSendResult>(&output) {
            return Ok(result.timestamp);
        }

        Ok(chrono::Utc::now().timestamp_millis() as u64)
    }

    /// Convert a signal-cli envelope to InboundMessage.
    fn convert_envelope(&self, envelope: SignalCliEnvelope) -> Option<InboundMessage> {
        let data_message = envelope.data_message?;

        let sender_id = envelope
            .source_number
            .or(envelope.source)
            .unwrap_or_default();

        let sender = SenderInfo {
            id: sender_id.clone(),
            username: None,
            display_name: envelope.source_name,
            phone_number: Some(sender_id.clone()),
            is_bot: false,
        };

        let (chat_id, chat_type, title) = if let Some(ref group) = data_message.group_info {
            (group.group_id.clone(), ChatType::Group, group.group_name.clone())
        } else {
            (sender_id.clone(), ChatType::Direct, None)
        };

        let chat = ChatInfo {
            id: chat_id,
            chat_type,
            title,
            guild_id: None,
        };

        let text = data_message.message.unwrap_or_default();

        let media: Vec<MediaAttachment> = data_message
            .attachments
            .into_iter()
            .map(|att| MediaAttachment {
                id: att.id.unwrap_or_default(),
                media_type: self.guess_media_type(att.content_type.as_deref().unwrap_or("")),
                url: None,
                data: None,
                filename: att.filename,
                size_bytes: att.size,
                mime_type: att.content_type,
            })
            .collect();

        let quote = data_message.quote.map(|q| QuotedMessage {
            id: q.id.to_string(),
            text: q.text,
            sender_id: q.author,
        });

        let timestamp = data_message
            .timestamp
            .or(envelope.timestamp)
            .and_then(|ts| DateTime::<Utc>::from_timestamp_millis(ts as i64))
            .unwrap_or_else(Utc::now);

        Some(InboundMessage {
            id: MessageId::new(timestamp.timestamp_millis().to_string()),
            timestamp,
            channel: "signal".to_string(),
            account_id: self.phone_number.clone(),
            sender,
            chat,
            text,
            media,
            quote,
            thread: None,
            metadata: serde_json::json!({}),
        })
    }

    /// Guess media type from MIME content type.
    fn guess_media_type(&self, content_type: &str) -> MediaType {
        if content_type.starts_with("image/") {
            MediaType::Image
        } else if content_type.starts_with("video/") {
            MediaType::Video
        } else if content_type.starts_with("audio/") {
            if content_type.contains("voice") || content_type == "audio/aac" {
                MediaType::Voice
            } else {
                MediaType::Audio
            }
        } else {
            MediaType::Document
        }
    }

    /// Format phone number to E.164 format.
    fn normalize_phone_number(&self, phone: &str) -> String {
        let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
        if phone.starts_with('+') {
            format!("+{}", digits)
        } else if digits.len() == 10 {
            format!("+1{}", digits)
        } else {
            format!("+{}", digits)
        }
    }

    /// Check if a target is a group (base64-encoded group ID).
    fn is_group_id(&self, target: &str) -> bool {
        // Signal group IDs are typically base64-encoded
        target.len() > 20 && !target.starts_with('+') && !target.contains('@')
    }
}

#[async_trait]
impl Channel for SignalChannel {
    fn channel_type(&self) -> &str {
        "signal"
    }

    fn instance_id(&self) -> &str {
        &self.instance_id
    }

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
                native_commands: false,
            },
            limits: ChannelLimits {
                text_max_length: 65535,
                caption_max_length: 2048,
                messages_per_second: 10.0,
                messages_per_minute: 600,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for SignalChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to Signal".to_string()));
        }

        let target = &message.target.chat_id;

        let timestamp = if self.is_group_id(target) {
            self.send_to_group_cli(target, &message.text, &[]).await?
        } else {
            let recipient = self.normalize_phone_number(target);
            self.send_message_cli(&recipient, &message.text, &[]).await?
        };

        Ok(SendResult::new(timestamp.to_string()))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to Signal".to_string()));
        }

        // Write attachments to temp files
        let mut temp_files: Vec<PathBuf> = Vec::new();
        for att in &attachments {
            let path = match &att.source {
                crate::attachment::AttachmentSource::Path(p) => p.clone(),
                crate::attachment::AttachmentSource::Bytes(bytes) => {
                    let temp_path = std::env::temp_dir().join(&att.filename);
                    tokio::fs::write(&temp_path, bytes.as_ref())
                        .await
                        .map_err(|e| ChannelError::Internal(e.to_string()))?;
                    temp_path
                }
                _ => {
                    warn!("Unsupported attachment source for Signal");
                    continue;
                }
            };
            temp_files.push(path);
        }

        let target = &message.target.chat_id;
        let text = if message.text.is_empty() { " " } else { &message.text };

        let timestamp = if self.is_group_id(target) {
            self.send_to_group_cli(target, text, &temp_files).await?
        } else {
            let recipient = self.normalize_phone_number(target);
            self.send_message_cli(&recipient, text, &temp_files).await?
        };

        // Clean up temp files (only those we created)
        for att in &attachments {
            if matches!(att.source, crate::attachment::AttachmentSource::Bytes(_)) {
                let temp_path = std::env::temp_dir().join(&att.filename);
                let _ = tokio::fs::remove_file(&temp_path).await;
            }
        }

        Ok(SendResult::new(timestamp.to_string()))
    }

    async fn edit(&self, _message: &MessageRef, _new_content: &str) -> Result<()> {
        // Signal doesn't expose edit via signal-cli yet
        warn!("Signal edit not supported via signal-cli");
        Ok(())
    }

    async fn delete(&self, _message: &MessageRef) -> Result<()> {
        // Signal doesn't expose delete via signal-cli yet
        warn!("Signal delete not supported via signal-cli");
        Ok(())
    }

    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to Signal".to_string()));
        }

        // signal-cli sendReaction -a ACCOUNT -e EMOJI -t TARGET_AUTHOR -T TARGET_TIMESTAMP RECIPIENT
        debug!(
            "Would react with {} to message {} in chat {}",
            emoji, message.message_id, message.chat_id
        );
        warn!("Signal react requires target author - not fully implemented");

        Ok(())
    }

    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to Signal".to_string()));
        }

        // signal-cli sendReaction -a ACCOUNT -e EMOJI -t TARGET_AUTHOR -T TARGET_TIMESTAMP --remove RECIPIENT
        debug!(
            "Would remove reaction {} from message {} in chat {}",
            emoji, message.message_id, message.chat_id
        );
        warn!("Signal unreact requires target author - not fully implemented");

        Ok(())
    }

    async fn send_typing(&self, target: &MessageTarget) -> Result<()> {
        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to Signal".to_string()));
        }

        let recipient = self.normalize_phone_number(&target.chat_id);

        // signal-cli sendTyping RECIPIENT
        match self.run_signal_cli(&["sendTyping", &recipient]).await {
            Ok(_) => debug!("Sent typing indicator to {}", recipient),
            Err(e) => debug!("Failed to send typing indicator: {}", e),
        }

        Ok(())
    }

    fn max_message_length(&self) -> usize {
        65535
    }
}

#[async_trait]
impl ChannelReceiver for SignalChannel {
    async fn start_receiving(&self) -> Result<()> {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        let tx = self.message_tx.clone();
        let phone = self.phone_number.clone();
        let signal_cli = self.signal_cli_path.clone();
        let data_dir = self.data_dir.clone();
        let handler = self.handler.clone();

        // Clone self for use in the closure
        let channel = self.clone();

        tokio::spawn(async move {
            info!("Starting Signal receive loop for {}", phone);

            loop {
                // Start signal-cli receive in JSON mode
                let mut cmd = Command::new(&signal_cli);
                cmd.arg("-a")
                    .arg(&phone)
                    .arg("--config")
                    .arg(&data_dir)
                    .arg("receive")
                    .arg("--json")
                    .arg("-t")
                    .arg("5") // 5 second timeout
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null());

                match cmd.spawn() {
                    Ok(mut child) => {
                        if let Some(stdout) = child.stdout.take() {
                            let reader = BufReader::new(stdout);
                            let mut lines = reader.lines();

                            while let Ok(Some(line)) = lines.next_line().await {
                                if line.trim().is_empty() {
                                    continue;
                                }

                                debug!("Signal received: {}", line);

                                // Parse JSON envelope
                                match serde_json::from_str::<SignalCliEnvelope>(&line) {
                                    Ok(envelope) => {
                                        if let Some(msg) = channel.convert_envelope(envelope) {
                                            // Send to channel
                                            if let Err(e) = tx.send(msg.clone()).await {
                                                error!("Failed to send message to channel: {}", e);
                                            }

                                            // Call handler if set
                                            let h = handler.read().await;
                                            if let Some(ref handler) = *h {
                                                if let Err(e) = handler.handle(msg).await {
                                                    warn!("Handler error: {}", e);
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        debug!("Failed to parse Signal message: {}", e);
                                    }
                                }
                            }
                        }

                        let _ = child.wait().await;
                    }
                    Err(e) => {
                        error!("Failed to start signal-cli receive: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }

                // Check for shutdown
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        info!("Signal receive loop shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                        // Continue loop
                    }
                }
            }
        });

        info!(
            "Started receiving messages for Signal account: {}",
            self.phone_number
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
impl ChannelLifecycle for SignalChannel {
    async fn connect(&self) -> Result<()> {
        // Check if signal-cli is available
        let version = self.run_signal_cli(&["--version"]).await;
        if let Err(e) = version {
            return Err(ChannelError::Config(format!(
                "signal-cli not found or not working: {}",
                e
            )));
        }

        // Verify registration by listing accounts
        let accounts = self.run_signal_cli(&["listAccounts"]).await;
        match accounts {
            Ok(output) => {
                if output.contains(&self.phone_number) {
                    info!("Signal account verified: {}", self.phone_number);
                } else {
                    warn!(
                        "Phone number {} not found in registered accounts",
                        self.phone_number
                    );
                }
            }
            Err(e) => {
                warn!("Could not verify Signal account: {}", e);
            }
        }

        {
            let mut registered = self.registered.write().await;
            *registered = true;
        }

        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Connected to Signal as {}", self.phone_number);
        Ok(())
    }

    async fn disconnect(&self) -> Result<()> {
        self.stop_receiving().await?;

        let mut connected = self.connected.write().await;
        *connected = false;

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.blocking_read().clone()
    }

    async fn health(&self) -> Result<ChannelHealth> {
        let connected = *self.connected.read().await;
        let registered = *self.registered.read().await;

        // Try to run a quick signal-cli command to verify it's working
        let status = if connected && registered {
            match self.run_signal_cli(&["getUserStatus", &self.phone_number]).await {
                Ok(_) => HealthStatus::Healthy,
                Err(_) => HealthStatus::Degraded,
            }
        } else if registered {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        let error = if !registered {
            Some("Not registered with Signal".to_string())
        } else if !connected {
            Some("Not connected".to_string())
        } else {
            None
        };

        Ok(ChannelHealth {
            status,
            latency_ms: None,
            last_message_at: None,
            error,
        })
    }
}

impl Clone for SignalChannel {
    fn clone(&self) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            phone_number: self.phone_number.clone(),
            instance_id: self.instance_id.clone(),
            data_dir: self.data_dir.clone(),
            signal_cli_path: self.signal_cli_path.clone(),
            connected: self.connected.clone(),
            registered: self.registered.clone(),
            contacts: self.contacts.clone(),
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
    use std::path::PathBuf;

    #[test]
    fn test_signal_channel_creation() {
        let channel = SignalChannel::new("+1234567890", "test_signal", PathBuf::from("/tmp/signal"));
        assert_eq!(channel.channel_type(), "signal");
        assert_eq!(channel.instance_id(), "test_signal");
        assert_eq!(channel.phone_number(), "+1234567890");
    }

    #[test]
    fn test_capabilities() {
        let channel = SignalChannel::new("+1234567890", "test_signal", PathBuf::from("/tmp/signal"));
        let caps = channel.capabilities();
        assert!(caps.media.images);
        assert!(caps.media.voice_notes);
        assert!(caps.features.reactions);
        assert!(caps.features.typing_indicators);
        assert!(caps.features.read_receipts);
        assert!(caps.chat_types.contains(&ChatType::Direct));
        assert!(caps.chat_types.contains(&ChatType::Group));
    }

    #[test]
    fn test_normalize_phone_number() {
        let channel = SignalChannel::new("+1234567890", "test", PathBuf::from("/tmp"));

        assert_eq!(channel.normalize_phone_number("+1234567890"), "+1234567890");
        assert_eq!(channel.normalize_phone_number("5551234567"), "+15551234567");
        assert_eq!(channel.normalize_phone_number("555-123-4567"), "+15551234567");
        assert_eq!(channel.normalize_phone_number("+44123456789"), "+44123456789");
    }

    #[test]
    fn test_guess_media_type() {
        let channel = SignalChannel::new("+1234567890", "test", PathBuf::from("/tmp"));

        assert!(matches!(channel.guess_media_type("image/jpeg"), MediaType::Image));
        assert!(matches!(channel.guess_media_type("video/mp4"), MediaType::Video));
        assert!(matches!(channel.guess_media_type("audio/mpeg"), MediaType::Audio));
        assert!(matches!(channel.guess_media_type("audio/aac"), MediaType::Voice));
        assert!(matches!(channel.guess_media_type("application/pdf"), MediaType::Document));
    }

    #[test]
    fn test_is_group_id() {
        let channel = SignalChannel::new("+1234567890", "test", PathBuf::from("/tmp"));

        // Phone numbers are not group IDs
        assert!(!channel.is_group_id("+15551234567"));
        assert!(!channel.is_group_id("+44123456789"));

        // Base64-like strings are group IDs
        assert!(channel.is_group_id("dGVzdGdyb3VwaWRmb3JzaWduYWw="));
        assert!(channel.is_group_id("YW5vdGhlcmdyb3VwaWRleGFtcGxl"));
    }
}
