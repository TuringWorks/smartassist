//! Message routing for channels.
//!
//! Supports rule-based routing with advanced conditions including peer kind
//! matching, plus session and thread binding so conversations stick to the
//! same agent once routed.

use crate::error::ChannelError;
use crate::Result;
use smartassist_core::types::{AgentId, ChatType, InboundMessage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// Kind of peer that sent a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeerKind {
    /// Regular user.
    User,

    /// Bot account.
    Bot,

    /// Administrator or privileged user.
    Admin,
}

/// Registry for session and thread bindings.
///
/// Once a message from a chat or thread is routed to an agent, subsequent
/// messages are automatically sent to the same agent without re-evaluating
/// rules.
#[derive(Debug, Clone, Default)]
pub struct BindingRegistry {
    /// Maps `(channel, account, chat_id)` -> `AgentId`.
    session_bindings: Arc<RwLock<HashMap<String, AgentId>>>,

    /// Maps `(channel, account, chat_id, thread_id)` -> `AgentId`.
    thread_bindings: Arc<RwLock<HashMap<String, AgentId>>>,
}

impl BindingRegistry {
    /// Create a new binding registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Bind a chat session to an agent.
    pub async fn bind_session(
        &self,
        channel: &str,
        account: &str,
        chat_id: &str,
        agent_id: AgentId,
    ) {
        let key = format!("{}:{}:{}", channel, account, chat_id);
        let mut map = self.session_bindings.write().await;
        map.insert(key, agent_id);
    }

    /// Bind a thread to an agent.
    pub async fn bind_thread(
        &self,
        channel: &str,
        account: &str,
        chat_id: &str,
        thread_id: &str,
        agent_id: AgentId,
    ) {
        let key = format!("{}:{}:{}:{}", channel, account, chat_id, thread_id);
        let mut map = self.thread_bindings.write().await;
        map.insert(key, agent_id);
    }

    /// Look up the bound agent for a chat session.
    pub async fn resolve_session(
        &self,
        channel: &str,
        account: &str,
        chat_id: &str,
    ) -> Option<AgentId> {
        let key = format!("{}:{}:{}", channel, account, chat_id);
        let map = self.session_bindings.read().await;
        map.get(&key).cloned()
    }

    /// Look up the bound agent for a thread.
    pub async fn resolve_thread(
        &self,
        channel: &str,
        account: &str,
        chat_id: &str,
        thread_id: &str,
    ) -> Option<AgentId> {
        let key = format!("{}:{}:{}:{}", channel, account, chat_id, thread_id);
        let map = self.thread_bindings.read().await;
        map.get(&key).cloned()
    }

    /// Remove a session binding.
    pub async fn unbind_session(
        &self,
        channel: &str,
        account: &str,
        chat_id: &str,
    ) {
        let key = format!("{}:{}:{}", channel, account, chat_id);
        let mut map = self.session_bindings.write().await;
        map.remove(&key);
    }

    /// Remove a thread binding.
    pub async fn unbind_thread(
        &self,
        channel: &str,
        account: &str,
        chat_id: &str,
        thread_id: &str,
    ) {
        let key = format!("{}:{}:{}:{}", channel, account, chat_id, thread_id);
        let mut map = self.thread_bindings.write().await;
        map.remove(&key);
    }

    /// Clear all bindings.
    pub async fn clear_all(&self) {
        let mut sessions = self.session_bindings.write().await;
        sessions.clear();
        let mut threads = self.thread_bindings.write().await;
        threads.clear();
    }

    /// Count total bindings.
    pub async fn len(&self) -> usize {
        let sessions = self.session_bindings.read().await;
        let threads = self.thread_bindings.read().await;
        sessions.len() + threads.len()
    }
}

/// Router for directing messages to agents.
#[derive(Debug)]
pub struct Router {
    /// Routing rules in priority order.
    rules: Vec<RouteRule>,

    /// Default agent if no rules match.
    default_agent: Option<AgentId>,

