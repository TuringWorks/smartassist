//! Common types for model providers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Message role in a conversation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message (instructions).
    System,
    /// User message.
    User,
    /// Assistant message.
    Assistant,
    /// Tool result message.
    Tool,
}

impl MessageRole {
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
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message role.
    pub role: MessageRole,

    /// Message content.
    pub content: MessageContent,

    /// Optional name for the message sender.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Tool call ID (for tool results).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_call_id: None,
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_call_id: None,
        }
    }

    /// Create a tool result message.
    pub fn tool_result(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: MessageContent::Text(content.into()),
            name: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }

    /// Create a message with image content.
    pub fn with_image(role: MessageRole, text: impl Into<String>, image: ImageContent) -> Self {
        Self {
            role,
            content: MessageContent::Parts(vec![
                ContentPart::Text(text.into()),
                ContentPart::Image(image),
            ]),
            name: None,
            tool_call_id: None,
        }
    }

    /// Get the text content of the message.
    pub fn text(&self) -> Option<&str> {
        match &self.content {
            MessageContent::Text(s) => Some(s),
            MessageContent::Parts(parts) => {
                for part in parts {
                    if let ContentPart::Text(s) = part {
                        return Some(s);
                    }
                }
                None
            }
        }
    }
}

/// Message content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Simple text content.
    Text(String),

    /// Multi-part content (text + images, etc.).
    Parts(Vec<ContentPart>),
}

/// A part of multi-modal content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Text content.
    Text(String),

    /// Image content.
    Image(ImageContent),

    /// Tool use request.
    ToolUse(ToolUse),

    /// Tool result.
    ToolResult(ToolResultContent),
}

/// Image content for vision models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageContent {
    /// Image source type.
    #[serde(rename = "type")]
    pub source_type: ImageSourceType,

    /// Media type (e.g., "image/jpeg").
    pub media_type: String,

    /// Image data (base64 for base64, URL for url).
    pub data: String,
}

impl ImageContent {
    /// Create an image from base64 data.
    pub fn base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self {
            source_type: ImageSourceType::Base64,
            media_type: media_type.into(),
            data: data.into(),
        }
    }

    /// Create an image from a URL.
    pub fn url(url: impl Into<String>) -> Self {
        Self {
            source_type: ImageSourceType::Url,
            media_type: String::new(),
            data: url.into(),
        }
    }
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

/// Tool use request from the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUse {
    /// Unique ID for this tool use.
    pub id: String,

    /// Tool name.
    pub name: String,

    /// Tool arguments as JSON.
    pub input: serde_json::Value,
}

/// Tool result content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResultContent {
    /// Tool call ID this result is for.
    pub tool_use_id: String,

    /// Tool result content.
    pub content: String,

    /// Whether the tool execution failed.
    #[serde(default)]
    pub is_error: bool,
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
    pub extra: HashMap<String, serde_json::Value>,
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

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name.
    pub name: String,

    /// Tool description.
    pub description: String,

    /// Input schema (JSON Schema).
    pub input_schema: serde_json::Value,
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

/// Chat completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Response ID.
    pub id: String,

    /// Model used.
    pub model: String,

    /// Response content.
    pub content: String,

    /// Tool calls requested by the model.
    #[serde(default)]
    pub tool_calls: Vec<ToolUse>,

    /// Stop reason.
    pub stop_reason: StopReason,

    /// Token usage.
    pub usage: Usage,

    /// Response metadata.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ChatResponse {
    /// Check if the model wants to use tools.
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
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

/// Token usage information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Input/prompt tokens.
    pub input_tokens: usize,

    /// Output/completion tokens.
    pub output_tokens: usize,

    /// Cache read tokens (if caching is used).
    #[serde(default)]
    pub cache_read_tokens: usize,

    /// Cache creation tokens (if caching is used).
    #[serde(default)]
    pub cache_creation_tokens: usize,
}

impl Usage {
    /// Get total tokens used.
    pub fn total_tokens(&self) -> usize {
        self.input_tokens + self.output_tokens
    }
}

/// Token count result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCount {
    /// Number of tokens.
    pub count: usize,

    /// Model used for counting.
    pub model: String,
}

/// Model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model ID.
    pub id: String,

    /// Display name.
    pub name: String,

    /// Model description.
    #[serde(default)]
    pub description: String,

    /// Maximum context window.
    pub context_window: usize,

    /// Maximum output tokens.
    pub max_output: usize,

    /// Input price per million tokens.
    #[serde(default)]
    pub input_price: f64,

    /// Output price per million tokens.
    #[serde(default)]
    pub output_price: f64,

    /// Model capabilities.
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Streaming event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Stream started.
    Start {
        id: String,
        model: String,
    },

    /// Text delta.
    ContentDelta {
        delta: String,
    },

    /// Tool use started.
    ToolUseStart {
        id: String,
        name: String,
    },

    /// Tool input delta (partial JSON).
    ToolInputDelta {
        delta: String,
    },

    /// Stream completed.
    End {
        stop_reason: StopReason,
        usage: Usage,
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
    fn test_message_creation() {
        let sys = Message::system("You are a helpful assistant.");
        assert!(sys.role.is_system());
        assert_eq!(sys.text(), Some("You are a helpful assistant."));

        let user = Message::user("Hello!");
        assert!(user.role.is_user());
        assert_eq!(user.text(), Some("Hello!"));

        let assistant = Message::assistant("Hi there!");
        assert!(assistant.role.is_assistant());
    }

    #[test]
    fn test_chat_options() {
        let opts = ChatOptions::with_max_tokens(1000)
            .temperature(0.7)
            .tool_choice(ToolChoice::Auto);

        assert_eq!(opts.max_tokens, Some(1000));
        assert_eq!(opts.temperature, Some(0.7));
        assert!(matches!(opts.tool_choice, Some(ToolChoice::Auto)));
    }

    #[test]
    fn test_usage() {
        let usage = Usage {
            input_tokens: 100,
            output_tokens: 50,
            cache_read_tokens: 0,
            cache_creation_tokens: 0,
        };

        assert_eq!(usage.total_tokens(), 150);
    }

    #[test]
    fn test_image_content() {
        let img = ImageContent::base64("image/jpeg", "abc123");
        assert_eq!(img.source_type, ImageSourceType::Base64);
        assert_eq!(img.media_type, "image/jpeg");

        let url_img = ImageContent::url("https://example.com/img.png");
        assert_eq!(url_img.source_type, ImageSourceType::Url);
    }
}
