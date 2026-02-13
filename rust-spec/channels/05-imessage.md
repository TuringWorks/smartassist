# iMessage Channel Specification

## Overview

iMessage channel implementation for macOS using AppleScript/JXA bridge.

## Dependencies

```toml
[dependencies]
tokio = { version = "1", features = ["full", "process"] }
serde_json = "1.0"
```

## Authentication

```rust
pub struct IMessageConfig {
    /// Apple ID email (for identification)
    pub apple_id: Option<String>,

    /// BlueBubbles server URL (optional alternative backend)
    pub bluebubbles_url: Option<String>,

    /// BlueBubbles password
    pub bluebubbles_password: Option<SecretString>,

    /// Messages database path (for reading history)
    pub messages_db_path: PathBuf,

    /// Polling interval in milliseconds
    pub poll_interval_ms: u64,
}

impl Default for IMessageConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        Self {
            apple_id: None,
            bluebubbles_url: None,
            bluebubbles_password: None,
            messages_db_path: home.join("Library/Messages/chat.db"),
            poll_interval_ms: 1000,
        }
    }
}
```

## Channel Implementation

```rust
pub struct IMessageChannel {
    config: IMessageConfig,
    message_tx: mpsc::Sender<InboundMessage>,
    message_rx: mpsc::Receiver<InboundMessage>,
    connected: AtomicBool,
    last_message_id: AtomicI64,
    poll_handle: Option<JoinHandle<()>>,
}

#[async_trait]
impl Channel for IMessageChannel {
    fn id(&self) -> &str { "imessage" }
    fn name(&self) -> &str { "iMessage" }

    fn capabilities(&self) -> ChannelCapabilities {
        ChannelCapabilities {
            chat_types: vec![ChatType::Direct, ChatType::Group],
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
                edits: false,
                deletes: false,
                typing_indicators: true,
                read_receipts: true,
                mentions: false,
                polls: false,
                buttons: false,
                inline_queries: false,
                commands: false,
                markdown: false,
                html: false,
            },
            limits: ChannelLimits {
                max_message_length: 20000,
                max_caption_length: 20000,
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

## AppleScript Bridge

```rust
impl IMessageChannel {
    async fn run_applescript(&self, script: &str) -> Result<String> {
        let output = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(ChannelError::ActionFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ))
        }
    }

    async fn run_jxa(&self, script: &str) -> Result<serde_json::Value> {
        let output = Command::new("osascript")
            .arg("-l")
            .arg("JavaScript")
            .arg("-e")
            .arg(script)
            .output()
            .await?;

        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            serde_json::from_str(&stdout)
                .map_err(|e| ChannelError::ActionFailed(e.to_string()))
        } else {
            Err(ChannelError::ActionFailed(
                String::from_utf8_lossy(&output.stderr).to_string()
            ))
        }
    }
}
```

## Message Handling

### Database Polling

```rust
impl IMessageChannel {
    async fn poll_messages(&self) -> Result<Vec<InboundMessage>> {
        let db = rusqlite::Connection::open(&self.config.messages_db_path)?;

        let last_id = self.last_message_id.load(Ordering::SeqCst);

        let mut stmt = db.prepare(r#"
            SELECT
                m.ROWID,
                m.guid,
                m.text,
                m.date,
                m.is_from_me,
                m.handle_id,
                h.id as sender_id,
                c.chat_identifier,
                c.display_name,
                c.group_id
            FROM message m
            LEFT JOIN handle h ON m.handle_id = h.ROWID
            LEFT JOIN chat_message_join cmj ON m.ROWID = cmj.message_id
            LEFT JOIN chat c ON cmj.chat_id = c.ROWID
            WHERE m.ROWID > ?
            ORDER BY m.ROWID ASC
            LIMIT 100
        "#)?;

        let messages: Vec<InboundMessage> = stmt.query_map([last_id], |row| {
            Ok(self.row_to_message(row))
        })?.filter_map(|r| r.ok()).collect();

        if let Some(last) = messages.last() {
            if let Ok(id) = last.id.as_str().parse::<i64>() {
                self.last_message_id.store(id, Ordering::SeqCst);
            }
        }

        Ok(messages)
    }

    fn row_to_message(&self, row: &rusqlite::Row) -> InboundMessage {
        let rowid: i64 = row.get(0).unwrap_or(0);
        let guid: String = row.get(1).unwrap_or_default();
        let text: Option<String> = row.get(2).ok();
        let date: i64 = row.get(3).unwrap_or(0);
        let is_from_me: bool = row.get::<_, i32>(4).unwrap_or(0) == 1;
        let sender_id: Option<String> = row.get(6).ok();
        let chat_identifier: Option<String> = row.get(7).ok();
        let display_name: Option<String> = row.get(8).ok();
        let group_id: Option<String> = row.get(9).ok();

        // Convert Apple's CoreData timestamp (nanoseconds since 2001-01-01)
        let timestamp = self.apple_timestamp_to_datetime(date);

        InboundMessage {
            id: MessageId::new(rowid.to_string()),
            timestamp,
            channel: "imessage".to_string(),
            account_id: self.config.apple_id.clone().unwrap_or_default(),
            sender: SenderInfo {
                id: if is_from_me {
                    self.config.apple_id.clone().unwrap_or_else(|| "me".to_string())
                } else {
                    sender_id.clone().unwrap_or_default()
                },
                username: None,
                display_name: None,
                phone_number: sender_id,
                is_bot: false,
            },
            chat: ChatInfo {
                id: chat_identifier.clone().unwrap_or_default(),
                chat_type: if group_id.is_some() {
                    ChatType::Group
                } else {
                    ChatType::Direct
                },
                title: display_name,
                guild_id: None,
            },
            text: text.unwrap_or_default(),
            media: vec![], // Attachments fetched separately
            quote: None,
            thread: None,
            metadata: serde_json::json!({
                "guid": guid,
                "is_from_me": is_from_me,
            }),
        }
    }

    fn apple_timestamp_to_datetime(&self, timestamp: i64) -> DateTime<Utc> {
        // Apple timestamps are nanoseconds since 2001-01-01
        let apple_epoch = DateTime::parse_from_rfc3339("2001-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let duration = chrono::Duration::nanoseconds(timestamp);
        apple_epoch + duration
    }
}
```

### Outbound Message Sending

```rust
impl IMessageChannel {
    async fn send_message(&self, recipient: &str, message: &OutboundMessage) -> Result<SendResult> {
        let escaped_text = message.text.replace("\"", "\\\"").replace("\n", "\\n");
        let escaped_recipient = recipient.replace("\"", "\\\"");

        let script = format!(r#"
            tell application "Messages"
                set targetService to 1st service whose service type = iMessage
                set targetBuddy to buddy "{}" of targetService
                send "{}" to targetBuddy
            end tell
        "#, escaped_recipient, escaped_text);

        self.run_applescript(&script).await?;

        Ok(SendResult {
            message_id: Utc::now().timestamp_millis().to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn send_message_with_effect(
        &self,
        recipient: &str,
        text: &str,
        effect_id: &str,
    ) -> Result<SendResult> {
        // Effects require JXA
        let script = format!(r#"
            (() => {{
                const Messages = Application('Messages');
                const service = Messages.services.byName('iMessage');
                const buddy = service.buddies.byId('{}');

                Messages.send('{}', {{
                    to: buddy,
                    withMessageEffect: '{}'
                }});

                return JSON.stringify({{ ok: true }});
            }})()
        "#, recipient, text.replace("'", "\\'"), effect_id);

        self.run_jxa(&script).await?;

        Ok(SendResult {
            message_id: Utc::now().timestamp_millis().to_string(),
            timestamp: Utc::now(),
        })
    }

    async fn send_attachment(&self, recipient: &str, file_path: &Path) -> Result<()> {
        let script = format!(r#"
            tell application "Messages"
                set targetService to 1st service whose service type = iMessage
                set targetBuddy to buddy "{}" of targetService
                send POSIX file "{}" to targetBuddy
            end tell
        "#, recipient, file_path.display());

        self.run_applescript(&script).await?;
        Ok(())
    }
}
```

## iMessage Effects

```rust
/// Available iMessage effects
#[derive(Debug, Clone, Copy)]
pub enum IMessageEffect {
    Slam,
    Loud,
    Gentle,
    InvisibleInk,
    Echo,
    Spotlight,
    Balloons,
    Confetti,
    Love,
    Lasers,
    Fireworks,
    Celebration,
}

impl IMessageEffect {
    pub fn effect_id(&self) -> &'static str {
        match self {
            Self::Slam => "com.apple.MobileSMS.expressivesend.impact",
            Self::Loud => "com.apple.MobileSMS.expressivesend.loud",
            Self::Gentle => "com.apple.MobileSMS.expressivesend.gentle",
            Self::InvisibleInk => "com.apple.MobileSMS.expressivesend.invisibleink",
            Self::Echo => "com.apple.messages.effect.CKEchoEffect",
            Self::Spotlight => "com.apple.messages.effect.CKSpotlightEffect",
            Self::Balloons => "com.apple.messages.effect.CKHappyBirthdayEffect",
            Self::Confetti => "com.apple.messages.effect.CKConfettiEffect",
            Self::Love => "com.apple.messages.effect.CKHeartEffect",
            Self::Lasers => "com.apple.messages.effect.CKLasersEffect",
            Self::Fireworks => "com.apple.messages.effect.CKFireworksEffect",
            Self::Celebration => "com.apple.messages.effect.CKSparklesEffect",
        }
    }
}
```

## iMessage-Specific Actions

```rust
impl IMessageChannel {
    /// Send a tapback reaction
    pub async fn send_tapback(
        &self,
        chat_id: &str,
        message_guid: &str,
        tapback: Tapback,
    ) -> Result<()> {
        // Tapbacks require direct SQLite manipulation or BlueBubbles
        if let Some(ref bb_url) = self.config.bluebubbles_url {
            self.send_tapback_bluebubbles(bb_url, chat_id, message_guid, tapback).await
        } else {
            Err(ChannelError::NotSupported("Tapbacks require BlueBubbles".into()))
        }
    }

    /// Start typing indicator
    pub async fn start_typing(&self, chat_id: &str) -> Result<()> {
        // Typing indicators are automatic in Messages.app
        // Can be simulated via BlueBubbles
        Ok(())
    }

    /// Get chat participants
    pub async fn get_chat_participants(&self, chat_id: &str) -> Result<Vec<String>> {
        let db = rusqlite::Connection::open(&self.config.messages_db_path)?;

        let mut stmt = db.prepare(r#"
            SELECT h.id
            FROM handle h
            JOIN chat_handle_join chj ON h.ROWID = chj.handle_id
            JOIN chat c ON chj.chat_id = c.ROWID
            WHERE c.chat_identifier = ?
        "#)?;

        let participants: Vec<String> = stmt.query_map([chat_id], |row| {
            row.get(0)
        })?.filter_map(|r| r.ok()).collect();

        Ok(participants)
    }

    /// Get recent chats
    pub async fn get_recent_chats(&self, limit: usize) -> Result<Vec<ChatInfo>> {
        let db = rusqlite::Connection::open(&self.config.messages_db_path)?;

        let mut stmt = db.prepare(r#"
            SELECT DISTINCT
                c.chat_identifier,
                c.display_name,
                c.group_id,
                MAX(m.date) as last_message
            FROM chat c
            JOIN chat_message_join cmj ON c.ROWID = cmj.chat_id
            JOIN message m ON cmj.message_id = m.ROWID
            GROUP BY c.ROWID
            ORDER BY last_message DESC
            LIMIT ?
        "#)?;

        let chats: Vec<ChatInfo> = stmt.query_map([limit], |row| {
            let chat_identifier: String = row.get(0)?;
            let display_name: Option<String> = row.get(1).ok();
            let group_id: Option<String> = row.get(2).ok();

            Ok(ChatInfo {
                id: chat_identifier,
                chat_type: if group_id.is_some() {
                    ChatType::Group
                } else {
                    ChatType::Direct
                },
                title: display_name,
                guild_id: None,
            })
        })?.filter_map(|r| r.ok()).collect();

        Ok(chats)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Tapback {
    Love,
    Like,
    Dislike,
    Laugh,
    Emphasis,
    Question,
}
```

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum IMessageError {
    #[error("Messages.app not available")]
    MessagesNotAvailable,

    #[error("Database not accessible: {0}")]
    DatabaseError(String),

    #[error("AppleScript error: {0}")]
    AppleScriptError(String),

    #[error("Recipient not found: {0}")]
    RecipientNotFound(String),

    #[error("iMessage not enabled for recipient")]
    NotIMessage,

    #[error("Effect not supported: {0}")]
    EffectNotSupported(String),

    #[error("BlueBubbles error: {0}")]
    BlueBubblesError(String),
}
```
