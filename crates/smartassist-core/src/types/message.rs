//! Message types for inbound and outbound messages.

use super::{ChatType, MessageId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

/// An inbound message from a messaging channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Message ID (channel-specific).
    pub id: MessageId,

    /// Timestamp when the message was received.
    pub timestamp: DateTime<Utc>,

    /// Source channel (telegram, discord, etc.).
    pub channel: String,

    /// Account ID (bot account).
    pub account_id: String,

    /// Sender information.
    pub sender: SenderInfo,

    /// Chat/conversation information.
    pub chat: ChatInfo,

    /// Text content.
    pub text: String,

    /// Media attachments.
    #[serde(default)]
    pub media: Vec<MediaAttachment>,

    /// Quoted/replied message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quote: Option<QuotedMessage>,

    /// Thread information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread: Option<ThreadInfo>,

    /// Channel-specific metadata.
    #[serde(default)]
    pub metadata: Value,
}

impl Default for InboundMessage {
    fn default() -> Self {
        Self {
            id: MessageId::new(""),
            timestamp: Utc::now(),
            channel: String::new(),
            account_id: String::new(),
            sender: SenderInfo::default(),
            chat: ChatInfo::default(),
            text: String::new(),
            media: Vec::new(),
            quote: None,
            thread: None,
            metadata: Value::Null,
        }
    }
}

/// Information about the message sender.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SenderInfo {
    /// Unique sender ID.
    pub id: String,

    /// Username (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,

    /// Phone number (for SMS/WhatsApp/Signal).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,

    /// Whether the sender is a bot.
    #[serde(default)]
    pub is_bot: bool,
}

/// Information about the chat/conversation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatInfo {
    /// Chat ID.
    pub id: String,

    /// Type of chat.
    #[serde(default)]
    pub chat_type: ChatType,

    /// Chat title (for groups/channels).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Guild/team ID (Discord server, Slack workspace).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild_id: Option<String>,
}

/// A media attachment on a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaAttachment {
    /// Attachment ID.
    pub id: String,

    /// Type of media.
    pub media_type: MediaType,

    /// URL to download the media.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Raw media data (for inline attachments).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<u8>>,

    /// Original filename.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,

    /// File size in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,

    /// MIME type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Type of media attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaType {
    Image,
    Audio,
    Video,
    Voice,
    Document,
    Sticker,
}

impl Default for MediaType {
    fn default() -> Self {
        Self::Document
    }
}

/// A quoted/replied-to message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotedMessage {
    /// ID of the quoted message.
    pub id: String,

    /// Text of the quoted message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,

    /// Sender ID of the quoted message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender_id: Option<String>,
}

/// Thread information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadInfo {
    /// Thread ID.
    pub id: String,

    /// Parent message/thread ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
}

/// An outbound message to be sent to a channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Target for the message (chat/thread).
    pub target: super::MessageTarget,

    /// Text content.
    pub text: String,

    /// Media attachments.
    #[serde(default)]
    pub media: Vec<MediaPayload>,

    /// Mentions in the message.
    #[serde(default)]
    pub mentions: Vec<Mention>,

    /// Reply to message ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,

    /// Send options.
    #[serde(default)]
    pub options: SendOptions,
}

/// A media payload for outbound messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaPayload {
    /// Type of media.
    pub media_type: MediaType,

    /// Source of the media.
    pub source: MediaSource,

    /// Filename to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,

    /// Caption for the media.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

/// Source of media content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum MediaSource {
    /// URL to fetch media from.
    Url(String),

    /// Local file path.
    Path(PathBuf),

    /// Raw bytes (base64 encoded in JSON).
    Bytes(Vec<u8>),
}

/// A mention in a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Mention {
    /// User ID being mentioned.
    pub user_id: String,

    /// Username (for display).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Offset in the text where mention starts.
    pub offset: usize,

    /// Length of the mention text.
    pub length: usize,
}

