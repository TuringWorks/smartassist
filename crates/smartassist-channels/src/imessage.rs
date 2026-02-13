//! iMessage channel implementation.
//!
//! This channel integrates with Apple's iMessage on macOS.
//! It uses AppleScript for sending messages and reads incoming messages
//! from the Messages SQLite database at `~/Library/Messages/chat.db`.
//!
//! **Requirements:**
//! - macOS only
//! - Full Disk Access permission (System Settings > Privacy & Security > Full Disk Access)
//! - Messages app must be configured with an Apple ID or phone number

#![cfg(feature = "imessage")]

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
    MessageTarget, OutboundMessage, SenderInfo,
};
use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

/// iMessage channel implementation.
///
/// Uses AppleScript to send messages and monitors the Messages SQLite
/// database for incoming messages. Only works on macOS.
pub struct IMessageChannel {
    /// Channel instance ID.
    instance_id: String,

    /// Apple ID or phone number for the account.
    account_id: String,

    /// Path to Messages database.
    database_path: PathBuf,

    /// Connection state.
    connected: Arc<RwLock<bool>>,

    /// Last processed message ROWID.
    last_rowid: Arc<RwLock<i64>>,

    /// Contact cache.
    contacts: Arc<RwLock<HashMap<String, ContactInfo>>>,

    /// Incoming message channel.
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: Arc<RwLock<mpsc::Receiver<InboundMessage>>>,

    /// Message handler.
    handler: Arc<RwLock<Option<Box<dyn MessageHandler>>>>,

    /// Shutdown signal.
    shutdown: Arc<RwLock<Option<tokio::sync::oneshot::Sender<()>>>>,
}

/// Contact information from iMessage.
#[derive(Debug, Clone)]
pub struct ContactInfo {
    /// Handle ID (phone number or email).
    pub handle_id: String,

    /// Display name from Contacts.
    pub display_name: Option<String>,

    /// Is this an iMessage or SMS contact.
    pub is_imessage: bool,
}

impl std::fmt::Debug for IMessageChannel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IMessageChannel")
            .field("instance_id", &self.instance_id)
            .field("account_id", &self.account_id)
            .finish()
    }
}

impl IMessageChannel {
    /// Create a new iMessage channel.
    pub fn new(instance_id: impl Into<String>, account_id: impl Into<String>) -> Self {
        let (tx, rx) = mpsc::channel(1000);

        // Default Messages database path
        let database_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library")
            .join("Messages")
            .join("chat.db");

        Self {
            instance_id: instance_id.into(),
            account_id: account_id.into(),
            database_path,
            connected: Arc::new(RwLock::new(false)),
            last_rowid: Arc::new(RwLock::new(0)),
            contacts: Arc::new(RwLock::new(HashMap::new())),
            message_tx: tx,
            message_rx: Arc::new(RwLock::new(rx)),
            handler: Arc::new(RwLock::new(None)),
            shutdown: Arc::new(RwLock::new(None)),
        }
    }

