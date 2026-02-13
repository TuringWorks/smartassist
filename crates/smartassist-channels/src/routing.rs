//! Message routing for channels.

use crate::error::ChannelError;
use crate::Result;
use smartassist_core::types::{AgentId, ChatType, InboundMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::debug;

/// Router for directing messages to agents.
#[derive(Debug)]
pub struct Router {
    /// Routing rules in priority order.
    rules: Vec<RouteRule>,

    /// Default agent if no rules match.
    default_agent: Option<AgentId>,

    /// Cache of recent routing decisions.
    cache: HashMap<String, RouteMatch>,
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    /// Create a new router.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            default_agent: None,
            cache: HashMap::new(),
        }
    }

    /// Set the default agent.
    pub fn with_default_agent(mut self, agent_id: AgentId) -> Self {
        self.default_agent = Some(agent_id);
        self
    }

    /// Add a routing rule.
    pub fn add_rule(&mut self, rule: RouteRule) {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Remove a routing rule by ID.
    pub fn remove_rule(&mut self, rule_id: &str) {
        self.rules.retain(|r| r.id != rule_id);
    }

    /// Route a message to an agent.
    pub fn route(&self, message: &InboundMessage) -> Result<RouteMatch> {
        // Check cache first
        let cache_key = self.cache_key(message);
        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.clone());
        }

        let sender_name = message.sender.display_name.as_deref().unwrap_or(&message.sender.id);

        // Try each rule in priority order
        for rule in &self.rules {
            if rule.matches(message) {
                let route_match = RouteMatch {
                    agent_id: rule.agent_id.clone(),
                    rule_id: Some(rule.id.clone()),
                    reason: MatchReason::Rule(rule.id.clone()),
                };
                debug!(
                    "Routed message from {} to agent {} (rule: {})",
                    sender_name,
                    route_match.agent_id,
                    rule.id
                );
                return Ok(route_match);
            }
        }

        // Fall back to default agent
        if let Some(ref default) = self.default_agent {
            let route_match = RouteMatch {
                agent_id: default.clone(),
                rule_id: None,
                reason: MatchReason::Default,
            };
            debug!(
                "Routed message from {} to default agent {}",
                sender_name,
                default
            );
            return Ok(route_match);
        }

        Err(ChannelError::Routing(format!(
            "No route found for message from {}",
            sender_name
        )))
    }

    /// Clear the routing cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Generate a cache key for a message.
    fn cache_key(&self, message: &InboundMessage) -> String {
        format!(
            "{}:{}:{}",
            message.channel,
            message.account_id,
            message.sender.id
        )
    }
}

/// A routing rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRule {
    /// Rule identifier.
    pub id: String,

    /// Target agent ID.
    pub agent_id: AgentId,

    /// Rule priority (higher = checked first).
    #[serde(default)]
    pub priority: i32,

    /// Match conditions.
    #[serde(default)]
    pub conditions: RouteConditions,
}

impl RouteRule {
    /// Create a new routing rule.
    pub fn new(id: impl Into<String>, agent_id: AgentId) -> Self {
        Self {
            id: id.into(),
            agent_id,
            priority: 0,
            conditions: RouteConditions::default(),
        }
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    /// Add a channel condition.
    pub fn match_channel(mut self, channel: impl Into<String>) -> Self {
        self.conditions.channel = Some(channel.into());
        self
    }

    /// Add an account condition.
    pub fn match_account(mut self, account: impl Into<String>) -> Self {
        self.conditions.account = Some(account.into());
        self
    }

    /// Add a peer condition.
    pub fn match_peer(mut self, peer: impl Into<String>) -> Self {
        self.conditions.peer = Some(peer.into());
        self
    }

    /// Add a guild/server condition.
    pub fn match_guild(mut self, guild: impl Into<String>) -> Self {
        self.conditions.guild = Some(guild.into());
        self
    }

    /// Add a chat type condition.
    pub fn match_chat_type(mut self, chat_type: ChatType) -> Self {
        self.conditions.chat_type = Some(chat_type);
        self
    }

    /// Check if this rule matches a message.
    pub fn matches(&self, message: &InboundMessage) -> bool {
        // Check channel
        if let Some(ref channel) = self.conditions.channel {
            if message.channel != *channel {
                return false;
            }
        }

        // Check account
        if let Some(ref account) = self.conditions.account {
            if message.account_id != *account {
                return false;
            }
        }

        // Check peer
        if let Some(ref peer) = self.conditions.peer {
            if message.sender.id != *peer {
                return false;
            }
        }

        // Check guild
        if let Some(ref guild) = self.conditions.guild {
            if message.chat.guild_id.as_deref() != Some(guild.as_str()) {
                return false;
            }
        }

        // Check chat type
        if let Some(ref chat_type) = self.conditions.chat_type {
            if message.chat.chat_type != *chat_type {
                return false;
            }
        }

        true
    }
}

/// Conditions for route matching.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RouteConditions {
    /// Match specific channel type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,