/// Options for sending messages.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SendOptions {
    /// Disable link previews.
    #[serde(default)]
    pub disable_preview: bool,

    /// Send silently (no notification).
    #[serde(default)]
    pub silent: bool,

    /// Parse mode for formatting.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_mode: Option<ParseMode>,

    /// Keyboard/buttons to attach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<Value>,
}

/// Parse mode for message formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseMode {
    Markdown,
    Html,
    Plain,
}

/// Result of delivering a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryResult {
    /// Channel the message was delivered to.
    pub channel: String,

    /// Message ID assigned by the channel.
    pub message_id: String,

    /// Chat ID.
    pub chat_id: String,

    /// Delivery timestamp.
    pub timestamp: DateTime<Utc>,

    /// Channel-specific metadata.
    #[serde(default)]
    pub metadata: Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbound_message_default() {
        let msg = InboundMessage::default();
        assert_eq!(msg.id.as_str(), "");
        assert!(msg.channel.is_empty());
        assert!(msg.account_id.is_empty());
        assert!(msg.text.is_empty());
        assert!(msg.media.is_empty());
        assert!(msg.quote.is_none());
        assert!(msg.thread.is_none());
        assert_eq!(msg.metadata, Value::Null);
    }

    #[test]
    fn test_media_type_default_is_document() {
        assert_eq!(MediaType::default(), MediaType::Document);
    }

    #[test]
    fn test_media_type_serde_roundtrip() {
        let types = [
            MediaType::Image,
            MediaType::Audio,
            MediaType::Video,
            MediaType::Voice,
            MediaType::Document,
            MediaType::Sticker,
        ];
        for media_type in &types {
            let json = serde_json::to_string(media_type).unwrap();
            let parsed: MediaType = serde_json::from_str(&json).unwrap();
            assert_eq!(*media_type, parsed);
        }
    }

    #[test]
    fn test_media_type_serde_values() {
        // Verify the rename_all = "lowercase" serialization.
        assert_eq!(serde_json::to_string(&MediaType::Image).unwrap(), "\"image\"");
        assert_eq!(serde_json::to_string(&MediaType::Voice).unwrap(), "\"voice\"");
    }

    #[test]
    fn test_parse_mode_serde_roundtrip() {
        let modes = [ParseMode::Markdown, ParseMode::Html, ParseMode::Plain];
        for mode in &modes {
            let json = serde_json::to_string(mode).unwrap();
            let parsed: ParseMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, parsed);
        }
    }

    #[test]
    fn test_outbound_message_default() {
        let msg = OutboundMessage::default();
        assert!(msg.text.is_empty());
        assert!(msg.media.is_empty());
        assert!(msg.mentions.is_empty());
        assert!(msg.reply_to.is_none());
        assert!(!msg.options.disable_preview);
        assert!(!msg.options.silent);
    }

    #[test]
    fn test_send_options_default() {
        let opts = SendOptions::default();
        assert!(!opts.disable_preview);
        assert!(!opts.silent);
        assert!(opts.parse_mode.is_none());
        assert!(opts.keyboard.is_none());
    }

    #[test]
    fn test_sender_info_default() {
        let sender = SenderInfo::default();
        assert!(sender.id.is_empty());
        assert!(sender.username.is_none());
        assert!(sender.display_name.is_none());
        assert!(sender.phone_number.is_none());
        assert!(!sender.is_bot);
    }

    #[test]
    fn test_chat_info_default() {
        let chat = ChatInfo::default();
        assert!(chat.id.is_empty());
        assert_eq!(chat.chat_type, super::super::ChatType::Direct);
        assert!(chat.title.is_none());
        assert!(chat.guild_id.is_none());
    }

    #[test]
    fn test_media_source_url_serde() {
        let source = MediaSource::Url("https://example.com/img.png".to_string());
        let json = serde_json::to_string(&source).unwrap();
        let parsed: MediaSource = serde_json::from_str(&json).unwrap();
        match parsed {
            MediaSource::Url(url) => assert_eq!(url, "https://example.com/img.png"),
            _ => panic!("Expected MediaSource::Url"),
        }
    }
}
