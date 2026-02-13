//! OpenAI API provider.
//!
//! Supports GPT-4o, GPT-4, GPT-3.5, and other OpenAI models.

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

/// OpenAI API provider.
pub struct OpenAIProvider {
    /// API key.
    api_key: String,

    /// API base URL.
    base_url: String,

    /// HTTP client.
    client: Client,

    /// Model to use.
    model: String,

    /// Maximum tokens.
    max_tokens: usize,

    /// Temperature (0.0 - 2.0).
    temperature: Option<f32>,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".to_string(),
            client: Client::new(),
            model: "gpt-4o".to_string(),
            max_tokens: 4096,
            temperature: None,
        }
    }

    /// Set the base URL (for Azure OpenAI or compatible APIs).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the model.
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
                // Check if there are tool calls
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
                        ContentBlock::Thinking { thinking } => Some(format!("<thinking>{}</thinking>", thinking)),
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
impl ModelProvider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
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

        debug!("Sending request to OpenAI API");

        let response = self
            .client
            .post(format!("{}/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
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

        Ok(ModelResponse {
            content: MessageContent::Blocks(content_blocks),
            stop_reason,
            token_usage: TokenUsage {
                input: api_response.usage.prompt_tokens as u64,
                output: api_response.usage.completion_tokens as u64,
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
        if self.model.contains("gpt-4o") || self.model.contains("gpt-4-turbo") {
            128_000
        } else if self.model.contains("gpt-4") {
            8_192
        } else if self.model.contains("gpt-3.5") {
            16_385
        } else {
            128_000 // Default for newer models
        }
    }
}

// API types

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
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
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
    usage: ApiUsage,
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
        let provider = OpenAIProvider::new("test-key");
        assert_eq!(provider.name(), "openai");
        assert_eq!(provider.model(), "gpt-4o");
    }

    #[test]
    fn test_builder_pattern() {
        let provider = OpenAIProvider::new("test-key")
            .with_model("gpt-4")
            .with_max_tokens(8192)
            .with_temperature(0.7);

        assert_eq!(provider.model(), "gpt-4");
        assert_eq!(provider.max_tokens, 8192);
        assert_eq!(provider.temperature, Some(0.7));
    }
}
