//! Auto-reply engine for messaging channels.
//!
//! Provides rule-based automatic responses to incoming messages.
//! Rules can match by exact text, substring, or regex pattern.

use regex::Regex;
use smartassist_core::types::{
    InboundMessage, MessageTarget, OutboundMessage,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How a rule should match incoming message text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchMode {
    /// Match the entire message exactly.
    Exact,
    /// Match if the message contains the pattern.
    Contains,
    /// Match using a regex pattern.
    Regex,
}

/// A single auto-reply rule.
#[derive(Debug, Clone)]
pub struct AutoReplyRule {
    /// Unique rule identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Pattern to match against message text.
    pub pattern: String,
    /// How to apply the pattern.
    pub mode: MatchMode,
    /// Response text to send when matched.
    pub response: String,
    /// Whether the rule is active.
    pub enabled: bool,
    /// Minimum seconds between triggers (per chat).
    pub cooldown_secs: u64,
}

impl AutoReplyRule {
    /// Create a simple exact-match rule.
    pub fn exact(id: impl Into<String>, pattern: impl Into<String>, response: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            pattern: pattern.into(),
            mode: MatchMode::Exact,
            response: response.into(),
            enabled: true,
            cooldown_secs: 0,
        }
    }

    /// Create a contains-match rule.
    pub fn contains(id: impl Into<String>, pattern: impl Into<String>, response: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            pattern: pattern.into(),
            mode: MatchMode::Contains,
            response: response.into(),
            enabled: true,
            cooldown_secs: 0,
        }
    }

    /// Create a regex-match rule.
    pub fn regex(id: impl Into<String>, pattern: impl Into<String>, response: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: String::new(),
            pattern: pattern.into(),
            mode: MatchMode::Regex,
            response: response.into(),
            enabled: true,
            cooldown_secs: 0,
        }
    }

    /// Set a cooldown in seconds.
    pub fn with_cooldown(mut self, secs: u64) -> Self {
        self.cooldown_secs = secs;
        self
    }

    /// Set a human-readable name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }
}

/// Per-chat last trigger timestamps.
#[derive(Debug, Default)]
struct CooldownState {
    last_trigger: HashMap<String, Instant>,
}

impl CooldownState {
    fn is_ready(&self, rule_id: &str, cooldown: Duration) -> bool {
        if cooldown.is_zero() {
            return true;
        }
        self.last_trigger
            .get(rule_id)
            .map(|last| last.elapsed() >= cooldown)
            .unwrap_or(true)
    }

    fn mark_triggered(&mut self, rule_id: String) {
        self.last_trigger.insert(rule_id, Instant::now());
    }
}

/// Engine that evaluates auto-reply rules against incoming messages.
pub struct AutoReplyEngine {
    rules: Vec<AutoReplyRule>,
    cooldowns: Mutex<HashMap<String, CooldownState>>,
}

impl AutoReplyEngine {
    /// Create a new engine with no rules.
    pub fn new() -> Self {
        Self {
            rules: Vec::new(),
            cooldowns: Mutex::new(HashMap::new()),
        }
    }

    /// Add a rule.
    pub fn add_rule(&mut self, rule: AutoReplyRule) {
        self.rules.push(rule);
    }

