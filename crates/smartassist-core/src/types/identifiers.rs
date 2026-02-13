//! Strongly-typed identifiers.

use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Strongly-typed agent identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    /// Create a new agent ID, normalizing the input.
    pub fn new(id: impl Into<String>) -> Self {
        let normalized = id
            .into()
            .to_lowercase()
            .replace([' ', '-'], "_")
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        Self(normalized)
    }

    /// Get the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Default for AgentId {
    fn default() -> Self {
        Self::new("default")
    }
}

impl From<&str> for AgentId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for AgentId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Session key for conversation isolation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionKey(String);

impl SessionKey {
    /// Create a new session key.
    pub fn new(key: impl Into<String>) -> Self {
        Self(key.into())
    }

    /// Create a session key for a channel message.
    pub fn for_channel(channel: &str, account: &str, peer: &str, agent: &AgentId) -> Self {
        Self(format!("{}:{}:{}:{}", agent, channel, account, peer))
    }

    /// Create a session key for a subagent.
    pub fn for_subagent(parent: &AgentId) -> Self {
        let uuid = Uuid::new_v4();
        Self(format!("{}:subagent:{}", parent, uuid))
    }

    /// Check if this is a subagent session.
    pub fn is_subagent(&self) -> bool {
        self.0.contains(":subagent:")
    }

    /// Get the session key as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Extract agent ID from session key if present.
    pub fn agent_id(&self) -> Option<AgentId> {
        let parts: Vec<&str> = self.0.split(':').collect();
        if !parts.is_empty() {
            Some(AgentId::new(parts[0]))
        } else {
            None
        }
    }
}

impl fmt::Display for SessionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for SessionKey {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for SessionKey {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Approval request identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ApprovalId(String);

impl ApprovalId {
    /// Create a new random approval ID.
    pub fn new() -> Self {
        let bytes: [u8; 4] = rand::random();
        Self(base64::Engine::encode(
            &base64::engine::general_purpose::URL_SAFE_NO_PAD,
            bytes,
        ))
    }

    /// Create from an existing string.
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for ApprovalId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ApprovalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Message identifier (channel-specific).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(String);

impl MessageId {
    /// Create a new message ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for MessageId {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for MessageId {
    fn from(s: String) -> Self {
        Self::new(s)
    }
}

/// Request identifier for tracing.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RequestId(String);

impl RequestId {
    /// Create a new random request ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create from an existing string.
    pub fn from_string(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Get the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for RequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id_normalization() {
        assert_eq!(AgentId::new("My Agent").as_str(), "my_agent");
        assert_eq!(AgentId::new("test-agent").as_str(), "test_agent");
        assert_eq!(AgentId::new("UPPER").as_str(), "upper");
    }

    #[test]
    fn test_session_key_subagent() {
        let parent = AgentId::new("main");
        let key = SessionKey::for_subagent(&parent);
        assert!(key.is_subagent());
        assert!(key.as_str().starts_with("main:subagent:"));
    }

    #[test]
    fn test_session_key_channel() {
        let agent = AgentId::new("bot");
        let key = SessionKey::for_channel("telegram", "123", "456", &agent);
        assert!(!key.is_subagent());
        assert_eq!(key.as_str(), "bot:telegram:123:456");
    }
}