    /// Create from configuration.
    pub fn from_config(config: ChannelConfig) -> std::result::Result<Self, ChannelError> {
        let account_id = config
            .options
            .get("account_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut channel = Self::new(config.instance_id, account_id);

        // Allow custom database path for testing
        if let Some(db_path) = config.options.get("database_path").and_then(|v| v.as_str()) {
            channel.database_path = PathBuf::from(db_path);
        }

        Ok(channel)
    }

    /// Check if running on macOS.
    fn is_macos() -> bool {
        cfg!(target_os = "macos")
    }

    /// Execute AppleScript to send a message.
    #[cfg(target_os = "macos")]
    async fn send_via_applescript(
        &self,
        recipient: &str,
        message: &str,
    ) -> std::result::Result<(), ChannelError> {
        let script = format!(
            r#"
            tell application "Messages"
                set targetBuddy to "{recipient}"
                set targetService to id of 1st service whose service type = iMessage
                set theBuddy to buddy targetBuddy of service id targetService
                send "{message}" to theBuddy
            end tell
            "#,
            recipient = recipient.replace('"', r#"\""#),
            message = message.replace('"', r#"\""#).replace('\n', "\\n"),
        );

        let output = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .await
            .map_err(|e| ChannelError::Internal(format!("Failed to run AppleScript: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ChannelError::channel(
                "imessage",
                format!("AppleScript error: {}", stderr),
            ))
        }
    }

    #[cfg(not(target_os = "macos"))]
    async fn send_via_applescript(
        &self,
        _recipient: &str,
        _message: &str,
    ) -> std::result::Result<(), ChannelError> {
        Err(ChannelError::Internal(
            "iMessage is only supported on macOS".to_string(),
        ))
    }

    /// Send an attachment via AppleScript.
    #[cfg(target_os = "macos")]
    async fn send_file_via_applescript(
        &self,
        recipient: &str,
        file_path: &std::path::Path,
    ) -> std::result::Result<(), ChannelError> {
        let script = format!(
            r#"
            tell application "Messages"
                set targetBuddy to "{recipient}"
                set targetService to id of 1st service whose service type = iMessage
                set theBuddy to buddy targetBuddy of service id targetService
                send POSIX file "{file_path}" to theBuddy
            end tell
            "#,
            recipient = recipient.replace('"', r#"\""#),
            file_path = file_path.display(),
        );

        let output = tokio::process::Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .await
            .map_err(|e| ChannelError::Internal(format!("Failed to run AppleScript: {}", e)))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(ChannelError::channel(
                "imessage",
                format!("AppleScript error: {}", stderr),
            ))
        }
    }

    #[cfg(not(target_os = "macos"))]
    async fn send_file_via_applescript(
        &self,
        _recipient: &str,
        _file_path: &std::path::Path,
    ) -> std::result::Result<(), ChannelError> {
        Err(ChannelError::Internal(
            "iMessage is only supported on macOS".to_string(),
        ))
    }

    /// Convert a database row to InboundMessage.
    #[allow(dead_code)]
    fn convert_message(
        &self,
        rowid: i64,
        text: &str,
        handle_id: &str,
        is_from_me: bool,
        date: i64,
        chat_id: &str,
        attachment_id: Option<&str>,
        mime_type: Option<&str>,
        filename: Option<&str>,
    ) -> InboundMessage {
        let sender = if is_from_me {
            SenderInfo {
                id: "me".to_string(),
                username: None,
                display_name: Some("Me".to_string()),
                phone_number: None,
                is_bot: false,
            }
        } else {
            SenderInfo {
                id: handle_id.to_string(),
                username: None,
                display_name: None,
                phone_number: if handle_id.starts_with('+') {
                    Some(handle_id.to_string())
                } else {
                    None
                },
                is_bot: false,
            }
        };

        let chat = ChatInfo {
            id: chat_id.to_string(),
            chat_type: if chat_id.contains(";-;") || chat_id.contains("chat") {
                ChatType::Group
            } else {
                ChatType::Direct
            },
            title: None,
            guild_id: None,
        };

        // Convert Apple's date format (nanoseconds since 2001-01-01) to DateTime<Utc>
        // Apple's epoch is 978307200 seconds after Unix epoch
        let apple_epoch_offset = 978307200i64;
        let unix_timestamp = (date / 1_000_000_000) + apple_epoch_offset;
        let timestamp = DateTime::<Utc>::from_timestamp(unix_timestamp, 0).unwrap_or_else(Utc::now);

        let media = match (attachment_id, mime_type, filename) {
            (Some(att_id), Some(mt), fname) => {
                vec![MediaAttachment {
                    id: att_id.to_string(),
                    media_type: self.guess_media_type(mt),
                    url: None,
                    data: None,
                    filename: fname.map(|s| s.to_string()),
                    size_bytes: None,
                    mime_type: Some(mt.to_string()),
                }]
            }
            _ => vec![],
        };

        InboundMessage {
            id: MessageId::new(rowid.to_string()),
            timestamp,
            channel: "imessage".to_string(),
            account_id: self.account_id.clone(),
            sender,
            chat,
            text: text.to_string(),
            media,
            quote: None,
            thread: None,
            metadata: serde_json::json!({
                "rowid": rowid,
                "is_from_me": is_from_me,
            }),
        }
    }

    /// Guess media type from MIME type.
    fn guess_media_type(&self, mime_type: &str) -> MediaType {
        if mime_type.starts_with("image/") {
            MediaType::Image
        } else if mime_type.starts_with("video/") {
            MediaType::Video
        } else if mime_type.starts_with("audio/") {
            MediaType::Audio
        } else {
            MediaType::Document
        }
    }

    /// Normalize a phone number or handle.
    fn normalize_handle(&self, handle: &str) -> String {
        // Remove spaces and dashes
        let cleaned: String = handle.chars().filter(|c| !c.is_whitespace() && *c != '-').collect();

        // If it looks like a phone number, normalize it
        if cleaned.chars().all(|c| c.is_ascii_digit() || c == '+') {
            let digits: String = cleaned.chars().filter(|c| c.is_ascii_digit()).collect();
            if digits.len() == 10 {
                format!("+1{}", digits)
            } else if digits.len() == 11 && digits.starts_with('1') {
                format!("+{}", digits)
            } else {
                format!("+{}", digits)
            }
        } else {
            // Assume it's an email/Apple ID
            cleaned
        }
    }
}

/// Row data from the iMessage database query.
#[derive(Debug)]
struct MessageRow {
    rowid: i64,
    text: Option<String>,
    handle_id: Option<String>,
    is_from_me: bool,
    date: i64,
    chat_identifier: Option<String>,
    attachment_filename: Option<String>,
    attachment_mime_type: Option<String>,
    attachment_path: Option<String>,
}

/// Query the iMessage database for new messages.
fn query_new_messages(
    db_path: &PathBuf,
    last_rowid: i64,
) -> std::result::Result<Vec<MessageRow>, rusqlite::Error> {
    // Open database in read-only mode
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;

    // Query for messages newer than last_rowid
    // Joins: message -> handle (sender), chat_message_join -> chat, message_attachment_join -> attachment
    let mut stmt = conn.prepare(
        r#"
        SELECT
            m.ROWID,
            m.text,
            h.id as handle_id,
            m.is_from_me,
            m.date,
            c.chat_identifier,
            a.filename as attachment_filename,
            a.mime_type as attachment_mime_type,
            a.filename as attachment_path
        FROM message m
        LEFT JOIN handle h ON m.handle_id = h.ROWID
        LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
        LEFT JOIN chat c ON cmj.chat_id = c.ROWID
        LEFT JOIN message_attachment_join maj ON m.ROWID = maj.message_id
        LEFT JOIN attachment a ON maj.attachment_id = a.ROWID
        WHERE m.ROWID > ?1
        ORDER BY m.ROWID ASC
        LIMIT 100
        "#,
    )?;

    let rows = stmt.query_map([last_rowid], |row| {
        Ok(MessageRow {
            rowid: row.get(0)?,
            text: row.get(1)?,
            handle_id: row.get(2)?,
            is_from_me: row.get::<_, i32>(3)? != 0,
            date: row.get(4)?,
            chat_identifier: row.get(5)?,
            attachment_filename: row.get(6)?,
            attachment_mime_type: row.get(7)?,
            attachment_path: row.get(8)?,
        })
    })?;

    let mut messages = Vec::new();
    for row in rows {
        match row {
            Ok(msg) => messages.push(msg),
            Err(e) => {
                // Log but continue processing other messages
                tracing::warn!("Failed to parse message row: {}", e);
            }
        }
    }

    Ok(messages)
}

/// Get the maximum ROWID from the database (for initialization).
fn get_max_rowid(db_path: &PathBuf) -> std::result::Result<i64, rusqlite::Error> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let max_rowid: i64 = conn.query_row(
        "SELECT COALESCE(MAX(ROWID), 0) FROM message",
        [],
        |row| row.get(0),
    )?;
    Ok(max_rowid)
}

/// Convert a MessageRow to InboundMessage.
fn convert_row_to_inbound(
    row: &MessageRow,
    account_id: &str,
) -> InboundMessage {
    let sender = if row.is_from_me {
        SenderInfo {
            id: "me".to_string(),
            username: None,
            display_name: Some("Me".to_string()),
            phone_number: None,
            is_bot: false,
        }
    } else {
        let handle = row.handle_id.clone().unwrap_or_default();
        SenderInfo {
            id: handle.clone(),
            username: None,
            display_name: None,
            phone_number: if handle.starts_with('+') {
                Some(handle)
            } else {
                None
            },
            is_bot: false,
        }
    };

    let chat_id = row.chat_identifier.clone().unwrap_or_else(|| {
        row.handle_id.clone().unwrap_or_else(|| format!("unknown_{}", row.rowid))
    });

    let chat = ChatInfo {
        id: chat_id.clone(),
        chat_type: if chat_id.contains(";-;") || chat_id.contains("chat") {
            ChatType::Group
        } else {
            ChatType::Direct
        },
        title: None,
        guild_id: None,
    };

    // Convert Apple's date format (nanoseconds since 2001-01-01) to DateTime<Utc>
    // Apple's epoch is 978307200 seconds after Unix epoch
    let apple_epoch_offset = 978307200i64;
    let unix_timestamp = (row.date / 1_000_000_000) + apple_epoch_offset;
    let timestamp = DateTime::<Utc>::from_timestamp(unix_timestamp, 0).unwrap_or_else(Utc::now);

    // Build media attachments if present
    let media = match (&row.attachment_filename, &row.attachment_mime_type) {
        (Some(filename), Some(mime_type)) => {
            let media_type = if mime_type.starts_with("image/") {
                MediaType::Image
            } else if mime_type.starts_with("video/") {
                MediaType::Video
            } else if mime_type.starts_with("audio/") {
                MediaType::Audio
            } else {
                MediaType::Document
            };

            // Attachment path is relative to ~/Library/Messages/Attachments/
            let attachment_url = row.attachment_path.as_ref().map(|p| {
                if p.starts_with("~") {
                    // Expand tilde
                    dirs::home_dir()
                        .map(|h| h.join(&p[2..]).to_string_lossy().to_string())
                        .unwrap_or_else(|| p.clone())
                } else if p.starts_with("/") {
                    p.clone()
                } else {
                    // Relative path - prefix with Messages/Attachments
                    dirs::home_dir()
                        .map(|h| h.join("Library/Messages/Attachments").join(p).to_string_lossy().to_string())
                        .unwrap_or_else(|| p.clone())
                }
            });

            vec![MediaAttachment {
                id: format!("att_{}", row.rowid),
                media_type,
                url: attachment_url,
                data: None,
                filename: Some(filename.clone()),
                size_bytes: None,
                mime_type: Some(mime_type.clone()),
            }]
        }
        _ => vec![],
    };

    InboundMessage {
        id: MessageId::new(row.rowid.to_string()),
        timestamp,
        channel: "imessage".to_string(),
        account_id: account_id.to_string(),
        sender,
        chat,
        text: row.text.clone().unwrap_or_default(),
        media,
        quote: None,
        thread: None,
        metadata: serde_json::json!({
            "rowid": row.rowid,
            "is_from_me": row.is_from_me,
        }),
    }
}

#[async_trait]
impl Channel for IMessageChannel {
    fn channel_type(&self) -> &str {
        "imessage"
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
                stickers: false, // Stickers are complex in iMessage
                voice_notes: true,
                max_file_size_mb: 100,
            },
            features: ChannelFeatures {
                reactions: true,  // Tapback reactions
                threads: false,
                edits: true,      // iOS 16+
                deletes: true,    // iOS 16+
                typing_indicators: true,
                read_receipts: true,
                mentions: true,   // @mentions in groups
                polls: false,
                native_commands: false,
            },
            limits: ChannelLimits {
                text_max_length: 20000, // Practical limit, not enforced
                caption_max_length: 1000,
                messages_per_second: 5.0,
                messages_per_minute: 300,
            },
        }
    }
}

