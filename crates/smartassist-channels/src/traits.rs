//! Core channel traits.

use crate::attachment::Attachment;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{
    ChannelCapabilities, ChannelHealth, InboundMessage, MessageTarget, OutboundMessage,
};
use std::fmt::Debug;

/// Core channel trait combining all channel capabilities.
#[async_trait]
pub trait Channel: ChannelSender + ChannelReceiver + ChannelLifecycle + Send + Sync + Debug {
    /// Get the channel type identifier.
    fn channel_type(&self) -> &str;

    /// Get the channel instance identifier.
    fn instance_id(&self) -> &str;

    /// Get channel capabilities.
    fn capabilities(&self) -> ChannelCapabilities;

    /// Check if the channel supports a specific feature.
    fn supports(&self, feature: ChannelFeature) -> bool {
        let caps = self.capabilities();
        match feature {
            ChannelFeature::Images => caps.media.images,
            ChannelFeature::Audio => caps.media.audio,
            ChannelFeature::Video => caps.media.video,
            ChannelFeature::Files => caps.media.files,
            ChannelFeature::Stickers => caps.media.stickers,
            ChannelFeature::VoiceNotes => caps.media.voice_notes,
            ChannelFeature::Reactions => caps.features.reactions,
            ChannelFeature::Threads => caps.features.threads,
            ChannelFeature::Edits => caps.features.edits,
            ChannelFeature::Deletes => caps.features.deletes,
            ChannelFeature::TypingIndicators => caps.features.typing_indicators,
            ChannelFeature::ReadReceipts => caps.features.read_receipts,
            ChannelFeature::Mentions => caps.features.mentions,
            ChannelFeature::Polls => caps.features.polls,
        }
    }
}

/// Channel features that can be queried.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelFeature {
    Images,
    Audio,
    Video,
    Files,
    Stickers,
    VoiceNotes,
    Reactions,
    Threads,
    Edits,
    Deletes,
    TypingIndicators,
    ReadReceipts,
    Mentions,
    Polls,
}

/// Trait for sending messages through a channel.
#[async_trait]
pub trait ChannelSender: Send + Sync {
    /// Send a message through the channel.
    async fn send(&self, message: OutboundMessage) -> Result<SendResult>;

    /// Send a message with attachments.
    async fn send_with_attachments(
        &self,
        message: OutboundMessage,
        attachments: Vec<Attachment>,
    ) -> Result<SendResult>;

    /// Edit a previously sent message.
    /// Requires a MessageRef which includes both message_id and chat_id.
    async fn edit(&self, message: &MessageRef, new_content: &str) -> Result<()>;

    /// Delete a previously sent message.
    /// Requires a MessageRef which includes both message_id and chat_id.
    async fn delete(&self, message: &MessageRef) -> Result<()>;

    /// Add a reaction to a message.
    /// Requires a MessageRef which includes both message_id and chat_id.
    async fn react(&self, message: &MessageRef, emoji: &str) -> Result<()>;

    /// Remove a reaction from a message.
    /// Requires a MessageRef which includes both message_id and chat_id.
    async fn unreact(&self, message: &MessageRef, emoji: &str) -> Result<()>;

    /// Send a typing indicator.
    async fn send_typing(&self, target: &MessageTarget) -> Result<()>;

    /// Get the maximum message length for this channel.
    fn max_message_length(&self) -> usize {
        4096 // Default
    }
}

/// Reference to a specific message in a channel.
/// Used for operations that need to identify a message (edit, delete, react).
#[derive(Debug, Clone)]
pub struct MessageRef {
    /// The message ID.
    pub message_id: String,

    /// The chat/channel ID where the message exists.
    pub chat_id: String,

    /// Optional thread ID if the message is in a thread.
    pub thread_id: Option<String>,
}

impl MessageRef {
    /// Create a new message reference.
    pub fn new(message_id: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            chat_id: chat_id.into(),
            thread_id: None,
        }
    }

    /// Create a message reference in a thread.
    pub fn in_thread(
        message_id: impl Into<String>,
        chat_id: impl Into<String>,
        thread_id: impl Into<String>,
    ) -> Self {
        Self {
            message_id: message_id.into(),
            chat_id: chat_id.into(),
            thread_id: Some(thread_id.into()),
        }
    }
}

/// Result from sending a message.
#[derive(Debug, Clone)]
pub struct SendResult {
    /// Message ID assigned by the channel.
    pub message_id: String,

    /// Chat/channel ID where the message was sent.
    pub chat_id: String,

    /// Timestamp when the message was sent.
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Whether the message was delivered.
    pub delivered: bool,

    /// Additional metadata.
    pub metadata: std::collections::HashMap<String, serde_json::Value>,
}

