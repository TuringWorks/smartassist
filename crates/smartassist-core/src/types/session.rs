//! Session types for conversation management.

use super::{AgentId, SessionKey, ThinkingLevel, ToolDefinition};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// A conversation session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Unique session key.
    pub key: SessionKey,

    /// Agent this session belongs to.
    pub agent_id: AgentId,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Last activity timestamp.
    pub last_message_at: DateTime<Utc>,

    /// Conversation messages.
    #[serde(default)]
    pub messages: Vec<Message>,

    /// Token usage statistics.
    #[serde(default)]
    pub tokens: TokenUsage,

    /// Cost tracking.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost: Option<CostUsage>,

    /// Model override for this session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Thinking level override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level: Option<ThinkingLevel>,

    /// Typing indicator mode.
    #[serde(default)]
    pub type_mode: TypeMode,

    /// Session metadata.
    #[serde(default)]
    pub metadata: SessionMetadata,
}

impl Session {
    /// Create a new session.
    pub fn new(key: SessionKey, agent_id: AgentId) -> Self {
        let now = Utc::now();
        Self {
            key,
            agent_id,
            created_at: now,
            last_message_at: now,
            messages: Vec::new(),
            tokens: TokenUsage::default(),
            cost: None,
            model: None,
            thinking_level: None,
            type_mode: TypeMode::default(),
            metadata: SessionMetadata::default(),
        }
    }

    /// Add a message to the session.
    pub fn add_message(&mut self, message: Message) {
        self.last_message_at = Utc::now();
        self.messages.push(message);
    }

    /// Get the total number of tokens used.
    pub fn total_tokens(&self) -> u64 {
        self.tokens.total()
    }
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender.
    pub role: Role,

    /// Message content.
    pub content: MessageContent,

    /// Optional name (for tool messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Tool use ID (for tool results).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_use_id: Option<String>,

    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

impl Message {
    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_use_id: None,
            timestamp: Utc::now(),
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_use_id: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_use_id: None,
            timestamp: Utc::now(),
        }
    }

    /// Create a tool result message.
    pub fn tool_result(tool_use_id: impl Into<String>, content: impl Into<String>, is_error: bool) -> Self {
        Self {
            role: Role::Tool,
            content: MessageContent::Blocks(vec![ContentBlock::ToolResult {
                tool_use_id: tool_use_id.into(),
                content: content.into(),
                is_error,
            }]),
            name: None,
            tool_use_id: None,
            timestamp: Utc::now(),
        }
    }
}

/// Role of a message sender.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

impl Role {
    /// Check if this is a system message.
    pub fn is_system(&self) -> bool {
        matches!(self, Self::System)
    }

    /// Check if this is a user message.
    pub fn is_user(&self) -> bool {
        matches!(self, Self::User)
    }

    /// Check if this is an assistant message.
    pub fn is_assistant(&self) -> bool {
        matches!(self, Self::Assistant)
    }

    /// Check if this is a tool result message.
    pub fn is_tool(&self) -> bool {
        matches!(self, Self::Tool)
    }
}

/// Content of a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content.
    Text(String),

    /// Structured content blocks.
    Blocks(Vec<ContentBlock>),
}

impl MessageContent {
    /// Get as text if this is a simple text content.
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            Self::Blocks(blocks) => {
                if blocks.len() == 1 {
                    if let ContentBlock::Text { text } = &blocks[0] {
                        return Some(text);
                    }
                }
                None
            }
        }
    }

    /// Convert to text, joining blocks if necessary.
    pub fn to_text(&self) -> String {
        match self {
            Self::Text(s) => s.clone(),
            Self::Blocks(blocks) => blocks
                .iter()
                .filter_map(|b| match b {
                    ContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join(""),
        }
    }
}

/// A content block within a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    /// Text content.
    Text { text: String },

    /// Image content.
    Image { source: ImageSource },

    /// Tool use request.
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },

    /// Tool result.
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },

    /// Thinking content (extended thinking).
    Thinking { thinking: String },
}

/// Image source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageSourceType {
    /// Base64-encoded image data.
    Base64,
    /// URL to an image.
    Url,
}

/// Source of an image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    /// Source type.
    #[serde(rename = "type")]
    pub source_type: ImageSourceType,

    /// MIME type (e.g., "image/jpeg").
    pub media_type: String,

    /// Image data (base64 for Base64, URL for Url).
    pub data: String,
}

impl ImageSource {
    /// Create a base64 image source.
    pub fn base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            source_type: ImageSourceType::Base64,
            media_type: media_type.into(),
            data: data.into(),
        }
    }

    /// Create a URL image source.
    pub fn url(url: impl Into<String>) -> Self {
        Self {
            source_type: ImageSourceType::Url,
            media_type: String::new(),
            data: url.into(),
        }
    }
}

