//! Anthropic Claude API provider.

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

/// Anthropic API provider.
pub struct AnthropicProvider {
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
}

impl AnthropicProvider {
    /// Create a new Anthropic provider.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            client: Client::new(),
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
        }
    }

    /// Set the base URL.
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

    /// Build the API request.
    fn build_request(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> ApiRequest {
        let api_messages: Vec<ApiMessage> = messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(|m| self.convert_message(m))
            .collect();

        let system = messages
            .iter()
            .find(|m| m.role == Role::System)
            .map(|m| m.content.to_text());

        let api_tools: Vec<ApiTool> = tools
            .iter()
            .map(|t| ApiTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            })
            .collect();

        ApiRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: api_messages,
            system,
            tools: if api_tools.is_empty() {
                None
            } else {
                Some(api_tools)
            },
            stream: false,
        }
    }

    /// Convert a message to API format.
    fn convert_message(&self, message: &Message) -> ApiMessage {
        let role = match message.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "user", // Should be filtered out
            Role::Tool => "user",
        };

        let content = match &message.content {
            MessageContent::Text(text) => vec![ApiContent::Text {
                text: text.clone(),
            }],
            MessageContent::Blocks(blocks) => blocks
                .iter()
                .filter_map(|block| self.convert_block(block))
                .collect(),
        };

        ApiMessage {
            role: role.to_string(),
            content,
        }
    }

    /// Convert a content block to API format.
    fn convert_block(&self, block: &ContentBlock) -> Option<ApiContent> {
        match block {
            ContentBlock::Text { text } => Some(ApiContent::Text {
                text: text.clone(),
            }),
            ContentBlock::Image { source } => Some(ApiContent::Image {
                source: ApiImageSource {
                    source_type: source.source_type.clone(),
                    media_type: source.media_type.clone(),
                    data: source.data.clone(),
                },
            }),
            ContentBlock::ToolUse { id, name, input } => Some(ApiContent::ToolUse {
                id: id.clone(),
                name: name.clone(),
                input: input.clone(),
            }),
            ContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
            } => Some(ApiContent::ToolResult {
                tool_use_id: tool_use_id.clone(),
                content: content.clone(),
                is_error: Some(*is_error),
            }),
            ContentBlock::Thinking { thinking } => Some(ApiContent::Thinking {
                thinking: thinking.clone(),
            }),
        }
    }
}

#[async_trait]
impl ModelProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
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

        debug!("Sending request to Anthropic API");

        let response = self
            .client
            .post(format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
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

        // Convert response content
        let content_blocks: Vec<ContentBlock> = api_response
            .content
            .iter()
            .filter_map(|c| match c {
                ApiContent::Text { text } => Some(ContentBlock::Text { text: text.clone() }),
                ApiContent::ToolUse { id, name, input } => Some(ContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: input.clone(),
                }),
                ApiContent::Thinking { thinking } => Some(ContentBlock::Thinking {
                    thinking: thinking.clone(),
                }),
                _ => None,
            })
            .collect();

        Ok(ModelResponse {
            content: MessageContent::Blocks(content_blocks),
            stop_reason: api_response.stop_reason,
            token_usage: TokenUsage {
                input: api_response.usage.input_tokens as u64,
                output: api_response.usage.output_tokens as u64,
                cache_read: api_response.usage.cache_read_input_tokens.unwrap_or(0) as u64,
                cache_creation: api_response.usage.cache_creation_input_tokens.unwrap_or(0) as u64,
            },
        })
    }

    fn complete_stream(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + '_>> {
        // Simplified: streaming not yet implemented
        Box::pin(futures::stream::once(async {
            Err(AgentError::provider("Streaming not yet implemented"))
        }))
    }

    fn context_limit(&self) -> usize {
        if self.model.contains("opus") || self.model.contains("sonnet") || self.model.contains("haiku") {
            200_000
        } else {
            200_000 // Default for Claude models
        }
    }
}

// API types

#[derive(Debug, Serialize)]
struct ApiRequest {
    model: String,
    max_tokens: usize,
    messages: Vec<ApiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ApiTool>>,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ApiMessage {
    role: String,
    content: Vec<ApiContent>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApiContent {
    Text {
        text: String,
    },
    Image {
        source: ApiImageSource,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
    Thinking {
        thinking: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct ApiImageSource {
    #[serde(rename = "type")]
    source_type: String,
    media_type: String,
    data: String,
}

#[derive(Debug, Serialize)]
struct ApiTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    content: Vec<ApiContent>,
    stop_reason: Option<String>,
    usage: ApiUsage,
}

#[derive(Debug, Deserialize)]
struct ApiUsage {
    input_tokens: usize,
    output_tokens: usize,
    #[serde(default)]
    cache_read_input_tokens: Option<usize>,
    #[serde(default)]
    cache_creation_input_tokens: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = AnthropicProvider::new("test-key");
        assert_eq!(provider.name(), "anthropic");
    }
}