#[async_trait]
impl ChannelSender for IMessageChannel {
    async fn send(&self, message: OutboundMessage) -> Result<SendResult> {
        if !Self::is_macos() {
            return Err(ChannelError::Internal(
                "iMessage is only supported on macOS".to_string(),
            ));
        }

        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to iMessage".to_string()));
        }

        let recipient = self.normalize_handle(&message.target.chat_id);
        debug!("Sending iMessage to {}", recipient);

        self.send_via_applescript(&recipient, &message.text).await?;

        let msg_id = chrono::Utc::now().timestamp_millis().to_string();
        Ok(SendResult::new(msg_id))
    }

    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult> {
        if !Self::is_macos() {
            return Err(ChannelError::Internal(
                "iMessage is only supported on macOS".to_string(),
            ));
        }

        let connected = *self.connected.read().await;
        if !connected {
            return Err(ChannelError::Internal("Not connected to iMessage".to_string()));
        }

        let recipient = self.normalize_handle(&message.target.chat_id);

        // Send attachments first
        for attachment in &attachments {
            match &attachment.source {
                crate::attachment::AttachmentSource::Path(path) => {
                    self.send_file_via_applescript(&recipient, path).await?;
                }
                crate::attachment::AttachmentSource::Bytes(bytes) => {
                    // Write to temp file and send
                    let temp_path = std::env::temp_dir().join(&attachment.filename);
                    tokio::fs::write(&temp_path, bytes)
                        .await
                        .map_err(|e| ChannelError::Internal(e.to_string()))?;
                    self.send_file_via_applescript(&recipient, &temp_path).await?;
                    let _ = tokio::fs::remove_file(&temp_path).await;
                }
                _ => {
                    warn!("Unsupported attachment source for iMessage");
                }
            }
        }

        // Send text message if present
        if !message.text.is_empty() {
            return self.send(message).await;
        }

        let msg_id = chrono::Utc::now().timestamp_millis().to_string();
        Ok(SendResult::new(msg_id))
    }

    async fn edit(&self, _message: &MessageRef, _new_content: &str) -> Result<()> {
        // iMessage supports editing (iOS 16+) but not via AppleScript
        warn!("iMessage edit not supported via AppleScript");
        Ok(())
    }

    async fn delete(&self, _message: &MessageRef) -> Result<()> {
        // iMessage supports unsend (iOS 16+) but not via AppleScript
        warn!("iMessage delete/unsend not supported via AppleScript");
        Ok(())
    }

    async fn react(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        // Tapback reactions exist but aren't accessible via AppleScript
        warn!("iMessage reactions (Tapback) not supported via AppleScript");
        Ok(())
    }

    async fn unreact(&self, _message: &MessageRef, _emoji: &str) -> Result<()> {
        warn!("iMessage unreact not supported via AppleScript");
        Ok(())
    }

    async fn send_typing(&self, _target: &MessageTarget) -> Result<()> {
        // iMessage typing indicators are automatic
        // Not controllable via AppleScript
        Ok(())
    }

    fn max_message_length(&self) -> usize {
        20000
    }
}

