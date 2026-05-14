//! Heartbeat and status message filter.
//!
//! Detects periodic status messages, pings, and keep-alive traffic
//! from channels so they can be suppressed or handled specially.

use regex::Regex;
use smartassist_core::types::InboundMessage;
use std::collections::HashSet;

/// A heartbeat detection pattern.
#[derive(Debug, Clone)]
pub struct HeartbeatPattern {
    /// Human-readable name.
    pub name: String,
    /// Regex to match against message text.
    pub regex: Regex,
    /// Optional channel types where this pattern applies (empty = all).
    pub channels: HashSet<String>,
}

impl HeartbeatPattern {
    /// Create a pattern that applies to all channels.
    pub fn global(name: impl Into<String>, pattern: impl Into<String>) -> Option<Self> {
        Regex::new(&pattern.into())
            .ok()
            .map(|re| Self {
                name: name.into(),
                regex: re,
                channels: HashSet::new(),
            })
    }

    /// Create a pattern for a specific channel type.
    pub fn for_channel(
        name: impl Into<String>,
        channel: impl Into<String>,
        pattern: impl Into<String>,
    ) -> Option<Self> {
        Regex::new(&pattern.into())
            .ok()
            .map(|re| {
                let mut channels = HashSet::new();
                channels.insert(channel.into());
                Self {
                    name: name.into(),
                    regex: re,
                    channels,
                }
            })
    }

    /// Check if the pattern matches a message from the given channel.
    pub fn matches(&self, channel_type: &str, text: &str) -> bool {
        if !self.channels.is_empty() && !self.channels.contains(channel_type) {
            return false;
        }
        self.regex.is_match(text)
    }
}

/// Default heartbeat patterns shipped with SmartAssist.
pub fn default_patterns() -> Vec<HeartbeatPattern> {
    vec![
        // Discord bot keep-alive / status pings
        HeartbeatPattern::for_channel("discord_ping", "discord", r"^\s*ping\s*$").unwrap(),
        // Slack bot keep-alive
        HeartbeatPattern::for_channel("slack_status", "slack", r"^\s*$").unwrap(),
        // Telegram service messages
        HeartbeatPattern::for_channel("telegram_service", "telegram", r"^\[(?:photo|video|document|audio)\]$").unwrap(),
        // Generic short numeric/status codes
        HeartbeatPattern::global("numeric_status", r"^\d{3}$").unwrap(),
        // Empty or whitespace-only messages
        HeartbeatPattern::global("empty", r"^\s*$").unwrap(),
    ]
}

/// Filter that detects heartbeat messages.
pub struct HeartbeatFilter {
    patterns: Vec<HeartbeatPattern>,
}

impl HeartbeatFilter {
    /// Create a filter with the default patterns.
    pub fn new() -> Self {
        Self {
            patterns: default_patterns(),
        }
    }

    /// Create a filter with custom patterns.
    pub fn with_patterns(patterns: Vec<HeartbeatPattern>) -> Self {
        Self { patterns }
    }

    /// Add a pattern.
    pub fn add_pattern(&mut self, pattern: HeartbeatPattern) {
        self.patterns.push(pattern);
    }

    /// Check whether a message is a heartbeat / should be suppressed.
    pub fn is_heartbeat(&self, message: &InboundMessage) -> bool {
        let text = message.text.trim();
        let channel = message.channel.as_str();

        for pattern in &self.patterns {
            if pattern.matches(channel, text) {
                return true;
            }
        }

        false
    }

    /// Remove patterns by name.
    pub fn remove_pattern(&mut self, name: &str) {
        self.patterns.retain(|p| p.name != name);
    }

    /// List active pattern names.
    pub fn pattern_names(&self) -> Vec<&str> {
        self.patterns.iter().map(|p| p.name.as_str()).collect()
    }
}

impl Default for HeartbeatFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smartassist_core::types::{ChatInfo, ChatType, InboundMessage, MessageId, SenderInfo};

    fn heartbeat_msg(channel: &str, text: &str) -> InboundMessage {
        InboundMessage {
            id: MessageId::new("hb-1"),
            timestamp: chrono::Utc::now(),
            channel: channel.to_string(),
            account_id: "bot1".to_string(),
            sender: SenderInfo {
                id: "system".to_string(),
                username: None,
                display_name: None,
                phone_number: None,
                is_bot: false,
            },
            chat: ChatInfo {
                id: "chat1".to_string(),
                chat_type: ChatType::Direct,
                title: None,
                guild_id: None,
            },
            text: text.to_string(),
            media: vec![],
            quote: None,
            thread: None,
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn test_detects_discord_ping() {
        let filter = HeartbeatFilter::new();
        assert!(filter.is_heartbeat(&heartbeat_msg("discord", "ping")));
        assert!(filter.is_heartbeat(&heartbeat_msg("discord", "  ping  ")));
    }

    #[test]
    fn test_detects_empty_message() {
        let filter = HeartbeatFilter::new();
        assert!(filter.is_heartbeat(&heartbeat_msg("telegram", "")));
        assert!(filter.is_heartbeat(&heartbeat_msg("slack", "   ")));
    }

    #[test]
    fn test_detects_numeric_status() {
        let filter = HeartbeatFilter::new();
        assert!(filter.is_heartbeat(&heartbeat_msg("web", "200")));
        assert!(!filter.is_heartbeat(&heartbeat_msg("web", "200 OK")));
    }

    #[test]
    fn test_normal_message_not_heartbeat() {
        let filter = HeartbeatFilter::new();
        assert!(!filter.is_heartbeat(&heartbeat_msg("discord", "Hello everyone!")));
        assert!(!filter.is_heartbeat(&heartbeat_msg("telegram", "How are you?")));
    }

    #[test]
    fn test_channel_specific_pattern() {
        let filter = HeartbeatFilter::new();
        // "ping" pattern only applies to discord
        assert!(!filter.is_heartbeat(&heartbeat_msg("telegram", "ping")));
    }

    #[test]
    fn test_add_and_remove_pattern() {
        let mut filter = HeartbeatFilter::new();
        filter.remove_pattern("empty");
        assert!(!filter.is_heartbeat(&heartbeat_msg("discord", "")));

        filter.add_pattern(HeartbeatPattern::global("custom", r"^test$").unwrap());
        assert!(filter.is_heartbeat(&heartbeat_msg("any", "test")));
    }

    #[test]
    fn test_pattern_names() {
        let filter = HeartbeatFilter::new();
        let names = filter.pattern_names();
        assert!(names.contains(&"discord_ping"));
        assert!(names.contains(&"empty"));
    }
}
