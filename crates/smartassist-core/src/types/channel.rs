//! Channel-related types.

use serde::{Deserialize, Serialize};

/// Type of chat.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatType {
    /// Direct message / 1:1 chat.
    #[default]
    Direct,

    /// Group chat.
    Group,

    /// Broadcast channel.
    Channel,

    /// Thread within a group/channel.
    Thread,
}

/// Channel capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelCapabilities {
    /// Supported chat types.
    #[serde(default)]
    pub chat_types: Vec<ChatType>,

    /// Media capabilities.
    #[serde(default)]
    pub media: MediaCapabilities,

    /// Feature support.
    #[serde(default)]
    pub features: ChannelFeatures,

    /// Message limits.
    #[serde(default)]
    pub limits: ChannelLimits,
}

/// Media capabilities of a channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaCapabilities {
    /// Supports images.
    #[serde(default)]
    pub images: bool,

    /// Supports audio.
    #[serde(default)]
    pub audio: bool,

    /// Supports video.
    #[serde(default)]
    pub video: bool,

    /// Supports files/documents.
    #[serde(default)]
    pub files: bool,

    /// Supports stickers.
    #[serde(default)]
    pub stickers: bool,

    /// Supports voice notes.
    #[serde(default)]
    pub voice_notes: bool,

    /// Maximum file size in MB.
    #[serde(default)]
    pub max_file_size_mb: u32,
}

/// Feature support of a channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelFeatures {
    /// Supports reactions.
    #[serde(default)]
    pub reactions: bool,

    /// Supports threads.
    #[serde(default)]
    pub threads: bool,

    /// Supports message edits.
    #[serde(default)]
    pub edits: bool,

    /// Supports message deletes.
    #[serde(default)]
    pub deletes: bool,

    /// Supports typing indicators.
    #[serde(default)]
    pub typing_indicators: bool,

    /// Supports read receipts.
    #[serde(default)]
    pub read_receipts: bool,

    /// Supports mentions.
    #[serde(default)]
    pub mentions: bool,

    /// Supports polls.
    #[serde(default)]
    pub polls: bool,

    /// Supports native slash commands.
    #[serde(default)]
    pub native_commands: bool,
}

/// Message limits for a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelLimits {
    /// Maximum text message length.
    pub text_max_length: usize,

    /// Maximum caption length.
    pub caption_max_length: usize,

    /// Messages per second rate limit.
    pub messages_per_second: f32,

    /// Messages per minute rate limit.
    pub messages_per_minute: u32,
}

impl Default for ChannelLimits {
    fn default() -> Self {
        Self {
            text_max_length: 4096,
            caption_max_length: 1024,
            messages_per_second: 1.0,
            messages_per_minute: 30,
        }
    }
}

/// Channel metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMeta {
    /// Display label.
    pub label: String,

    /// Documentation URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,

    /// Alternative names/aliases.
    #[serde(default)]
    pub aliases: Vec<String>,

    /// Setup complexity (1-5).
    #[serde(default = "default_complexity")]
    pub setup_complexity: u8,

    /// Whether this is an extension channel.
    #[serde(default)]
    pub is_extension: bool,
}

fn default_complexity() -> u8 {
    3
}

/// Channel health status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelHealth {
    /// Health status.
    pub status: HealthStatus,

    /// Latency in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,

    /// Last message timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Error message if unhealthy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Default for ChannelHealth {
    fn default() -> Self {
        Self {
            status: HealthStatus::Unknown,
            latency_ms: None,
            last_message_at: None,
            error: None,
        }
    }
}

/// Health status of a channel.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Channel is healthy.
    Healthy,

    /// Channel is degraded but functional.
    Degraded,

    /// Channel is unhealthy.
    Unhealthy,

    /// Health status unknown.
    #[default]
    Unknown,
}

/// Target for message delivery.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageTarget {
    /// Chat/conversation ID.
    pub chat_id: String,

    /// Thread ID (if targeting a thread).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
}

impl MessageTarget {
    /// Create a new message target.
    pub fn new(chat_id: impl Into<String>) -> Self {
        Self {
            chat_id: chat_id.into(),
            thread_id: None,
        }
    }

    /// Create a message target with a thread.
    pub fn with_thread(chat_id: impl Into<String>, thread_id: impl Into<String>) -> Self {
        Self {
            chat_id: chat_id.into(),
            thread_id: Some(thread_id.into()),
        }
    }
}

/// DM (direct message) policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DmPolicy {
    /// Allow all DMs.
    Open,

    /// Require pairing for new senders.
    #[default]
    Pairing,

    /// Only allow whitelisted senders.
    Allowlist,

    /// Block all DMs.
    Blocked,
}

/// DM scope for session routing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DmScope {
    /// All DMs use the same session.
    Main,

    /// Separate session per peer.
    #[default]
    PerPeer,

    /// Separate session per channel + peer.
    PerChannelPeer,

    /// Fully isolated per account + channel + peer.
    PerAccountChannelPeer,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_type_default_is_direct() {
        assert_eq!(ChatType::default(), ChatType::Direct);
    }

    #[test]
    fn test_channel_limits_defaults() {
        let limits = ChannelLimits::default();
        assert_eq!(limits.text_max_length, 4096);
        assert_eq!(limits.caption_max_length, 1024);
        assert!((limits.messages_per_second - 1.0).abs() < f32::EPSILON);
        assert_eq!(limits.messages_per_minute, 30);
    }

    #[test]
    fn test_message_target_new() {
        let target = MessageTarget::new("chat-123");
        assert_eq!(target.chat_id, "chat-123");
        assert!(target.thread_id.is_none());
    }

    #[test]
    fn test_message_target_with_thread() {
        let target = MessageTarget::with_thread("chat-123", "thread-456");
        assert_eq!(target.chat_id, "chat-123");
        assert_eq!(target.thread_id.as_deref(), Some("thread-456"));
    }

    #[test]
    fn test_dm_policy_default_is_pairing() {
        assert_eq!(DmPolicy::default(), DmPolicy::Pairing);
    }

    #[test]
    fn test_dm_scope_default_is_per_peer() {
        assert_eq!(DmScope::default(), DmScope::PerPeer);
    }

    #[test]
    fn test_health_status_default_is_unknown() {
        assert_eq!(HealthStatus::default(), HealthStatus::Unknown);
    }

    #[test]
    fn test_channel_health_default() {
        let health = ChannelHealth::default();
        assert_eq!(health.status, HealthStatus::Unknown);
        assert!(health.latency_ms.is_none());
        assert!(health.last_message_at.is_none());
        assert!(health.error.is_none());
    }

    #[test]
    fn test_chat_type_serde_roundtrip() {
        let types = [ChatType::Direct, ChatType::Group, ChatType::Channel, ChatType::Thread];
        for ct in &types {
            let json = serde_json::to_string(ct).unwrap();
            let parsed: ChatType = serde_json::from_str(&json).unwrap();
            assert_eq!(*ct, parsed);
        }
    }

    #[test]
    fn test_health_status_serde_roundtrip() {
        let statuses = [
            HealthStatus::Healthy,
            HealthStatus::Degraded,
            HealthStatus::Unhealthy,
            HealthStatus::Unknown,
        ];
        for s in &statuses {
            let json = serde_json::to_string(s).unwrap();
            let parsed: HealthStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(*s, parsed);
        }
    }
}