#[async_trait]
impl ChannelReceiver for IMessageChannel {
    async fn start_receiving(&self) -> Result<()> {
        if !Self::is_macos() {
            return Err(ChannelError::Internal(
                "iMessage is only supported on macOS".to_string(),
            ));
        }

        // Check database accessibility
        if !self.database_path.exists() {
            return Err(ChannelError::Config(format!(
                "Messages database not found at {:?}. Make sure Messages app has been used.",
                self.database_path
            )));
        }

        // Initialize last_rowid to current max to only get new messages
        let initial_rowid = match get_max_rowid(&self.database_path) {
            Ok(rowid) => {
                info!("iMessage database initialized at ROWID {}", rowid);
                rowid
            }
            Err(e) => {
                // Common error: permission denied - need Full Disk Access
                if e.to_string().contains("unable to open") || e.to_string().contains("permission") {
                    return Err(ChannelError::Config(
                        "Cannot access Messages database. Please grant Full Disk Access permission \
                         in System Settings > Privacy & Security > Full Disk Access".to_string()
                    ));
                }
                return Err(ChannelError::channel("imessage", format!("Database error: {}", e)));
            }
        };

        {
            let mut last_rowid = self.last_rowid.write().await;
            *last_rowid = initial_rowid;
        }

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        {
            let mut shutdown = self.shutdown.write().await;
            *shutdown = Some(shutdown_tx);
        }

        let tx = self.message_tx.clone();
        let db_path = self.database_path.clone();
        let connected = self.connected.clone();
        let last_rowid = self.last_rowid.clone();
        let handler = self.handler.clone();
        let account_id = self.account_id.clone();

        tokio::spawn(async move {
            info!("Starting iMessage receive loop - polling {:?}", db_path);

            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        info!("iMessage receive loop shutting down");
                        break;
                    }
                    _ = tokio::time::sleep(tokio::time::Duration::from_millis(500)) => {
                        let is_connected = *connected.read().await;
                        if !is_connected {
                            debug!("iMessage not connected, skipping poll");
                            continue;
                        }

                        // Get current last_rowid
                        let current_rowid = *last_rowid.read().await;

                        // Query for new messages (run in blocking task since rusqlite is sync)
                        let db_path_clone = db_path.clone();
                        let query_result = tokio::task::spawn_blocking(move || {
                            query_new_messages(&db_path_clone, current_rowid)
                        }).await;

                        match query_result {
                            Ok(Ok(messages)) => {
                                if !messages.is_empty() {
                                    debug!("Found {} new iMessage(s)", messages.len());

                                    let mut max_rowid = current_rowid;

                                    for row in messages {
                                        // Skip messages from self unless configured otherwise
                                        if row.is_from_me {
                                            max_rowid = max_rowid.max(row.rowid);
                                            continue;
                                        }

                                        // Convert to InboundMessage
                                        let inbound = convert_row_to_inbound(&row, &account_id);

                                        debug!(
                                            "Received iMessage from {}: {}",
                                            inbound.sender.id,
                                            inbound.text.chars().take(50).collect::<String>()
                                        );

                                        // Call handler if set
                                        {
                                            let handler_guard = handler.read().await;
                                            if let Some(ref h) = *handler_guard {
                                                if let Err(e) = h.handle(inbound.clone()).await {
                                                    warn!("Message handler error: {}", e);
                                                }
                                            }
                                        }

                                        // Send to channel
                                        if let Err(e) = tx.send(inbound).await {
                                            warn!("Failed to send message to channel: {}", e);
                                        }

                                        max_rowid = max_rowid.max(row.rowid);
                                    }

                                    // Update last_rowid
                                    if max_rowid > current_rowid {
                                        let mut rowid = last_rowid.write().await;
                                        *rowid = max_rowid;
                                    }
                                }
                            }
                            Ok(Err(e)) => {
                                // Database query error - log but continue
                                warn!("iMessage database query error: {}", e);
                            }
                            Err(e) => {
                                // Task join error
                                error!("iMessage query task failed: {}", e);
                            }
                        }
                    }
                }
            }
        });

        info!(
            "Started receiving messages for iMessage (instance: {})",
            self.instance_id
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
impl ChannelLifecycle for IMessageChannel {
    async fn connect(&self) -> Result<()> {
        if !Self::is_macos() {
            return Err(ChannelError::Internal(
                "iMessage is only supported on macOS".to_string(),
            ));
        }

        // Check if Messages database exists
        if !self.database_path.exists() {
            return Err(ChannelError::Config(format!(
                "Messages database not found at {:?}. \
                 Make sure Messages app has been used at least once.",
                self.database_path
            )));
        }

        // Verify we can actually read the database (requires Full Disk Access)
        let db_path = self.database_path.clone();
        let access_check = tokio::task::spawn_blocking(move || {
            get_max_rowid(&db_path)
        }).await;

        match access_check {
            Ok(Ok(rowid)) => {
                info!("iMessage database accessible, current ROWID: {}", rowid);
            }
            Ok(Err(e)) => {
                let err_str = e.to_string();
                if err_str.contains("unable to open") || err_str.contains("permission") || err_str.contains("readonly") {
                    return Err(ChannelError::Config(
                        "Cannot access Messages database. Please grant Full Disk Access:\n\
                         1. Open System Settings\n\
                         2. Go to Privacy & Security > Full Disk Access\n\
                         3. Add your terminal app or the application running this code".to_string()
                    ));
                }
                return Err(ChannelError::channel("imessage", format!("Database error: {}", e)));
            }
            Err(e) => {
                return Err(ChannelError::Internal(format!("Task error: {}", e)));
            }
        }

        // Check if Messages app is available
        #[cfg(target_os = "macos")]
        {
            let output = tokio::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to (name of processes) contains \"Messages\"")
                .output()
                .await;

            match output {
                Ok(o) if o.status.success() => {
                    debug!("Messages app check completed");
                }
                Ok(o) => {
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    warn!("iMessage check warning: {}", stderr);
                }
                Err(e) => {
                    warn!("Failed to check Messages app: {}", e);
                }
            }
        }

        let mut connected = self.connected.write().await;
        *connected = true;

        info!("Connected to iMessage (database: {:?})", self.database_path);
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
        if !Self::is_macos() {
            return Ok(ChannelHealth {
                status: HealthStatus::Unhealthy,
                latency_ms: None,
                last_message_at: None,
                error: Some("iMessage is only supported on macOS".to_string()),
            });
        }

        let connected = *self.connected.read().await;

        Ok(ChannelHealth {
            status: if connected {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            },
            latency_ms: Some(0), // Local
            last_message_at: None,
            error: if connected {
                None
            } else {
                Some("Not connected".to_string())
            },
        })
    }
}