impl SendResult {
    /// Create a new send result.
    pub fn new(message_id: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            chat_id: String::new(),
            timestamp: chrono::Utc::now(),
            delivered: true,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Create a new send result with chat ID.
    pub fn with_chat(message_id: impl Into<String>, chat_id: impl Into<String>) -> Self {
        Self {
            message_id: message_id.into(),
            chat_id: chat_id.into(),
            timestamp: chrono::Utc::now(),
            delivered: true,
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Mark as not delivered.
    pub fn not_delivered(mut self) -> Self {
        self.delivered = false;
        self
    }

    /// Add metadata.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Convert to a MessageRef for subsequent operations.
    pub fn as_ref(&self) -> MessageRef {
        MessageRef::new(&self.message_id, &self.chat_id)
    }
}

/// Trait for receiving messages from a channel.
#[async_trait]
pub trait ChannelReceiver: Send + Sync {
    /// Start receiving messages.
    async fn start_receiving(&self) -> Result<()>;

    /// Stop receiving messages.
    async fn stop_receiving(&self) -> Result<()>;

    /// Get the next incoming message (blocking).
    async fn receive(&self) -> Result<InboundMessage>;

    /// Try to get the next incoming message (non-blocking).
    async fn try_receive(&self) -> Result<Option<InboundMessage>>;

    /// Set the message handler callback.
    fn set_handler(&self, handler: Box<dyn MessageHandler>);
}

/// Handler for incoming messages.
#[async_trait]
pub trait MessageHandler: Send + Sync {
    /// Handle an incoming message.
    async fn handle(&self, message: InboundMessage) -> Result<()>;
}

/// Trait for channel lifecycle management.
#[async_trait]
pub trait ChannelLifecycle: Send + Sync {
    /// Connect to the channel.
    async fn connect(&self) -> Result<()>;

    /// Disconnect from the channel.
    async fn disconnect(&self) -> Result<()>;

    /// Check if the channel is connected.
    fn is_connected(&self) -> bool;

    /// Get channel health status.
    async fn health(&self) -> Result<ChannelHealth>;

    /// Reconnect to the channel.
    async fn reconnect(&self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }
}

/// Configuration for a channel instance.
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// Channel type.
    pub channel_type: String,

    /// Instance identifier.
    pub instance_id: String,

    /// Account identifier (e.g., bot token reference).
    pub account_id: String,

    /// Whether the channel is enabled.
    pub enabled: bool,

    /// Additional configuration options.
    pub options: std::collections::HashMap<String, serde_json::Value>,
}

impl ChannelConfig {
    /// Create a new channel config.
    pub fn new(
        channel_type: impl Into<String>,
        instance_id: impl Into<String>,
        account_id: impl Into<String>,
    ) -> Self {
        Self {
            channel_type: channel_type.into(),
            instance_id: instance_id.into(),
            account_id: account_id.into(),
            enabled: true,
            options: std::collections::HashMap::new(),
        }
    }

    /// Set an option.
    pub fn with_option(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.options.insert(key.into(), value);
        self
    }

    /// Disable the channel.
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

/// Factory for creating channel instances.
#[async_trait]
pub trait ChannelFactory: Send + Sync {
    /// Create a channel instance from configuration.
    async fn create(&self, config: ChannelConfig) -> Result<Box<dyn Channel>>;

    /// Get the channel type this factory creates.
    fn channel_type(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_result() {
        let result = SendResult::with_chat("msg123", "chat456")
            .with_metadata("thread_id", serde_json::json!("thread789"));

        assert_eq!(result.message_id, "msg123");
        assert_eq!(result.chat_id, "chat456");
        assert!(result.delivered);
        assert!(result.metadata.contains_key("thread_id"));

        // Test conversion to MessageRef
        let msg_ref = result.as_ref();
        assert_eq!(msg_ref.message_id, "msg123");
        assert_eq!(msg_ref.chat_id, "chat456");
    }

    #[test]
    fn test_message_ref() {
        let msg_ref = MessageRef::new("msg123", "chat456");
        assert_eq!(msg_ref.message_id, "msg123");
        assert_eq!(msg_ref.chat_id, "chat456");
        assert!(msg_ref.thread_id.is_none());

        let thread_ref = MessageRef::in_thread("msg123", "chat456", "thread789");
        assert_eq!(thread_ref.thread_id, Some("thread789".to_string()));
    }

    #[test]
    fn test_channel_config() {
        let config = ChannelConfig::new("telegram", "bot1", "token123")
            .with_option("webhook_url", serde_json::json!("https://example.com"));

        assert_eq!(config.channel_type, "telegram");
        assert_eq!(config.instance_id, "bot1");
        assert!(config.enabled);
    }
}
