//! Ollama provider for local open-source models.
//!
//! Supports Llama, Mistral, Qwen, DeepSeek, and other models via Ollama.
//! Ollama must be running locally (default: http://localhost:11434).

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

/// Ollama provider for local models.
pub struct OllamaProvider {
    /// Base URL for Ollama server.
    base_url: String,

    /// HTTP client.
    client: Client,

    /// Model to use.
    model: String,

    /// Context window size (num_ctx).
    context_size: Option<usize>,

    /// Temperature (0.0 - 2.0).
    temperature: Option<f32>,

    /// Number of tokens to predict.
    num_predict: Option<usize>,
}

impl OllamaProvider {
    /// Create a new Ollama provider with the default URL.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            client: Client::new(),
            model: model.into(),
            context_size: None,
            temperature: None,
            num_predict: None,
        }
    }

    /// Set the base URL for Ollama server.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the context window size.
    pub fn with_context_size(mut self, size: usize) -> Self {
        self.context_size = Some(size);
        self
    }

    /// Set the temperature.
    pub fn with_temperature(mut self, temperature: f32) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// Set the maximum tokens to predict.
    pub fn with_num_predict(mut self, num_predict: usize) -> Self {
        self.num_predict = Some(num_predict);
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
                            description: t.description.clone(),
                            parameters: t.input_schema.clone(),
                        },
                    })
                    .collect(),
            )
        };

        let options = if self.context_size.is_some()
            || self.temperature.is_some()
            || self.num_predict.is_some()
        {
            Some(ApiOptions {
                num_ctx: self.context_size,
                temperature: self.temperature,
                num_predict: self.num_predict,
            })
        } else {
            None
        };

        ApiRequest {
            model: self.model.clone(),
            messages: api_messages,
            tools: api_tools,
            options,
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

        let content = match &message.content {
            MessageContent::Text(text) => text.clone(),
            MessageContent::Blocks(blocks) => {
                // Combine text blocks and tool results
                blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.clone()),
                        ContentBlock::Thinking { thinking } => {
                            Some(format!("<thinking>{}</thinking>", thinking))
                        }
                        ContentBlock::ToolResult { content, .. } => Some(content.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
        };

        // Extract tool calls for assistant messages
        let tool_calls: Option<Vec<ApiToolCall>> = match &message.content {
            MessageContent::Blocks(blocks) => {
                let calls: Vec<ApiToolCall> = blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::ToolUse { id, name, input } => Some(ApiToolCall {
                            id: Some(id.clone()),
                            call_type: Some("function".to_string()),
                            function: ApiFunctionCall {
                                name: name.clone(),
                                arguments: input.clone(),
                            },
                        }),
                        _ => None,
                    })
                    .collect();
                if calls.is_empty() {
                    None
                } else {
                    Some(calls)
                }
            }
            _ => None,
        };

        ApiMessage {
            role: role.to_string(),
            content,
            tool_calls,
            images: None,
        }
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
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

        debug!("Sending request to Ollama API: {}", self.base_url);

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                if e.is_connect() {
                    AgentError::provider(format!(
                        "Cannot connect to Ollama at {}. Is Ollama running?",
                        self.base_url
                    ))
                } else {
                    AgentError::provider(format!("Request failed: {}", e))
                }
            })?;

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

        // Convert response content
        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        // Add text content
        if !api_response.message.content.is_empty() {
            content_blocks.push(ContentBlock::Text {
                text: api_response.message.content.clone(),
            });
        }

        // Add tool calls
        if let Some(tool_calls) = &api_response.message.tool_calls {
            for tc in tool_calls {
                content_blocks.push(ContentBlock::ToolUse {
                    id: tc.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    name: tc.function.name.clone(),
                    input: tc.function.arguments.clone(),
                });
            }
        }

        // Determine stop reason
        let stop_reason = if api_response.done {
            Some(api_response.done_reason.unwrap_or_else(|| "stop".to_string()))
        } else {
            None
        };

        Ok(ModelResponse {
            content: MessageContent::Blocks(content_blocks),
            stop_reason,
            token_usage: TokenUsage {
                input: api_response.prompt_eval_count.unwrap_or(0) as u64,
                output: api_response.eval_count.unwrap_or(0) as u64,
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
        32_000 // Default for local models
    }
}

// API types

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<ApiOptions>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ApiOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    num_ctx: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_predict: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
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
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    call_type: Option<String>,
    function: ApiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiFunctionCall {
    name: String,
    arguments: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    message: ApiResponseMessage,
    done: bool,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<usize>,
    #[serde(default)]
    eval_count: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ApiResponseMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = OllamaProvider::new("llama3.2");
        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.model(), "llama3.2");
    }

    #[test]
    fn test_builder_pattern() {
        let provider = OllamaProvider::new("mistral")
            .with_base_url("http://192.168.1.100:11434")
            .with_context_size(8192)
            .with_temperature(0.8)
            .with_num_predict(2048);

        assert_eq!(provider.model(), "mistral");
        assert_eq!(provider.base_url, "http://192.168.1.100:11434");
        assert_eq!(provider.context_size, Some(8192));
        assert_eq!(provider.temperature, Some(0.8));
        assert_eq!(provider.num_predict, Some(2048));
    }

    #[test]
    fn test_default_url() {
        let provider = OllamaProvider::new("llama3.2");
        assert_eq!(provider.base_url, "http://localhost:11434");
    }
}
