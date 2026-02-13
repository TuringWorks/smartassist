//! OpenAI GPT provider implementation.
//!
//! This module provides integration with OpenAI's GPT models.

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

/// Default OpenAI API base URL.
const DEFAULT_API_BASE: &str = "https://api.openai.com/v1";

/// OpenAI GPT provider.
pub struct OpenAIProvider {
    /// HTTP client.
    client: Client,

    /// API key.
    api_key: SecretString,

    /// API base URL.
    api_base: String,

    /// Organization ID (optional).
    organization: Option<String>,

    /// Default model to use.
    default_model: String,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider with an API key.
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
            organization: None,
            default_model: "gpt-4o".to_string(),
        })
    }

    /// Create a new provider from environment variable.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .map_err(|_| ProviderError::config("OPENAI_API_KEY environment variable not set"))?;
        Self::new(api_key)
    }

    /// Set the API base URL (for Azure OpenAI or compatible APIs).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.api_base = url.into();
        self
    }

    /// Set the organization ID.
    pub fn with_organization(mut self, org: impl Into<String>) -> Self {
        self.organization = Some(org.into());
        self
    }

    /// Set the default model.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Convert messages to OpenAI format.
    fn convert_messages(&self, messages: &[Message]) -> Result<Vec<OpenAIMessage>> {
        let mut converted = Vec::new();

        for msg in messages {
            let role = match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };

            let content = match &msg.content {
                MessageContent::Text(s) => OpenAIContent::Text(s.clone()),
                MessageContent::Parts(parts) => {
                    let mut openai_parts = Vec::new();
                    for part in parts {
                        match part {
                            crate::ContentPart::Text(s) => {
                                openai_parts.push(OpenAIContentPart::Text { text: s.clone() });
                            }
                            crate::ContentPart::Image(img) => {
                                let url = if img.source_type == crate::ImageSourceType::Base64 {
                                    format!(
                                        "data:{};base64,{}",
                                        img.media_type, img.data
                                    )
                                } else {
                                    img.data.clone()
                                };
                                openai_parts.push(OpenAIContentPart::ImageUrl {
                                    image_url: ImageUrl { url },
                                });
                            }
                            _ => {}
                        }
                    }
                    OpenAIContent::Parts(openai_parts)
                }
            };

            let message = OpenAIMessage {
                role: role.to_string(),
                content: Some(content),
                name: msg.name.clone(),
                tool_call_id: msg.tool_call_id.clone(),
                tool_calls: None,
            };

            converted.push(message);
        }

        Ok(converted)
    }

    /// Convert tools to OpenAI format.
    fn convert_tools(&self, tools: &[crate::ToolDefinition]) -> Vec<OpenAITool> {
        tools
            .iter()
            .map(|t| OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunction {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                },
            })
            .collect()
    }

    /// Parse OpenAI response.
    fn parse_response(&self, response: OpenAIResponse) -> Result<ChatResponse> {
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::internal("No choices in response"))?;

        let content = match choice.message.content {
            Some(OpenAIContent::Text(s)) => s,
            Some(OpenAIContent::Parts(parts)) => {
                parts
                    .into_iter()
                    .filter_map(|p| match p {
                        OpenAIContentPart::Text { text } => Some(text),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("")
            }
            None => String::new(),
        };

        let tool_calls = choice
            .message
            .tool_calls
            .unwrap_or_default()
            .into_iter()
            .map(|tc| ToolUse {
                id: tc.id,
                name: tc.function.name,
                input: serde_json::from_str(&tc.function.arguments).unwrap_or_default(),
            })
            .collect();

        let stop_reason = match choice.finish_reason.as_deref() {
            Some("stop") => StopReason::EndTurn,
            Some("length") => StopReason::MaxTokens,
            Some("tool_calls") => StopReason::ToolUse,
            Some("content_filter") => StopReason::ContentFilter,
            _ => StopReason::Unknown,
        };

        Ok(ChatResponse {
            id: response.id,
            model: response.model,
            content,
            tool_calls,
            stop_reason,
            usage: Usage {
                input_tokens: response.usage.prompt_tokens,
                output_tokens: response.usage.completion_tokens,
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
            metadata: HashMap::new(),
        })
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.api_key.expose_secret())
                .parse()
                .unwrap(),
        );

        if let Some(org) = &self.organization {
            headers.insert("OpenAI-Organization", org.parse().unwrap());
        }

        let response = self
            .client
            .get(format!("{}/models", self.api_base))
            .headers(headers)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ProviderError::server_error(
                response.status().as_u16(),
                "Failed to list models",
            ));
        }

        #[derive(Deserialize)]
        struct ModelsResponse {
            data: Vec<OpenAIModel>,
        }

        #[derive(Deserialize)]
        struct OpenAIModel {
            id: String,
        }

        let result: ModelsResponse = response.json().await?;

        // Filter to chat models and add metadata
        let chat_models: Vec<ModelInfo> = result
            .data
            .into_iter()
            .filter(|m| m.id.starts_with("gpt-") || m.id.starts_with("o1") || m.id.starts_with("o3"))
            .map(|m| {
                let (context_window, max_output) = match m.id.as_str() {
                    "gpt-4o" | "gpt-4o-2024-08-06" => (128_000, 16_384),
                    "gpt-4o-mini" | "gpt-4o-mini-2024-07-18" => (128_000, 16_384),
                    "gpt-4-turbo" | "gpt-4-turbo-2024-04-09" => (128_000, 4_096),
                    "gpt-4" | "gpt-4-0613" => (8_192, 4_096),
                    "gpt-3.5-turbo" => (16_385, 4_096),
                    "o1" | "o1-2024-12-17" => (200_000, 100_000),
                    "o1-mini" | "o1-mini-2024-09-12" => (128_000, 65_536),
                    "o3-mini" | "o3-mini-2025-01-31" => (200_000, 100_000),
                    _ => (128_000, 4_096),
                };

                ModelInfo {
                    id: m.id.clone(),
                    name: m.id.clone(),
                    description: String::new(),
                    context_window,
                    max_output,
                    input_price: 0.0,
                    output_price: 0.0,
                    capabilities: vec!["tools".to_string()],
                }
            })
            .collect();

        Ok(chat_models)
    }

    async fn chat(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<ChatResponse> {
        let options = options.unwrap_or_default();
        let converted_messages = self.convert_messages(messages)?;

        let request = OpenAIRequest {
            model: model.to_string(),
            messages: converted_messages,
            max_tokens: options.max_tokens,
            temperature: options.temperature,
            top_p: options.top_p,
            stop: options.stop,
            tools: options.tools.as_ref().map(|t| self.convert_tools(t)),
            tool_choice: options.tool_choice.as_ref().map(|c| match c {
                crate::ToolChoice::Auto => OpenAIToolChoice::Auto,
                crate::ToolChoice::Any => OpenAIToolChoice::Required,
                crate::ToolChoice::None => OpenAIToolChoice::None,
                crate::ToolChoice::Tool { name } => OpenAIToolChoice::Function {
                    name: name.clone(),
                },
            }),
            stream: false,
            user: options.user,
        };

        debug!("Sending request to OpenAI: model={}", model);

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.api_key.expose_secret())
                .parse()
                .unwrap(),
        );

        if let Some(org) = &self.organization {
            headers.insert("OpenAI-Organization", org.parse().unwrap());
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.api_base))
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body: OpenAIError = response.json().await.unwrap_or_else(|_| OpenAIError {
                error: OpenAIErrorDetail {
                    message: "Unknown error".to_string(),
                    error_type: "unknown".to_string(),
                    code: None,
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

        let response: OpenAIResponse = response.json().await?;
        self.parse_response(response)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<CompletionStream> {
        let options = options.unwrap_or_default();
        let converted_messages = self.convert_messages(messages)?;

        let request = OpenAIRequest {
            model: model.to_string(),
            messages: converted_messages,
            max_tokens: options.max_tokens,
            temperature: options.temperature,
            top_p: options.top_p,
            stop: options.stop,
            tools: options.tools.as_ref().map(|t| self.convert_tools(t)),
            tool_choice: options.tool_choice.as_ref().map(|c| match c {
                crate::ToolChoice::Auto => OpenAIToolChoice::Auto,
                crate::ToolChoice::Any => OpenAIToolChoice::Required,
                crate::ToolChoice::None => OpenAIToolChoice::None,
                crate::ToolChoice::Tool { name } => OpenAIToolChoice::Function {
                    name: name.clone(),
                },
            }),
            stream: true,
            user: options.user,
        };

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.api_key.expose_secret())
                .parse()
                .unwrap(),
        );

        if let Some(org) = &self.organization {
            headers.insert("OpenAI-Organization", org.parse().unwrap());
        }

        let response = self
            .client
            .post(format!("{}/chat/completions", self.api_base))
            .headers(headers)
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body: OpenAIError = response.json().await.unwrap_or_else(|_| OpenAIError {
                error: OpenAIErrorDetail {
                    message: "Unknown error".to_string(),
                    error_type: "unknown".to_string(),
                    code: None,
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

        let byte_stream = response.bytes_stream();
        let event_stream = byte_stream.eventsource();

        let stream = event_stream.filter_map(move |result| {
            async move {
                match result {
                    Ok(event) => {
                        if event.data.is_empty() || event.data == "[DONE]" {
                            return None;
                        }

                        let parsed: std::result::Result<OpenAIStreamChunk, _> =
                            serde_json::from_str(&event.data);

                        match parsed {
                            Ok(chunk) => {
                                if let Some(choice) = chunk.choices.into_iter().next() {
                                    if let Some(content) = choice.delta.content {
                                        return Some(Ok(StreamEvent::ContentDelta {
                                            delta: content,
                                        }));
                                    }

                                    if let Some(finish_reason) = choice.finish_reason {
                                        let stop_reason = match finish_reason.as_str() {
                                            "stop" => StopReason::EndTurn,
                                            "length" => StopReason::MaxTokens,
                                            "tool_calls" => StopReason::ToolUse,
                                            "content_filter" => StopReason::ContentFilter,
                                            _ => StopReason::Unknown,
                                        };

                                        return Some(Ok(StreamEvent::End {
                                            stop_reason,
                                            usage: Usage::default(),
                                        }));
                                    }
                                }
                                None
                            }
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

    async fn count_tokens(&self, _model: &str, messages: &[Message]) -> Result<TokenCount> {
        // OpenAI doesn't have a public token counting endpoint
        // We estimate based on characters (~4 chars per token)
        let total_chars: usize = messages
            .iter()
            .filter_map(|m| m.text())
            .map(|t| t.len())
            .sum();

        Ok(TokenCount {
            count: total_chars / 4,
            model: _model.to_string(),
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tools: true,
            vision: true,
            system_messages: true,
            max_context: Some(128_000),
            max_output: Some(16_384),
        }
    }
}

// Internal types for OpenAI API

#[derive(Serialize)]
struct OpenAIRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<OpenAIToolChoice>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<OpenAIContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum OpenAIContent {
    Text(String),
    Parts(Vec<OpenAIContentPart>),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OpenAIContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Serialize, Deserialize)]
struct ImageUrl {
    url: String,
}

#[derive(Serialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAIFunction,
}

#[derive(Serialize)]
struct OpenAIFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum OpenAIToolChoice {
    Auto,
    Required,
    None,
    Function { name: String },
}

#[derive(Deserialize)]
struct OpenAIResponse {
    id: String,
    model: String,
    choices: Vec<OpenAIChoice>,
    usage: OpenAIUsage,
}

#[derive(Deserialize)]
struct OpenAIChoice {
    message: OpenAIMessage,
    finish_reason: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct OpenAIToolCall {
    id: String,
    function: OpenAIFunctionCall,
}

#[derive(Serialize, Deserialize)]
struct OpenAIFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Deserialize)]
struct OpenAIUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

#[derive(Deserialize)]
struct OpenAIError {
    error: OpenAIErrorDetail,
}

#[derive(Deserialize)]
struct OpenAIErrorDetail {
    message: String,
    #[serde(rename = "type")]
    error_type: String,
    code: Option<String>,
}

// Streaming types

#[derive(Deserialize)]
struct OpenAIStreamChunk {
    choices: Vec<OpenAIStreamChoice>,
}

#[derive(Deserialize)]
struct OpenAIStreamChoice {
    delta: OpenAIStreamDelta,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct OpenAIStreamDelta {
    content: Option<String>,
    tool_calls: Option<Vec<OpenAIToolCallDelta>>,
}

#[derive(Deserialize)]
struct OpenAIToolCallDelta {
    index: usize,
    id: Option<String>,
    function: Option<OpenAIFunctionDelta>,
}

#[derive(Deserialize)]
struct OpenAIFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = OpenAIProvider::new("test-key").unwrap();
        assert_eq!(provider.name(), "openai");
    }

    #[test]
    fn test_provider_empty_key() {
        let result = OpenAIProvider::new("");
        assert!(result.is_err());
    }

    #[test]
    fn test_capabilities() {
        let provider = OpenAIProvider::new("test-key").unwrap();
        let caps = provider.capabilities();

        assert!(caps.streaming);
        assert!(caps.tools);
        assert!(caps.vision);
    }
}
