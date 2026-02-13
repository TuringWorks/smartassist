//! OpenRouter API provider.
//!
//! OpenRouter provides unified access to many AI models through a single API.
//! Supports models from Anthropic, OpenAI, Google, Meta, Mistral, and more.
//! See: https://openrouter.ai/docs

use super::{ModelProvider, ModelResponse, StreamEvent};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use futures::Stream;
use smartassist_core::types::{ContentBlock, Message, MessageContent, Role, TokenUsage, ToolDefinition};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use tracing::debug;

/// OpenRouter API provider.
///
/// Provides access to 100+ models through a unified API.
pub struct OpenRouterProvider {
    /// API key.
    api_key: String,

    /// API base URL.
    base_url: String,

    /// HTTP client.
    client: Client,

    /// Model to use (e.g., "anthropic/claude-3.5-sonnet", "openai/gpt-4o").
    model: String,

    /// Maximum tokens.
    max_tokens: usize,

    /// Temperature (0.0 - 2.0).
    temperature: Option<f32>,

    /// Site URL for rankings (optional).
    site_url: Option<String>,

    /// Site name for rankings (optional).
    site_name: Option<String>,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://openrouter.ai/api/v1".to_string(),
            client: Client::new(),
            model: "anthropic/claude-3.5-sonnet".to_string(),
            max_tokens: 4096,
            temperature: None,
            site_url: None,
            site_name: None,
        }
    }

    /// Set the model (e.g., "anthropic/claude-3.5-sonnet", "openai/gpt-4o", "meta-llama/llama-3.2-90b-instruct").
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the maximum tokens.
    pub fn with_max_tokens(mut self, max_tokens: usize) -> Self {
        self.max_tokens = max_tokens;
        self
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the site URL for OpenRouter rankings.
    pub fn with_site_url(mut self, url: impl Into<String>) -> Self {
        self.site_url = Some(url.into());
        self
    }

    /// Set the site name for OpenRouter rankings.
    pub fn with_site_name(mut self, name: impl Into<String>) -> Self {
        self.site_name = Some(name.into());
        self
    }

    /// Build the API request.
    fn build_request(&self, messages: &[Message], tools: &[ToolDefinition]) -> ApiRequest {
        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .map(|m| self.convert_message(m))
            .collect();

        let api_tools: Option<Vec<ApiTool>> = if tools.is_empty() {
            None
        } else {
            Some(
                tools
                    .iter()
                    .map(|t| ApiTool {
                        tool_type: "function".to_string(),
                        function: ApiFunction {
                            name: t.name.clone(),
                            description: Some(t.description.clone()),
                            parameters: t.input_schema.clone(),
                        },
                    })
                    .collect(),
            )
        };

        ApiRequest {
            model: self.model.clone(),
            messages: api_messages,
            max_tokens: Some(self.max_tokens),
            temperature: self.temperature,
            tools: api_tools,
            stream: false,
        }
    }

    /// Convert a message to API format.
    fn convert_message(&self, message: &Message) -> ApiMessage {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        };

        match &message.content {
            MessageContent::Text(text) => ApiMessage {
                role: role.to_string(),
                content: Some(ApiMessageContent::Text(text.clone())),
                tool_calls: None,
                tool_call_id: message.tool_use_id.clone(),
                name: message.name.clone(),
            },
            MessageContent::Blocks(blocks) => {
                // Check for tool calls
                let tool_calls: Vec<ApiToolCall> = blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::ToolUse { id, name, input } => Some(ApiToolCall {
                            id: id.clone(),
                            call_type: "function".to_string(),
                            function: ApiFunctionCall {
                                name: name.clone(),
                                arguments: serde_json::to_string(input).unwrap_or_default(),
                            },
                        }),
                        _ => None,
                    })
                    .collect();

                // Check for tool results
                let tool_result = blocks.iter().find_map(|block| match block {
                    ContentBlock::ToolResult { content, .. } => Some(content.clone()),
                    _ => None,
                });

                // Get text content
                let text_content: String = blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.clone()),
                        ContentBlock::Thinking { thinking } => {
                            Some(format!("<thinking>{}</thinking>", thinking))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                let content = if let Some(result) = tool_result {
                    Some(ApiMessageContent::Text(result))
                } else if !text_content.is_empty() {
                    Some(ApiMessageContent::Text(text_content))
                } else {
                    None
                };

                ApiMessage {
                    role: role.to_string(),
                    content,
                    tool_calls: if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
                    tool_call_id: message.tool_use_id.clone(),
                    name: message.name.clone(),
                }
            }
        }
    }
}