    /// Match specific account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,

    /// Match specific peer (sender).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer: Option<String>,

    /// Match specific guild/server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild: Option<String>,

    /// Match specific chat type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_type: Option<ChatType>,
}

/// Result of routing a message.
#[derive(Debug, Clone)]
pub struct RouteMatch {
    /// Target agent ID.
    pub agent_id: AgentId,

    /// ID of the rule that matched (if any).
    pub rule_id: Option<String>,

    /// Reason for the match.
    pub reason: MatchReason,
}

/// Reason for a route match.
#[derive(Debug, Clone)]
pub enum MatchReason {
    /// Matched a specific rule.
    Rule(String),

    /// Used default agent.
    Default,

    /// Used cached routing.
    Cached,
}

/// Builder for creating routers.
#[derive(Debug, Default)]
pub struct RouterBuilder {
    router: Router,
}

impl RouterBuilder {
    /// Create a new router builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default agent.
    pub fn default_agent(mut self, agent_id: AgentId) -> Self {
        self.router.default_agent = Some(agent_id);
        self
    }

    /// Add a rule.
    pub fn rule(mut self, rule: RouteRule) -> Self {
        self.router.add_rule(rule);
        self
    }

    /// Build the router.
    pub fn build(self) -> Router {
        self.router
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smartassist_core::types::{ChatInfo, MessageId, SenderInfo};

    fn test_message(channel: &str, sender_id: &str) -> InboundMessage {
        InboundMessage {
            id: MessageId::new("msg123"),
            timestamp: chrono::Utc::now(),
            channel: channel.to_string(),
            account_id: "test_account".to_string(),
            sender: SenderInfo {
                id: sender_id.to_string(),
                username: Some("testuser".to_string()),
                display_name: Some("Test User".to_string()),
                phone_number: None,
                is_bot: false,
            },
            chat: ChatInfo {
                id: "chat123".to_string(),
                chat_type: ChatType::Direct,
                title: None,
                guild_id: None,
            },
            text: "Hello".to_string(),
            media: vec![],
            quote: None,
            thread: None,
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn test_routing_with_rules() {
        let mut router = Router::new().with_default_agent(AgentId::new("default"));

        router.add_rule(
            RouteRule::new("telegram_rule", AgentId::new("telegram_agent"))
                .match_channel("telegram")
                .with_priority(10),
        );

        router.add_rule(
            RouteRule::new("discord_rule", AgentId::new("discord_agent"))
                .match_channel("discord")
                .with_priority(10),
        );

        // Test telegram routing
        let telegram_msg = test_message("telegram", "user1");
        let result = router.route(&telegram_msg).unwrap();
        assert_eq!(result.agent_id.as_str(), "telegram_agent");

        // Test discord routing
        let discord_msg = test_message("discord", "user2");
        let result = router.route(&discord_msg).unwrap();
        assert_eq!(result.agent_id.as_str(), "discord_agent");

        // Test default routing
        let other_msg = test_message("slack", "user3");
        let result = router.route(&other_msg).unwrap();
        assert_eq!(result.agent_id.as_str(), "default");
    }

    #[test]
    fn test_rule_priority() {
        let mut router = Router::new();

        router.add_rule(
            RouteRule::new("low_priority", AgentId::new("agent1"))
                .match_channel("telegram")
                .with_priority(1),
        );

        router.add_rule(
            RouteRule::new("high_priority", AgentId::new("agent2"))
                .match_channel("telegram")
                .with_priority(10),
        );

        let msg = test_message("telegram", "user1");
        let result = router.route(&msg).unwrap();
        assert_eq!(result.agent_id.as_str(), "agent2");
    }

    #[test]
    fn test_no_route_error() {
        let router = Router::new(); // No default, no rules

        let msg = test_message("telegram", "user1");
        let result = router.route(&msg);
        assert!(result.is_err());
    }
}