/// Token usage statistics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens.
    pub input: u64,

    /// Output tokens.
    pub output: u64,

    /// Cache creation tokens.
    #[serde(default)]
    pub cache_creation: u64,

    /// Cache read tokens.
    #[serde(default)]
    pub cache_read: u64,
}

impl TokenUsage {
    /// Get the total token count.
    pub fn total(&self) -> u64 {
        self.input + self.output + self.cache_creation + self.cache_read
    }

    /// Add another usage to this one.
    pub fn add(&mut self, other: &TokenUsage) {
        self.input += other.input;
        self.output += other.output;
        self.cache_creation += other.cache_creation;
        self.cache_read += other.cache_read;
    }
}

/// Cost usage in USD.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostUsage {
    /// Input cost.
    pub input_usd: f64,

    /// Output cost.
    pub output_usd: f64,

    /// Total cost.
    pub total_usd: f64,
}

/// Typing indicator mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TypeMode {
    /// Show typing indicator.
    #[default]
    Typing,

    /// Never show typing.
    Never,

    /// Show during thinking.
    Thinking,

    /// Show per message.
    Message,
}

/// Session metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Channel context.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,

    /// Account ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,

    /// Peer ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_id: Option<String>,

    /// Custom labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of response.
    EndTurn,
    /// Hit a stop sequence.
    StopSequence,
    /// Hit max tokens limit.
    MaxTokens,
    /// Model wants to use a tool.
    ToolUse,
    /// Content was filtered.
    ContentFilter,
    /// Unknown reason.
    Unknown,
}

/// Tool choice mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// Model decides whether to use tools.
    Auto,
    /// Model must use a tool.
    Any,
    /// Model cannot use tools.
    None,
    /// Model must use a specific tool.
    Tool { name: String },
}

/// Chat completion options.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChatOptions {
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,

    /// Temperature for sampling (0.0 to 2.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,

    /// Top-p sampling parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,

    /// Top-k sampling parameter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,

    /// Stop sequences.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,

    /// Tools available for the model to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,

    /// Tool choice mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,

    /// User identifier for rate limiting/abuse detection.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,

    /// Additional provider-specific options.
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl ChatOptions {
    /// Create new chat options with max tokens.
    pub fn with_max_tokens(max_tokens: usize) -> Self {
        Self {
            max_tokens: Some(max_tokens),
            ..Default::default()
        }
    }

    /// Set temperature.
    pub fn temperature(mut self, temp: f32) -> Self {
        self.temperature = Some(temp);
        self
    }

    /// Set tools.
    pub fn tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Set tool choice.
    pub fn tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }
}

/// Chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Response ID.
    pub id: String,

    /// Model used.
    pub model: String,

    /// Response content (may contain structured blocks).
    pub content: MessageContent,

    /// Stop reason.
    pub stop_reason: StopReason,

    /// Token usage.
    pub usage: TokenUsage,

    /// Tool calls requested by the model.
    #[serde(default)]
    pub tool_calls: Vec<ContentBlock>,

    /// Response metadata.
    #[serde(default)]
    pub metadata: HashMap<String, Value>,
}

impl ChatResponse {
    /// Check if the model wants to use tools.
    pub fn has_tool_calls(&self) -> bool {
        self.tool_calls.iter().any(|b| matches!(b, ContentBlock::ToolUse { .. }))
    }

    /// Get the text content of the response.
    pub fn to_text(&self) -> String {
        self.content.to_text()
    }
}