#[async_trait]
impl ModelProvider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    fn model(&self) -> &str {
        &self.model
    }

    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ModelResponse> {
        let request = self.build_request(messages, tools);

        debug!("Sending request to OpenRouter API");

        let mut req_builder = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        // Add optional OpenRouter-specific headers
        if let Some(ref site_url) = self.site_url {
            req_builder = req_builder.header("HTTP-Referer", site_url);
        }
        if let Some(ref site_name) = self.site_name {
            req_builder = req_builder.header("X-Title", site_name);
        }

        let response = req_builder
            .json(&request)
            .send()
            .await
            .map_err(|e| AgentError::provider(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(AgentError::provider(format!(
                "API error {}: {}",
                status, body
            )));
        }

        let api_response: ApiResponse = response
            .json()
            .await
            .map_err(|e| AgentError::provider(format!("Failed to parse response: {}", e)))?;

        // Get the first choice
        let choice = api_response
            .choices
            .first()
            .ok_or_else(|| AgentError::provider("No response choices"))?;

        // Convert response content
        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        // Add text content
        if let Some(ApiMessageContent::Text(text)) = &choice.message.content {
            if !text.is_empty() {
                content_blocks.push(ContentBlock::Text { text: text.clone() });
            }
        }

        // Add tool calls
        if let Some(tool_calls) = &choice.message.tool_calls {
            for tc in tool_calls {
                let input: serde_json::Value =
                    serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null);
                content_blocks.push(ContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.function.name.clone(),
                    input,
                });
            }
        }

        let stop_reason = choice.finish_reason.clone();

        // Get usage (may be None for some models)
        let usage = api_response.usage.unwrap_or(ApiUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
        });

        Ok(ModelResponse {
            content: MessageContent::Blocks(content_blocks),
            stop_reason,
            token_usage: TokenUsage {
                input: usage.prompt_tokens as u64,
                output: usage.completion_tokens as u64,
                cache_read: 0,
                cache_creation: 0,
            },
        })
    }

    fn complete_stream(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + '_>> {
        // Streaming not yet implemented
        Box::pin(futures::stream::once(async {
            Err(AgentError::provider("Streaming not yet implemented"))
        }))
    }

    fn context_limit(&self) -> usize {
        128_000 // Conservative default; actual varies by routed model
    }
}

// API types (OpenAI-compatible format)

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<ApiMessageContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ApiMessageContent {
    Text(String),
    Parts(Vec<ApiContentPart>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApiContentPart {
    Text { text: String },
    ImageUrl { image_url: ApiImageUrl },
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiImageUrl {
    url: String,
}

#[derive(Debug, Serialize)]
struct ApiTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: ApiFunction,
}

#[derive(Debug, Serialize)]
struct ApiFunction {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiToolCall {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: ApiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiFunctionCall {
    name: String,
    arguments: String,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    choices: Vec<ApiChoice>,
    usage: Option<ApiUsage>,
}

#[derive(Debug, Deserialize)]
struct ApiChoice {
    message: ApiResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiResponseMessage {
    content: Option<ApiMessageContent>,
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ApiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = OpenRouterProvider::new("test-key");
        assert_eq!(provider.name(), "openrouter");
        assert_eq!(provider.model(), "anthropic/claude-3.5-sonnet");
    }

    #[test]
    fn test_builder_pattern() {
        let provider = OpenRouterProvider::new("test-key")
            .with_model("openai/gpt-4o")
            .with_max_tokens(8192)
            .with_temperature(0.7)
            .with_site_url("https://myapp.com")
            .with_site_name("My App");

        assert_eq!(provider.model(), "openai/gpt-4o");
        assert_eq!(provider.max_tokens, 8192);
        assert_eq!(provider.temperature, Some(0.7));
        assert_eq!(provider.site_url, Some("https://myapp.com".to_string()));
        assert_eq!(provider.site_name, Some("My App".to_string()));
    }

    #[test]
    fn test_various_models() {
        // Test that various model formats work
        let models = vec![
            "anthropic/claude-3.5-sonnet",
            "openai/gpt-4o",
            "meta-llama/llama-3.2-90b-instruct",
            "google/gemini-pro-1.5",
            "mistralai/mistral-large",
        ];

        for model in models {
            let provider = OpenRouterProvider::new("key").with_model(model);
            assert_eq!(provider.model(), model);
        }
    }
}
