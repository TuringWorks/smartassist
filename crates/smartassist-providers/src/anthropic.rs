//! Anthropic Claude provider implementation.
//!
//! This module provides integration with Anthropic's Claude models.
//!
//! # Example
//!
//! ```rust,ignore
//! use smartassist_providers::{AnthropicProvider, Message, ChatOptions};
//!
//! let provider = AnthropicProvider::new("your-api-key")?;
//! let response = provider.chat(
//!     "claude-sonnet-4-20250514",
//!     &[Message::user("Hello!")],
//!     None,
//! ).await?;
//! ```

use crate::{
    ChatOptions, ChatResponse, CompletionStream, Message, MessageContent, MessageRole, ModelInfo,
    Provider, ProviderCapabilities, ProviderError, Result, StopReason, StreamEvent, TokenCount,
    ToolUse, Usage,
};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Default Anthropic API base URL.
const DEFAULT_API_BASE: &str = "https://api.anthropic.com";

/// Current API version.
const API_VERSION: &str = "2023-06-01";

/// Anthropic Claude provider.
pub struct AnthropicProvider {
    /// HTTP client.
    client: Client,

    /// API key.
    api_key: SecretString,

    /// API base URL.
    api_base: String,

    /// Default model to use.
    default_model: String,

    /// Request timeout in seconds.
    timeout: u64,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider with an API key.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let api_key = api_key.into();
        if api_key.is_empty() {
            return Err(ProviderError::config("API key is required"));
        }

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .map_err(|e| ProviderError::config(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            api_key: SecretString::new(api_key),
            api_base: DEFAULT_API_BASE.to_string(),
            default_model: "claude-sonnet-4-20250514".to_string(),
            timeout: 300,
        })
    }

    /// Create a new provider from environment variable.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| ProviderError::config("ANTHROPIC_API_KEY environment variable not set"))?;
        Self::new(api_key)
    }

    /// Set the API base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.api_base = url.into();
        self
    }

    /// Set the default model.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Set the request timeout.
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout = seconds;
        self
    }

    /// Convert messages to Anthropic format.
    fn convert_messages(
        &self,
        messages: &[Message],
    ) -> Result<(Option<String>, Vec<AnthropicMessage>)> {
        let mut system = None;
        let mut converted = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // Anthropic handles system message separately
                    if let Some(text) = msg.text() {
                        system = Some(text.to_string());
                    }
                }
                MessageRole::User => {
                    converted.push(AnthropicMessage {
                        role: "user".to_string(),
                        content: self.convert_content(&msg.content)?,
                    });
                }
                MessageRole::Assistant => {
                    converted.push(AnthropicMessage {
                        role: "assistant".to_string(),
                        content: self.convert_content(&msg.content)?,
                    });
                }
                MessageRole::Tool => {
                    // Tool results are handled as user messages with tool_result content
                    if let Some(tool_call_id) = &msg.tool_call_id {
                        converted.push(AnthropicMessage {
                            role: "user".to_string(),
                            content: AnthropicContent::Parts(vec![AnthropicContentPart::ToolResult {
                                tool_use_id: tool_call_id.clone(),
                                content: msg.text().unwrap_or("").to_string(),
                            }]),
                        });
                    }
                }
            }
        }

        Ok((system, converted))
    }

    /// Convert content to Anthropic format.
    fn convert_content(&self, content: &MessageContent) -> Result<AnthropicContent> {
        match content {
            MessageContent::Text(s) => Ok(AnthropicContent::Text(s.clone())),
            MessageContent::Parts(parts) => {
                let mut converted = Vec::new();
                for part in parts {
                    match part {
                        crate::ContentPart::Text(s) => {
                            converted.push(AnthropicContentPart::Text { text: s.clone() });
                        }
                        crate::ContentPart::Image(img) => {
                            converted.push(AnthropicContentPart::Image {
                                source: ImageSource {
                                    source_type: match img.source_type {
                                        crate::ImageSourceType::Base64 => "base64".to_string(),
                                        crate::ImageSourceType::Url => "url".to_string(),
                                    },
                                    media_type: img.media_type.clone(),
                                    data: img.data.clone(),
                                },
                            });
                        }
                        crate::ContentPart::ToolUse(tool) => {
                            converted.push(AnthropicContentPart::ToolUse {
                                id: tool.id.clone(),
                                name: tool.name.clone(),
                                input: tool.input.clone(),
                            });
                        }
                        crate::ContentPart::ToolResult(result) => {
                            converted.push(AnthropicContentPart::ToolResult {
                                tool_use_id: result.tool_use_id.clone(),
                                content: result.content.clone(),
                            });
                        }
                    }
                }
                Ok(AnthropicContent::Parts(converted))
            }
        }
    }

    /// Convert tools to Anthropic format.
    fn convert_tools(&self, tools: &[crate::ToolDefinition]) -> Vec<AnthropicTool> {
        tools
            .iter()
            .map(|t| AnthropicTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect()
    }

    /// Parse Anthropic response.
    fn parse_response(&self, response: AnthropicResponse) -> ChatResponse {
        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for block in &response.content {
            match block {
                AnthropicContentBlock::Text { text } => {
                    content.push_str(text);
                }
                AnthropicContentBlock::ToolUse { id, name, input } => {
                    tool_calls.push(ToolUse {
                        id: id.clone(),
                        name: name.clone(),
                        input: input.clone(),
                    });
                }
            }
        }

        let stop_reason = match response.stop_reason.as_deref() {
            Some("end_turn") => StopReason::EndTurn,
            Some("stop_sequence") => StopReason::StopSequence,
            Some("max_tokens") => StopReason::MaxTokens,
            Some("tool_use") => StopReason::ToolUse,
            _ => StopReason::Unknown,
        };

        ChatResponse {
            id: response.id,
            model: response.model,
            content,
            tool_calls,
            stop_reason,
            usage: Usage {
                input_tokens: response.usage.input_tokens,
                output_tokens: response.usage.output_tokens,
                cache_read_tokens: response.usage.cache_read_input_tokens.unwrap_or(0),
                cache_creation_tokens: response.usage.cache_creation_input_tokens.unwrap_or(0),
            },
            metadata: HashMap::new(),
        }
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        // Anthropic doesn't have a list models endpoint, so we return known models
        Ok(vec![
            ModelInfo {
                id: "claude-opus-4-20250514".to_string(),
                name: "Claude Opus 4".to_string(),
                description: "Most capable model for complex tasks".to_string(),
                context_window: 200_000,
                max_output: 32_000,
                input_price: 15.0,
                output_price: 75.0,
                capabilities: vec![
                    "vision".to_string(),
                    "tools".to_string(),
                    "computer_use".to_string(),
                ],
            },
            ModelInfo {
                id: "claude-sonnet-4-20250514".to_string(),
                name: "Claude Sonnet 4".to_string(),
                description: "Best balance of performance and speed".to_string(),
                context_window: 200_000,
                max_output: 64_000,
                input_price: 3.0,
                output_price: 15.0,
                capabilities: vec![
                    "vision".to_string(),
                    "tools".to_string(),
                    "computer_use".to_string(),
                ],
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".to_string(),
                name: "Claude 3.5 Haiku".to_string(),
                description: "Fastest model for simple tasks".to_string(),
                context_window: 200_000,
                max_output: 8192,
                input_price: 0.80,
                output_price: 4.0,
                capabilities: vec!["vision".to_string(), "tools".to_string()],
            },
        ])
    }

    async fn chat(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<ChatResponse> {
        let options = options.unwrap_or_default();
        let (system, converted_messages) = self.convert_messages(messages)?;

        let request = AnthropicRequest {
            model: model.to_string(),
            messages: converted_messages,
            max_tokens: options.max_tokens.unwrap_or(4096),
            system,
            temperature: options.temperature,
            top_p: options.top_p,
            top_k: options.top_k,
            stop_sequences: options.stop,
            tools: options.tools.as_ref().map(|t| self.convert_tools(t)),
            tool_choice: options.tool_choice.as_ref().map(|c| match c {
                crate::ToolChoice::Auto => AnthropicToolChoice::Auto,
                crate::ToolChoice::Any => AnthropicToolChoice::Any,
                crate::ToolChoice::None => AnthropicToolChoice::None,
                crate::ToolChoice::Tool { name } => AnthropicToolChoice::Tool {
                    name: name.clone(),
                },
            }),
            stream: false,
        };

        debug!("Sending request to Anthropic: model={}", model);

        let response = self
            .client
            .post(format!("{}/v1/messages", self.api_base))
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body: AnthropicError = response.json().await.unwrap_or_else(|_| AnthropicError {
                error: AnthropicErrorDetail {
                    error_type: "unknown".to_string(),
                    message: "Unknown error".to_string(),
                },
            });

            return match status.as_u16() {
                401 => Err(ProviderError::auth(error_body.error.message)),
                429 => Err(ProviderError::rate_limit(error_body.error.message, None)),
                400 => Err(ProviderError::invalid_request(error_body.error.message)),
                _ => Err(ProviderError::server_error(
                    status.as_u16(),
                    error_body.error.message,
                )),
            };
        }

        let response: AnthropicResponse = response.json().await?;
        Ok(self.parse_response(response))
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<CompletionStream> {
        let options = options.unwrap_or_default();
        let (system, converted_messages) = self.convert_messages(messages)?;

        let request = AnthropicRequest {
            model: model.to_string(),
            messages: converted_messages,
            max_tokens: options.max_tokens.unwrap_or(4096),
            system,
            temperature: options.temperature,
            top_p: options.top_p,
            top_k: options.top_k,
            stop_sequences: options.stop,
            tools: options.tools.as_ref().map(|t| self.convert_tools(t)),
            tool_choice: options.tool_choice.as_ref().map(|c| match c {
                crate::ToolChoice::Auto => AnthropicToolChoice::Auto,
                crate::ToolChoice::Any => AnthropicToolChoice::Any,
                crate::ToolChoice::None => AnthropicToolChoice::None,
                crate::ToolChoice::Tool { name } => AnthropicToolChoice::Tool {
                    name: name.clone(),
                },
            }),
            stream: true,
        };

        let response = self
            .client
            .post(format!("{}/v1/messages", self.api_base))
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", API_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body: AnthropicError = response.json().await.unwrap_or_else(|_| AnthropicError {
                error: AnthropicErrorDetail {
                    error_type: "unknown".to_string(),
                    message: "Unknown error".to_string(),
                },
            });

            return match status.as_u16() {
                401 => Err(ProviderError::auth(error_body.error.message)),
                429 => Err(ProviderError::rate_limit(error_body.error.message, None)),
                400 => Err(ProviderError::invalid_request(error_body.error.message)),
                _ => Err(ProviderError::server_error(
                    status.as_u16(),
                    error_body.error.message,
                )),
            };
        }

        let model = model.to_string();
        let byte_stream = response.bytes_stream();

        let event_stream = byte_stream.eventsource();

        let stream = event_stream.filter_map(move |result| {
            let model = model.clone();
            async move {
                match result {
                    Ok(event) => {
                        if event.data.is_empty() || event.data == "[DONE]" {
                            return None;
                        }

                        let parsed: std::result::Result<AnthropicStreamEvent, _> =
                            serde_json::from_str(&event.data);

                        match parsed {
                            Ok(sse) => match sse {
                                AnthropicStreamEvent::MessageStart { message } => {
                                    Some(Ok(StreamEvent::Start {
                                        id: message.id,
                                        model: model.clone(),
                                    }))
                                }
                                AnthropicStreamEvent::ContentBlockDelta { delta, .. } => {
                                    if let Some(text) = delta.text {
                                        Some(Ok(StreamEvent::ContentDelta { delta: text }))
                                    } else {
                                        None
                                    }
                                }
                                AnthropicStreamEvent::ContentBlockStart { content_block, .. } => {
                                    if let AnthropicContentBlock::ToolUse { id, name, .. } =
                                        content_block
                                    {
                                        Some(Ok(StreamEvent::ToolUseStart { id, name }))
                                    } else {
                                        None
                                    }
                                }
                                AnthropicStreamEvent::MessageDelta { delta, usage } => {
                                    let stop_reason = match delta.stop_reason.as_deref() {
                                        Some("end_turn") => StopReason::EndTurn,
                                        Some("stop_sequence") => StopReason::StopSequence,
                                        Some("max_tokens") => StopReason::MaxTokens,
                                        Some("tool_use") => StopReason::ToolUse,
                                        _ => StopReason::Unknown,
                                    };

                                    Some(Ok(StreamEvent::End {
                                        stop_reason,
                                        usage: Usage {
                                            input_tokens: 0,
                                            output_tokens: usage.output_tokens,
                                            cache_read_tokens: 0,
                                            cache_creation_tokens: 0,
                                        },
                                    }))
                                }
                                _ => None,
                            },
                            Err(e) => {
                                warn!("Failed to parse SSE event: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => Some(Err(ProviderError::stream(e.to_string()))),
                }
            }
        });

        Ok(Box::pin(stream))
    }

    async fn count_tokens(&self, model: &str, messages: &[Message]) -> Result<TokenCount> {
        let (system, converted_messages) = self.convert_messages(messages)?;

        let request = serde_json::json!({
            "model": model,
            "messages": converted_messages,
            "system": system,
        });

        let response = self
            .client
            .post(format!("{}/v1/messages/count_tokens", self.api_base))
            .header("x-api-key", self.api_key.expose_secret())
            .header("anthropic-version", API_VERSION)
            .header("anthropic-beta", "token-counting-2024-11-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ProviderError::server_error(
                response.status().as_u16(),
                "Failed to count tokens",
            ));
        }

        #[derive(Deserialize)]
        struct TokenCountResponse {
            input_tokens: usize,
        }

        let result: TokenCountResponse = response.json().await?;
        Ok(TokenCount {
            count: result.input_tokens,
            model: model.to_string(),
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tools: true,
            vision: true,
            system_messages: true,
            max_context: Some(200_000),
            max_output: Some(64_000),
        }
    }
}

// Internal types for Anthropic API

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    messages: Vec<AnthropicMessage>,
    max_tokens: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<AnthropicToolChoice>,
    stream: bool,
}