    /// Remove a rule by ID.
    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        let len = self.rules.len();
        self.rules.retain(|r| r.id != rule_id);
        self.rules.len() < len
    }

    /// List all rules.
    pub fn list_rules(&self) -> &[AutoReplyRule] {
        &self.rules
    }

    /// Evaluate all rules against a message and return the first matching reply.
    pub fn evaluate(&self, message: &InboundMessage) -> Option<OutboundMessage> {
        let chat_id = message.chat.id.clone();
        let text = message.text.trim();

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }
            if !self.check_cooldown(&rule.id, &chat_id, rule.cooldown_secs) {
                continue;
            }
            if Self::matches(&rule.mode, &rule.pattern, text) {
                self.record_trigger(&rule.id, &chat_id);
                return Some(OutboundMessage {
                    target: MessageTarget::new(chat_id),
                    text: rule.response.clone(),
                    media: vec![],
                    mentions: vec![],
                    reply_to: Some(message.id.as_str().to_string()),
                    options: Default::default(),
                });
            }
        }

        None
    }

    fn matches(mode: &MatchMode, pattern: &str, text: &str) -> bool {
        match mode {
            MatchMode::Exact => text == pattern,
            MatchMode::Contains => text.contains(pattern),
            MatchMode::Regex => {
                Regex::new(pattern)
                    .map(|re| re.is_match(text))
                    .unwrap_or(false)
            }
        }
    }

    fn check_cooldown(&self,
        rule_id: &str,
        chat_id: &str,
        cooldown_secs: u64,
    ) -> bool {
        if cooldown_secs == 0 {
            return true;
        }
        let mut map = self.cooldowns.lock().unwrap();
        let state = map.entry(chat_id.to_string()).or_default();
        state.is_ready(rule_id, Duration::from_secs(cooldown_secs))
    }

    fn record_trigger(&self,
        rule_id: &str,
        chat_id: &str,
    ) {
        let mut map = self.cooldowns.lock().unwrap();
        let state = map.entry(chat_id.to_string()).or_default();
        state.mark_triggered(rule_id.to_string());
    }
}

impl Default for AutoReplyEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smartassist_core::types::{ChatInfo, ChatType, InboundMessage, MessageId, SenderInfo};

    fn test_message(text: &str) -> InboundMessage {
        InboundMessage {
            id: MessageId::new("msg-1"),
            timestamp: chrono::Utc::now(),
            channel: "telegram".to_string(),
            account_id: "bot1".to_string(),
            sender: SenderInfo {
                id: "user1".to_string(),
                username: None,
                display_name: Some("Alice".to_string()),
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
    fn test_exact_match() {
        let mut engine = AutoReplyEngine::new();
        engine.add_rule(AutoReplyRule::exact("hi", "hello", "Hi there!"));

        assert!(engine.evaluate(&test_message("hello")).is_some());
        assert!(engine.evaluate(&test_message("hello world")).is_none());
        assert!(engine.evaluate(&test_message("hi")).is_none());
    }

    #[test]
    fn test_contains_match() {
        let mut engine = AutoReplyEngine::new();
        engine.add_rule(AutoReplyRule::contains("help", "help", "I can help you!"));

        assert!(engine.evaluate(&test_message("I need help")).is_some());
        assert!(engine.evaluate(&test_message("help me")).is_some());
        assert!(engine.evaluate(&test_message("hello")).is_none());
    }

    #[test]
    fn test_regex_match() {
        let mut engine = AutoReplyEngine::new();
        engine.add_rule(AutoReplyRule::regex("price", r"\bprice\b", "Check our pricing page."));

        assert!(engine.evaluate(&test_message("what is the price?")).is_some());
        assert!(engine.evaluate(&test_message("priced items")).is_none());
    }

    #[test]
    fn test_disabled_rule_ignored() {
        let mut engine = AutoReplyEngine::new();
        let mut rule = AutoReplyRule::exact("disabled", "test", "response");
        rule.enabled = false;
        engine.add_rule(rule);

        assert!(engine.evaluate(&test_message("test")).is_none());
    }

    #[test]
    fn test_cooldown_blocks_repeat() {
        let mut engine = AutoReplyEngine::new();
        engine.add_rule(
            AutoReplyRule::exact("cool", "cool", "cool response")
                .with_cooldown(60),
        );

        let msg = test_message("cool");
        assert!(engine.evaluate(&msg).is_some());
        assert!(engine.evaluate(&msg).is_none()); // within cooldown
    }

    #[test]
    fn test_remove_rule() {
        let mut engine = AutoReplyEngine::new();
        engine.add_rule(AutoReplyRule::exact("a", "a", "A"));
        assert!(engine.remove_rule("a"));
        assert!(!engine.remove_rule("a"));
    }

    #[test]
    fn test_response_has_reply_to() {
        let mut engine = AutoReplyEngine::new();
        engine.add_rule(AutoReplyRule::exact("ping", "ping", "pong"));

        let reply = engine.evaluate(&test_message("ping")).unwrap();
        assert_eq!(reply.text, "pong");
        assert_eq!(reply.reply_to, Some("msg-1".to_string()));
        assert_eq!(reply.target.chat_id, "chat1");
    }
}
