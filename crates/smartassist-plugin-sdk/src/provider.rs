//! Model provider plugin support.
//!
//! This module provides traits for plugins that add AI model providers.

use crate::{PluginContext, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

/// Model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model ID.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Model description.
    pub description: Option<String>,
    /// Context window size.
    pub context_window: Option<u32>,
    /// Max output tokens.
    pub max_output_tokens: Option<u32>,
    /// Whether the model supports vision.
    pub vision: bool,
    /// Whether the model supports tools.
    pub tool_use: bool,
}

/// Model configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model ID.
    pub model: String,
    /// Temperature.
    pub temperature: Option<f32>,
    /// Top-p.
    pub top_p: Option<f32>,
    /// Max tokens.
    pub max_tokens: Option<u32>,
    /// Stop sequences.
    pub stop_sequences: Option<Vec<String>>,
}

/// Provider configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API key.
    pub api_key: Option<String>,
    /// Base URL.
    pub base_url: Option<String>,
    /// Organization ID.
    pub organization_id: Option<String>,
    /// Additional options.
    pub options: HashMap<String, serde_json::Value>,
}

/// Token usage information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Input tokens.
    pub input_tokens: u32,
    /// Output tokens.
    pub output_tokens: u32,
    /// Cache creation tokens.
    pub cache_creation_input_tokens: Option<u32>,
    /// Cache read tokens.
    pub cache_read_input_tokens: Option<u32>,
}

/// Capabilities of a model provider.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    /// Whether the provider supports streaming responses.
    pub streaming: bool,

    /// Whether the provider supports tool/function calling.
    pub tool_use: bool,

    /// Whether the provider supports vision (image inputs).
    pub vision: bool,

    /// Whether the provider supports extended thinking.
    pub extended_thinking: bool,

    /// Whether the provider supports system prompts.
    pub system_prompt: bool,

    /// Maximum context window size.
    pub max_context_tokens: Option<u32>,

    /// Maximum output tokens.
    pub max_output_tokens: Option<u32>,

    /// Supported modalities.
    pub modalities: Vec<Modality>,
}

/// Input/output modalities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    /// Text input/output.
    Text,
    /// Image input.
    Image,
    /// Audio input/output.
    Audio,
    /// Video input.
    Video,
    /// Document input (PDF, etc.).
    Document,
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message role.
    pub role: MessageRole,

    /// Message content.
    pub content: MessageContent,

    /// Tool call ID (for tool results).
    pub tool_call_id: Option<String>,
}

/// Message role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message.
    System,
    /// User message.
    User,
    /// Assistant message.
    Assistant,
    /// Tool result message.
    Tool,
}

/// Message content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Text content.
    Text(String),
    /// Multi-part content.
    Parts(Vec<ContentPart>),
}

/// A part of multi-part content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Text part.
    Text { text: String },
    /// Image part.
    Image {
        #[serde(flatten)]
        source: ImageSource,
    },
    /// Tool use part.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Tool result part.
    ToolResult {
        tool_use_id: String,
        content: String,
        is_error: Option<bool>,
    },
}

/// Image source.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64 encoded image.
    Base64 {
        media_type: String,
        data: String,
    },
    /// URL to image.
    Url {
        url: String,
    },
}

/// Request to generate a response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    /// Messages in the conversation.
    pub messages: Vec<Message>,

    /// Model to use.
    pub model: String,

    /// System prompt.
    pub system: Option<String>,

    /// Maximum tokens to generate.
    pub max_tokens: Option<u32>,

    /// Temperature for sampling.
    pub temperature: Option<f32>,

    /// Top-p for nucleus sampling.
    pub top_p: Option<f32>,

    /// Stop sequences.
    pub stop_sequences: Option<Vec<String>>,

    /// Tool definitions.
    pub tools: Option<Vec<smartassist_core::types::ToolDefinition>>,

    /// Whether to stream the response.
    pub stream: bool,

    /// Additional provider-specific options.
    pub options: HashMap<String, serde_json::Value>,
}

/// Response from generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateResponse {
    /// Response ID.
    pub id: String,

    /// Model used.
    pub model: String,

    /// Generated content.
    pub content: Vec<ContentPart>,

    /// Stop reason.
    pub stop_reason: Option<StopReason>,

    /// Token usage.
    pub usage: Option<Usage>,
}

/// Reason the generation stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Reached end of response.
    EndTurn,
    /// Hit max tokens limit.
    MaxTokens,
    /// Hit a stop sequence.
    StopSequence,
    /// Tool use requested.
    ToolUse,
}

/// A streaming event from generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// Message started.
    MessageStart {
        id: String,
        model: String,
    },
    /// Content block started.
    ContentBlockStart {
        index: usize,
        content_type: String,
    },
    /// Text delta.
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    /// Content block stopped.
    ContentBlockStop {
        index: usize,
    },
    /// Message completed.
    MessageDelta {
        stop_reason: Option<StopReason>,
        usage: Option<Usage>,
    },
    /// Message stopped.
    MessageStop,
    /// Error occurred.
    Error {
        message: String,
    },
}

/// Delta content for streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentDelta {
    /// Text delta.
    TextDelta { text: String },
    /// Input JSON delta (for tool use).
    InputJsonDelta { partial_json: String },
}

/// Trait for model provider plugins.
#[async_trait]
pub trait ModelProviderPlugin: Send + Sync {
    /// Get the provider identifier.
    fn provider_id(&self) -> &str;

    /// Get the provider display name.
    fn display_name(&self) -> &str;

    /// Get provider capabilities.
    fn capabilities(&self) -> ProviderCapabilities;

    /// List available models.
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    /// Get information about a specific model.
    async fn get_model(&self, model_id: &str) -> Result<Option<ModelInfo>>;

    /// Generate a response.
    async fn generate(&self, request: GenerateRequest) -> Result<GenerateResponse>;

    /// Generate a streaming response.
    async fn generate_stream(
        &self,
        request: GenerateRequest,
    ) -> Result<mpsc::Receiver<StreamEvent>>;

    /// Configure the provider.
    async fn configure(&mut self, config: ProviderConfig, ctx: &PluginContext) -> Result<()>;

    /// Check if the provider is configured and ready.
    fn is_ready(&self) -> bool;

    /// Get the current provider configuration.
    fn config(&self) -> Option<&ProviderConfig>;
}

/// Factory for creating model provider instances.
#[async_trait]
pub trait ProviderPluginFactory: Send + Sync {
    /// Create a new provider instance.
    async fn create(
        &self,
        config: ProviderConfig,
        ctx: &PluginContext,
    ) -> Result<Arc<dyn ModelProviderPlugin>>;

    /// Get default model configuration.
    fn default_model_config(&self) -> ModelConfig {
        ModelConfig::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_capabilities() {
        let caps = ProviderCapabilities {
            streaming: true,
            tool_use: true,
            vision: true,
            extended_thinking: false,
            system_prompt: true,
            max_context_tokens: Some(200000),
            max_output_tokens: Some(8192),
            modalities: vec![Modality::Text, Modality::Image],
        };

        assert!(caps.streaming);
        assert!(caps.tool_use);
        assert!(caps.vision);
        assert!(!caps.extended_thinking);
        assert_eq!(caps.max_context_tokens, Some(200000));
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message {
            role: MessageRole::User,
            content: MessageContent::Text("Hello".to_string()),
            tool_call_id: None,
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_stream_event_serialization() {
        let event = StreamEvent::ContentBlockDelta {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: "Hello".to_string(),
            },
        };

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"content_block_delta\""));
        assert!(json.contains("\"text\":\"Hello\""));
    }
}