    /// Cache of recent routing decisions.
    cache: HashMap<String, RouteMatch>,

    /// Session and thread bindings.
    bindings: BindingRegistry,
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
            bindings: BindingRegistry::new(),
        }
    }

    /// Create a router with an existing binding registry.
    pub fn with_bindings(mut self, bindings: BindingRegistry) -> Self {
        self.bindings = bindings;
        self
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

    /// Get a reference to the binding registry.
    pub fn bindings(&self) -> &BindingRegistry {
        &self.bindings
    }

    /// Route a message to an agent.
    ///
    /// Resolution order:
    /// 1. Thread binding (if message has a thread_id)
    /// 2. Session binding (chat-level)
    /// 3. Routing rules (highest priority first)
    /// 4. If a matching rule has `bind_session`/`bind_thread`, create the binding
    /// 5. Default agent
    pub async fn route(&self,
        message: &InboundMessage,
    ) -> Result<RouteMatch> {
        let sender_name = message.sender.display_name.as_deref().unwrap_or(&message.sender.id);

        // 1. Thread binding
        if let Some(ref thread) = message.thread {
            if let Some(agent_id) = self
                .bindings
                .resolve_thread(&message.channel, &message.account_id, &message.chat.id, &thread.id)
                .await
            {
                debug!(
                    "Routed message from {} to agent {} (thread bound)",
                    sender_name, agent_id
                );
                return Ok(RouteMatch {
                    agent_id,
                    rule_id: None,
                    reason: MatchReason::ThreadBound,
                    binding_created: false,
                });
            }
        }

        // 2. Session binding
        if let Some(agent_id) = self
            .bindings
            .resolve_session(&message.channel, &message.account_id, &message.chat.id)
            .await
        {
            debug!(
                "Routed message from {} to agent {} (session bound)",
                sender_name, agent_id
            );
            return Ok(RouteMatch {
                agent_id,
                rule_id: None,
                reason: MatchReason::SessionBound,
                binding_created: false,
            });
        }

        // 3. Evaluate rules
        for rule in &self.rules {
            if rule.matches(message) {
                let agent_id = rule.agent_id.clone();

                // 4. Create bindings if requested
                let mut binding_created = false;
                if rule.conditions.bind_session {
                    self.bindings
                        .bind_session(
                            &message.channel,
                            &message.account_id,
                            &message.chat.id,
                            agent_id.clone(),
                        )
                        .await;
                    binding_created = true;
                }
                if rule.conditions.bind_thread {
                    if let Some(ref thread) = message.thread {
                        self.bindings
                            .bind_thread(
                                &message.channel,
                                &message.account_id,
                                &message.chat.id,
                                &thread.id,
                                agent_id.clone(),
                            )
                            .await;
                        binding_created = true;
                    }
                }

                let route_match = RouteMatch {
                    agent_id,
                    rule_id: Some(rule.id.clone()),
                    reason: MatchReason::Rule(rule.id.clone()),
                    binding_created,
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

        // 5. Fall back to default agent
        if let Some(ref default) = self.default_agent {
            let route_match = RouteMatch {
                agent_id: default.clone(),
                rule_id: None,
                reason: MatchReason::Default,
                binding_created: false,
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

    /// Add a peer kind condition.
    pub fn match_peer_kind(mut self, kind: PeerKind) -> Self {
        self.conditions.peer_kind = Some(kind);
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

    /// Bind the session (chat) to the target agent when this rule matches.
    pub fn with_session_binding(mut self) -> Self {
        self.conditions.bind_session = true;
        self
    }

    /// Bind the thread to the target agent when this rule matches.
    pub fn with_thread_binding(mut self) -> Self {
        self.conditions.bind_thread = true;
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

        // Check peer kind
        if let Some(ref peer_kind) = self.conditions.peer_kind {
            let actual = if message.sender.is_bot {
                PeerKind::Bot
            } else {
                // Admins cannot be detected from SenderInfo alone; default to User.
                PeerKind::User
            };
            if actual != *peer_kind {
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

    /// Match peer kind (user, bot, admin).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_kind: Option<PeerKind>,

    /// Match specific guild/server.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guild: Option<String>,

    /// Match specific chat type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_type: Option<ChatType>,

    /// Create a session binding when this rule matches.
    #[serde(default)]
    pub bind_session: bool,

    /// Create a thread binding when this rule matches.
    #[serde(default)]
    pub bind_thread: bool,
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

    /// Whether a new binding was created as part of this route.
    pub binding_created: bool,
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

    /// Resolved via session binding.
    SessionBound,

    /// Resolved via thread binding.
    ThreadBound,
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
    use smartassist_core::types::{ChatInfo, MessageId, SenderInfo, ThreadInfo};

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

    fn test_message_with_thread(channel: &str, sender_id: &str, thread_id: &str) -> InboundMessage {
        let mut msg = test_message(channel, sender_id);
        msg.thread = Some(ThreadInfo {
            id: thread_id.to_string(),
            parent_id: None,
        });
        msg
    }

    fn test_message_from_bot(channel: &str, sender_id: &str) -> InboundMessage {
        let mut msg = test_message(channel, sender_id);
        msg.sender.is_bot = true;
        msg
    }

    #[tokio::test]
    async fn test_routing_with_rules() {
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
        let result = router.route(&telegram_msg).await.unwrap();
        assert_eq!(result.agent_id.as_str(), "telegram_agent");

        // Test discord routing
        let discord_msg = test_message("discord", "user2");
        let result = router.route(&discord_msg).await.unwrap();
        assert_eq!(result.agent_id.as_str(), "discord_agent");

        // Test default routing
        let other_msg = test_message("slack", "user3");
        let result = router.route(&other_msg).await.unwrap();
        assert_eq!(result.agent_id.as_str(), "default");
    }

    #[tokio::test]
    async fn test_rule_priority() {
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
        let result = router.route(&msg).await.unwrap();
        assert_eq!(result.agent_id.as_str(), "agent2");
    }

    #[tokio::test]
    async fn test_no_route_error() {
        let router = Router::new(); // No default, no rules

        let msg = test_message("telegram", "user1");
        let result = router.route(&msg).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_session_binding_routes_subsequent_messages() {
        let mut router = Router::new();
        router.add_rule(
            RouteRule::new("bind_rule", AgentId::new("agent_a"))
                .match_channel("telegram")
                .with_session_binding(),
        );

        let msg = test_message("telegram", "user1");
        let result = router.route(&msg).await.unwrap();
        assert_eq!(result.agent_id.as_str(), "agent_a");
        assert!(result.binding_created);
        assert!(matches!(result.reason, MatchReason::Rule(_)));

        // Second message from same chat should use binding, not rule re-evaluation
        let msg2 = test_message("telegram", "user2");
        let result2 = router.route(&msg2).await.unwrap();
        assert_eq!(result2.agent_id.as_str(), "agent_a");
        assert!(!result2.binding_created);
        assert!(matches!(result2.reason, MatchReason::SessionBound));
    }

    #[tokio::test]
    async fn test_thread_binding_takes_precedence_over_session() {
        let mut router = Router::new();
        router.add_rule(
            RouteRule::new("bind_both", AgentId::new("agent_b"))
                .match_channel("slack")
                .with_session_binding()
                .with_thread_binding(),
        );

        let msg = test_message_with_thread("slack", "user1", "thread_1");
        let result = router.route(&msg).await.unwrap();
        assert_eq!(result.agent_id.as_str(), "agent_b");
        assert!(result.binding_created);

        // Same chat + same thread -> thread binding takes precedence over session binding
        let msg2 = test_message_with_thread("slack", "user2", "thread_1");
        let result2 = router.route(&msg2).await.unwrap();
        assert_eq!(result2.agent_id.as_str(), "agent_b");
        assert!(!result2.binding_created);
        assert!(matches!(result2.reason, MatchReason::ThreadBound));

        // Same chat, no thread -> session binding
        let msg3 = test_message("slack", "user3");
        let result3 = router.route(&msg3).await.unwrap();
        assert_eq!(result3.agent_id.as_str(), "agent_b");
        assert!(!result3.binding_created);
        assert!(matches!(result3.reason, MatchReason::SessionBound));
    }

    #[tokio::test]
    async fn test_peer_kind_matching() {
        let mut router = Router::new();
        router.add_rule(
            RouteRule::new("bot_rule", AgentId::new("bot_handler"))
                .match_channel("discord")
                .match_peer_kind(PeerKind::Bot),
        );
        router.add_rule(
            RouteRule::new("user_rule", AgentId::new("user_handler"))
                .match_channel("discord")
                .match_peer_kind(PeerKind::User),
        );

        let bot_msg = test_message_from_bot("discord", "bot1");
        let result = router.route(&bot_msg).await.unwrap();
        assert_eq!(result.agent_id.as_str(), "bot_handler");

        let user_msg = test_message("discord", "user1");
        let result = router.route(&user_msg).await.unwrap();
        assert_eq!(result.agent_id.as_str(), "user_handler");
    }

    #[tokio::test]
    async fn test_binding_registry_resolve_and_unbind() {
        let registry = BindingRegistry::new();

        registry
            .bind_session("telegram", "acct1", "chat1", AgentId::new("agent_x"))
            .await;

        let resolved = registry.resolve_session("telegram", "acct1", "chat1").await;
        assert_eq!(resolved.unwrap().as_str(), "agent_x");

        registry.unbind_session("telegram", "acct1", "chat1").await;
        let resolved = registry.resolve_session("telegram", "acct1", "chat1").await;
        assert!(resolved.is_none());
    }

    #[tokio::test]
    async fn test_binding_registry_thread_operations() {
        let registry = BindingRegistry::new();

        registry
            .bind_thread("slack", "acct2", "chat2", "t1", AgentId::new("agent_y"))
            .await;

        let resolved = registry.resolve_thread("slack", "acct2", "chat2", "t1").await;
        assert_eq!(resolved.unwrap().as_str(), "agent_y");

        // Different thread should not resolve
        let other = registry.resolve_thread("slack", "acct2", "chat2", "t2").await;
        assert!(other.is_none());

        registry.clear_all().await;
        assert_eq!(registry.len().await, 0);
    }

    #[tokio::test]
    async fn test_router_with_external_binding_registry() {
        let bindings = BindingRegistry::new();
        bindings
            .bind_session("web", "test_account", "chat3", AgentId::new("agent_z"))
            .await;

        let router = Router::new().with_bindings(bindings);
        let mut msg = test_message("web", "user9");
        msg.chat.id = "chat3".to_string();
        let result = router.route(&msg).await.unwrap();
        assert_eq!(result.agent_id.as_str(), "agent_z");
        assert!(matches!(result.reason, MatchReason::SessionBound));
    }

    #[test]
    fn test_peer_kind_serde() {
        let json = serde_json::to_string(&PeerKind::Bot).unwrap();
        assert_eq!(json, "\"bot\"");
        let parsed: PeerKind = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, PeerKind::Bot);
    }

    #[tokio::test]
    async fn test_rule_without_binding_does_not_create_binding() {
        let mut router = Router::new();
        router.add_rule(
            RouteRule::new("no_bind", AgentId::new("agent_nb"))
                .match_channel("telegram"),
        );

        let msg = test_message("telegram", "user1");
        let result = router.route(&msg).await.unwrap();
        assert!(!result.binding_created);

        // Second message should re-evaluate rule (no binding stored)
        let msg2 = test_message("telegram", "user1");
        let result2 = router.route(&msg2).await.unwrap();
        assert!(matches!(result2.reason, MatchReason::Rule(_)));
    }
}