/// Streaming event from a model provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Stream started.
    Start {
        id: String,
        model: String,
    },

    /// Text content delta.
    ContentDelta {
        delta: String,
    },

    /// Thinking content delta (extended thinking).
    ThinkingDelta {
        delta: String,
    },

    /// Tool use started.
    ToolUseStart {
        id: String,
        name: String,
    },

    /// Tool input delta (partial JSON).
    ToolInputDelta {
        id: String,
        delta: String,
    },

    /// Token usage update.
    Usage {
        usage: TokenUsage,
    },

    /// Stream completed.
    End {
        stop_reason: StopReason,
        usage: TokenUsage,
    },

    /// Error occurred.
    Error {
        message: String,
    },
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new() {
        let key = SessionKey::new("test-session");
        let agent = AgentId::new("bot");
        let session = Session::new(key.clone(), agent.clone());

        assert_eq!(session.key, key);
        assert_eq!(session.agent_id, agent);
        assert!(session.messages.is_empty());
        assert!(session.cost.is_none());
        assert!(session.model.is_none());
        assert!(session.thinking_level.is_none());
        assert_eq!(session.type_mode, TypeMode::Typing);
    }

    #[test]
    fn test_session_add_message() {
        let key = SessionKey::new("test-session");
        let agent = AgentId::new("bot");
        let mut session = Session::new(key, agent);
        let original_time = session.last_message_at;

        // Small sleep not needed; just check that adding a message works.
        let msg = Message::user("Hello");
        session.add_message(msg);

        assert_eq!(session.messages.len(), 1);
        // last_message_at should be updated (>= original).
        assert!(session.last_message_at >= original_time);
    }

    #[test]
    fn test_message_user() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, Role::User);
        assert!(msg.name.is_none());
        assert!(msg.tool_use_id.is_none());
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant("Hi there");
        assert_eq!(msg.role, Role::Assistant);
    }

    #[test]
    fn test_message_system() {
        let msg = Message::system("You are helpful.");
        assert_eq!(msg.role, Role::System);
    }

    #[test]
    fn test_message_content_as_text() {
        let text_content = MessageContent::Text("hello".to_string());
        assert_eq!(text_content.as_text(), Some("hello"));

        // Single Text block should also return Some.
        let block_content = MessageContent::Blocks(vec![ContentBlock::Text {
            text: "world".to_string(),
        }]);
        assert_eq!(block_content.as_text(), Some("world"));

        // Multiple blocks should return None.
        let multi = MessageContent::Blocks(vec![
            ContentBlock::Text { text: "a".to_string() },
            ContentBlock::Text { text: "b".to_string() },
        ]);
        assert!(multi.as_text().is_none());

        // Non-text block should return None.
        let thinking = MessageContent::Blocks(vec![ContentBlock::Thinking {
            thinking: "hmm".to_string(),
        }]);
        assert!(thinking.as_text().is_none());
    }

    #[test]
    fn test_message_content_to_text() {
        let text = MessageContent::Text("hello".to_string());
        assert_eq!(text.to_text(), "hello");

        // Blocks: only Text blocks are joined.
        let blocks = MessageContent::Blocks(vec![
            ContentBlock::Text { text: "foo".to_string() },
            ContentBlock::Thinking { thinking: "ignored".to_string() },
            ContentBlock::Text { text: "bar".to_string() },
        ]);
        assert_eq!(blocks.to_text(), "foobar");
    }

    #[test]
    fn test_token_usage_total() {
        let usage = TokenUsage {
            input: 100,
            output: 200,
            cache_creation: 50,
            cache_read: 30,
        };
        assert_eq!(usage.total(), 380);
    }

    #[test]
    fn test_token_usage_add() {
        let mut a = TokenUsage {
            input: 10,
            output: 20,
            cache_creation: 0,
            cache_read: 0,
        };
        let b = TokenUsage {
            input: 5,
            output: 15,
            cache_creation: 3,
            cache_read: 2,
        };
        a.add(&b);
        assert_eq!(a.input, 15);
        assert_eq!(a.output, 35);
        assert_eq!(a.cache_creation, 3);
        assert_eq!(a.cache_read, 2);
    }

    #[test]
    fn test_cost_usage_default() {
        let cost = CostUsage::default();
        assert_eq!(cost.input_usd, 0.0);
        assert_eq!(cost.output_usd, 0.0);
        assert_eq!(cost.total_usd, 0.0);
    }

    #[test]
    fn test_type_mode_serde_roundtrip() {
        let modes = [TypeMode::Typing, TypeMode::Never, TypeMode::Thinking, TypeMode::Message];
        for mode in &modes {
            let json = serde_json::to_string(mode).unwrap();
            let parsed: TypeMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, parsed);
        }
    }

    #[test]
    fn test_session_metadata_default() {
        let meta = SessionMetadata::default();
        assert!(meta.channel.is_none());
        assert!(meta.account_id.is_none());
        assert!(meta.peer_id.is_none());
        assert!(meta.labels.is_empty());
    }

    #[test]
    fn test_message_tool_result() {
        let msg = Message::tool_result("tu_123", "result data", false);
        assert_eq!(msg.role, Role::Tool);
        match &msg.content {
            MessageContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ContentBlock::ToolResult { tool_use_id, content, is_error } => {
                        assert_eq!(tool_use_id, "tu_123");
                        assert_eq!(content, "result data");
                        assert!(!is_error);
                    }
                    _ => panic!("Expected ToolResult block"),
                }
            }
            _ => panic!("Expected Blocks content"),
        }
    }
}