#[derive(Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContent,
}

#[derive(Serialize)]
#[serde(untagged)]
enum AnthropicContent {
    Text(String),
    Parts(Vec<AnthropicContentPart>),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentPart {
    Text { text: String },
    Image { source: ImageSource },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String },
}

#[derive(Serialize)]
struct ImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicToolChoice {
    Auto,
    Any,
    None,
    Tool { name: String },
}

#[derive(Deserialize)]
struct AnthropicResponse {
    id: String,
    model: String,
    content: Vec<AnthropicContentBlock>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
}

#[derive(Deserialize)]
struct AnthropicUsage {
    input_tokens: usize,
    output_tokens: usize,
    cache_read_input_tokens: Option<usize>,
    cache_creation_input_tokens: Option<usize>,
}

#[derive(Deserialize)]
struct AnthropicError {
    error: AnthropicErrorDetail,
}

#[derive(Deserialize)]
struct AnthropicErrorDetail {
    #[serde(rename = "type")]
    error_type: String,
    message: String,
}

// SSE types

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicStreamEvent {
    MessageStart {
        message: AnthropicStreamMessage,
    },
    ContentBlockStart {
        index: usize,
        content_block: AnthropicContentBlock,
    },
    ContentBlockDelta {
        index: usize,
        delta: ContentDelta,
    },
    ContentBlockStop {
        index: usize,
    },
    MessageDelta {
        delta: MessageDelta,
        usage: StreamUsage,
    },
    MessageStop,
    Ping,
    Error {
        error: AnthropicErrorDetail,
    },
}

#[derive(Deserialize)]
struct AnthropicStreamMessage {
    id: String,
}

#[derive(Deserialize)]
struct ContentDelta {
    text: Option<String>,
    partial_json: Option<String>,
}

#[derive(Deserialize)]
struct MessageDelta {
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct StreamUsage {
    output_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = AnthropicProvider::new("test-key").unwrap();
        assert_eq!(provider.name(), "anthropic");
    }

    #[test]
    fn test_provider_empty_key() {
        let result = AnthropicProvider::new("");
        assert!(result.is_err());
    }

    #[test]
    fn test_capabilities() {
        let provider = AnthropicProvider::new("test-key").unwrap();
        let caps = provider.capabilities();

        assert!(caps.streaming);
        assert!(caps.tools);
        assert!(caps.vision);
        assert_eq!(caps.max_context, Some(200_000));
    }

    #[tokio::test]
    async fn test_list_models() {
        let provider = AnthropicProvider::new("test-key").unwrap();
        let models = provider.list_models().await.unwrap();

        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id.contains("claude")));
    }
}
