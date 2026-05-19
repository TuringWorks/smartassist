//! DeepSeek API provider.
//!
//! Supports DeepSeek-V3, DeepSeek-V2, DeepSeek Coder, and DeepSeek Chat models.
//! API is OpenAI-compatible.
//! See: https://platform.deepseek.com/docs

use crate::{
    ChatOptions, ChatResponse, CompletionStream, ContentBlock, Message, MessageContent,
    ModelCapabilities, ModelInfo, Provider, ProviderCapabilities, ProviderError, Result, Role,
    StopReason, StreamEvent, TokenCount, TokenUsage, ToolDefinition,
};
use async_trait::async_trait;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Default DeepSeek API base URL.
const DEFAULT_API_BASE: &str = "https://api.deepseek.com";

/// DeepSeek API provider.
pub struct DeepSeekProvider {
    /// HTTP client.
    client: Client,

    /// API key.
    api_key: SecretString,

    /// API base URL.
    api_base: String,

    /// Default model.
    default_model: String,
}

impl DeepSeekProvider {
    /// Create a new DeepSeek provider with an API key.
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
            default_model: "deepseek-chat".to_string(),
        })
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

    /// Convert messages to DeepSeek API format.
    fn convert_messages(&self, messages: &[Message]) -> Vec<ApiMessage> {
        messages
            .iter()
            .map(|m| self.convert_message(m))
            .collect()
    }

    /// Convert a single message.
    fn convert_message(&self, message: &Message) -> ApiMessage {
        let role = match message.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        };

        let (content, tool_calls) = match &message.content {
            MessageContent::Text(text) => (Some(text.clone()), None),
            MessageContent::Blocks(blocks) => {
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

                let text: String = blocks
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Text { text } => Some(text.clone()),
                        ContentBlock::ToolResult { content, .. } => Some(content.clone()),
                        ContentBlock::Thinking { thinking } => {
                            Some(format!("<thinking>{}</thinking>", thinking))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                (
                    if text.is_empty() { None } else { Some(text) },
                    if tool_calls.is_empty() { None } else { Some(tool_calls) },
                )
            }
        };

        ApiMessage {
            role: role.to_string(),
            content,
            tool_calls,
            tool_call_id: message.tool_use_id.clone(),
        }
    }

    /// Convert tools to API format.
    fn convert_tools(&self, tools: &[ToolDefinition]) -> Vec<ApiTool> {
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
            .collect()
    }

    /// Parse a DeepSeek API response.
    fn parse_response(&self, response: ApiResponse, model: &str) -> Result<ChatResponse> {
        let choice = response
            .choices
            .first()
            .ok_or_else(|| ProviderError::internal("No choices in response"))?;

        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        if let Some(text) = &choice.message.content {
            if !text.is_empty() {
                content_blocks.push(ContentBlock::Text { text: text.clone() });
            }
        }

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

        let stop_reason = match choice.finish_reason.as_deref() {
            Some("stop") => StopReason::EndTurn,
            Some("length") => StopReason::MaxTokens,
            Some("tool_calls") => StopReason::ToolUse,
            Some("content_filter") => StopReason::ContentFilter,
            _ => StopReason::EndTurn,
        };

        let content = if content_blocks.is_empty() {
            MessageContent::Text(String::new())
        } else if content_blocks.len() == 1 {
            match content_blocks.into_iter().next() {
                Some(ContentBlock::Text { text }) => MessageContent::Text(text),
                other => MessageContent::Blocks(vec![other.unwrap()]),
            }
        } else {
            MessageContent::Blocks(content_blocks)
        };

        Ok(ChatResponse {
            id: response.id.unwrap_or_default(),
            model: model.to_string(),
            content,
            stop_reason,
            usage: TokenUsage {
                input: response.usage.prompt_tokens as u64,
                output: response.usage.completion_tokens as u64,
                cache_read: response.usage.prompt_cache_hit_tokens.unwrap_or(0) as u64,
                cache_creation: response.usage.prompt_cache_miss_tokens.unwrap_or(0) as u64,
            },
            tool_calls: vec![],
            metadata: std::collections::HashMap::new(),
        })
    }
}

#[async_trait]
impl Provider for DeepSeekProvider {
    fn name(&self) -> &str {
        "deepseek"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "deepseek-chat".to_string(),
                provider: "deepseek".to_string(),
                display_name: "DeepSeek Chat (V3)".to_string(),
                capabilities: ModelCapabilities {
                    vision: false,
                    tool_use: true,
                    extended_thinking: false,
                    streaming: true,
                    json_mode: true,
                },
                context_window: 64000,
                max_output_tokens: 4096,
                pricing: None,
            },
            ModelInfo {
                id: "deepseek-reasoner".to_string(),
                provider: "deepseek".to_string(),
                display_name: "DeepSeek Reasoner (R1)".to_string(),
                capabilities: ModelCapabilities {
                    vision: false,
                    tool_use: false,
                    extended_thinking: true,
                    streaming: true,
                    json_mode: true,
                },
                context_window: 64000,
                max_output_tokens: 4096,
                pricing: None,
            },
        ])
    }

    async fn chat(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<ChatResponse> {
        let api_messages = self.convert_messages(messages);
        let tools = options
            .as_ref()
            .and_then(|o| o.tools.as_ref())
            .map(|t| self.convert_tools(t))
            .unwrap_or_default();

        let request = ApiRequest {
            model: model.to_string(),
            messages: api_messages,
            max_tokens: options.as_ref().and_then(|o| o.max_tokens),
            temperature: options.as_ref().and_then(|o| o.temperature),
            tools: if tools.is_empty() { None } else { Some(tools) },
            stream: false,
        };

        debug!("Sending request to DeepSeek API (model: {})", model);

        let response = self
            .client
            .post(format!("{}/chat/completions", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| ProviderError::internal(format!("Request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(ProviderError::internal(format!(
                "API error {}: {}",
                status, body
            )));
        }

        let api_response: ApiResponse = response
            .json()
            .await
            .map_err(|e| ProviderError::internal(format!("Failed to parse response: {}", e)))?;

        self.parse_response(api_response, model)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<CompletionStream> {
        // Streaming not yet implemented for DeepSeek
        let response = self.chat(model, messages, options).await?;
        Ok(Box::pin(futures::stream::once(async move {
            Ok(StreamEvent::End {
                stop_reason: response.stop_reason,
                usage: response.usage,
            })
        })))
    }

    async fn count_tokens(&self, _model: &str, _messages: &[Message]) -> Result<TokenCount> {
        Err(ProviderError::internal(
            "Token counting not supported by DeepSeek",
        ))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: false,
            tools: true,
            vision: false,
            system_messages: true,
            max_context: Some(64000),
            max_output: Some(4096),
        }
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
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
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
    #[serde(default)]
    id: Option<String>,
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
    content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ApiToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ApiUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
    #[serde(default)]
    prompt_cache_hit_tokens: Option<usize>,
    #[serde(default)]
    prompt_cache_miss_tokens: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = DeepSeekProvider::new("test-key").unwrap();
        assert_eq!(provider.name(), "deepseek");
    }

    #[test]
    fn test_empty_key_rejected() {
        assert!(DeepSeekProvider::new("").is_err());
    }
}