impl Clone for IMessageChannel {
    fn clone(&self) -> Self {
        let (tx, rx) = mpsc::channel(1000);
        Self {
            instance_id: self.instance_id.clone(),
            account_id: self.account_id.clone(),
            database_path: self.database_path.clone(),
            connected: self.connected.clone(),
            last_rowid: self.last_rowid.clone(),
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

    #[test]
    fn test_imessage_channel_creation() {
        let channel = IMessageChannel::new("test_imessage", "test@icloud.com");
        assert_eq!(channel.channel_type(), "imessage");
        assert_eq!(channel.instance_id(), "test_imessage");
    }

    #[test]
    fn test_capabilities() {
        let channel = IMessageChannel::new("test_imessage", "");
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
    fn test_normalize_handle() {
        let channel = IMessageChannel::new("test", "");

        // Phone numbers
        assert_eq!(channel.normalize_handle("+1 555 123 4567"), "+15551234567");
        assert_eq!(channel.normalize_handle("555-123-4567"), "+15551234567");
        assert_eq!(channel.normalize_handle("15551234567"), "+15551234567");

        // Email/Apple ID
        assert_eq!(channel.normalize_handle("user@icloud.com"), "user@icloud.com");
    }

    #[test]
    fn test_guess_media_type() {
        let channel = IMessageChannel::new("test", "");

        assert!(matches!(
            channel.guess_media_type("image/heic"),
            MediaType::Image
        ));
        assert!(matches!(
            channel.guess_media_type("video/quicktime"),
            MediaType::Video
        ));
        assert!(matches!(
            channel.guess_media_type("audio/m4a"),
            MediaType::Audio
        ));
        assert!(matches!(
            channel.guess_media_type("application/pdf"),
            MediaType::Document
        ));
    }

    #[test]
    fn test_is_macos() {
        // This will return true on macOS, false on other platforms
        let result = IMessageChannel::is_macos();
        #[cfg(target_os = "macos")]
        assert!(result);
        #[cfg(not(target_os = "macos"))]
        assert!(!result);
    }

    #[test]
    fn test_convert_row_to_inbound() {
        // Test incoming message conversion
        let row = MessageRow {
            rowid: 12345,
            text: Some("Hello from iMessage!".to_string()),
            handle_id: Some("+15551234567".to_string()),
            is_from_me: false,
            date: 700000000000000000, // ~2023 in Apple's date format
            chat_identifier: Some("iMessage;-;+15551234567".to_string()),
            attachment_filename: None,
            attachment_mime_type: None,
            attachment_path: None,
        };

        let inbound = convert_row_to_inbound(&row, "test_account");

        assert_eq!(inbound.id.as_str(), "12345");
        assert_eq!(inbound.text, "Hello from iMessage!");
        assert_eq!(inbound.sender.id, "+15551234567");
        assert_eq!(inbound.sender.phone_number, Some("+15551234567".to_string()));
        assert!(!inbound.sender.is_bot);
        assert_eq!(inbound.chat.id, "iMessage;-;+15551234567");
        assert_eq!(inbound.channel, "imessage");
        assert!(inbound.media.is_empty());

        // Test message from self
        let from_me_row = MessageRow {
            rowid: 12346,
            text: Some("My reply".to_string()),
            handle_id: Some("+15551234567".to_string()),
            is_from_me: true,
            date: 700000000000000000,
            chat_identifier: Some("iMessage;-;+15551234567".to_string()),
            attachment_filename: None,
            attachment_mime_type: None,
            attachment_path: None,
        };

        let from_me_inbound = convert_row_to_inbound(&from_me_row, "test_account");
        assert_eq!(from_me_inbound.sender.id, "me");
        assert_eq!(from_me_inbound.sender.display_name, Some("Me".to_string()));

        // Test message with attachment
        let with_attachment = MessageRow {
            rowid: 12347,
            text: None,
            handle_id: Some("+15559876543".to_string()),
            is_from_me: false,
            date: 700000000000000000,
            chat_identifier: Some("+15559876543".to_string()),
            attachment_filename: Some("photo.heic".to_string()),
            attachment_mime_type: Some("image/heic".to_string()),
            attachment_path: Some("~/Library/Messages/Attachments/ab/12/photo.heic".to_string()),
        };

        let attachment_inbound = convert_row_to_inbound(&with_attachment, "test_account");
        assert_eq!(attachment_inbound.media.len(), 1);
        assert_eq!(attachment_inbound.media[0].filename, Some("photo.heic".to_string()));
        assert_eq!(attachment_inbound.media[0].mime_type, Some("image/heic".to_string()));
        assert!(matches!(attachment_inbound.media[0].media_type, MediaType::Image));
    }
}
