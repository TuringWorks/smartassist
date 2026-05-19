//! OpenRouter API provider.
//!
//! OpenRouter provides unified access to many AI models through a single API.
//! Supports models from Anthropic, OpenAI, Google, Meta, Mistral, and more.
//! See: https://openrouter.ai/docs

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

/// Default OpenRouter API base URL.
const DEFAULT_API_BASE: &str = "https://openrouter.ai/api/v1";

/// OpenRouter API provider.
pub struct OpenRouterProvider {
    /// HTTP client.
    client: Client,

    /// API key.
    api_key: SecretString,

    /// API base URL.
    api_base: String,

    /// Default model.
    default_model: String,

    /// Site URL for rankings (optional).
    site_url: Option<String>,

    /// Site name for rankings (optional).
    site_name: Option<String>,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider with an API key.
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
            default_model: "anthropic/claude-3.5-sonnet".to_string(),
            site_url: None,
            site_name: None,
        })
    }

    /// Set the default model.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
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

    /// Convert messages to OpenRouter API format.
    fn convert_messages(&self, messages: &[Message]) -> Vec<ApiMessage> {
        messages.iter().map(|m| self.convert_message(m)).collect()
    }

    /// Convert a single message.
    fn convert_message(&self, message: &Message) -> ApiMessage {
        let role = match message.role {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
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

                let tool_result = blocks.iter().find_map(|block| match block {
                    ContentBlock::ToolResult { content, .. } => Some(content.clone()),
                    _ => None,
                });

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

    /// Parse an OpenRouter API response.
    fn parse_response(&self, response: ApiResponse, model: &str) -> Result<ChatResponse> {
        let choice = response
            .choices
            .first()
            .ok_or_else(|| ProviderError::internal("No choices in response"))?;

        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        if let Some(ApiMessageContent::Text(text)) = &choice.message.content {
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

        let usage = response.usage.unwrap_or(ApiUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
        });

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
                input: usage.prompt_tokens as u64,
                output: usage.completion_tokens as u64,
                cache_read: 0,
                cache_creation: 0,
            },
            tool_calls: vec![],
            metadata: std::collections::HashMap::new(),
        })
    }
}

#[async_trait]
impl Provider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        // OpenRouter routes to many providers, return common ones
        Ok(vec![
            ModelInfo {
                id: "anthropic/claude-3.5-sonnet".to_string(),
                provider: "openrouter".to_string(),
                display_name: "Claude 3.5 Sonnet (via OpenRouter)".to_string(),
                capabilities: ModelCapabilities {
                    vision: true,
                    tool_use: true,
                    extended_thinking: false,
                    streaming: true,
                    json_mode: true,
                },
                context_window: 200000,
                max_output_tokens: 8192,
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

        debug!("Sending request to OpenRouter API (model: {})", model);

        let mut req_builder = self
            .client
            .post(format!("{}/chat/completions", self.api_base))
            .header("Authorization", format!("Bearer {}", self.api_key.expose_secret()))
            .header("Content-Type", "application/json");

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
        // Streaming not yet implemented for OpenRouter
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
            "Token counting not supported by OpenRouter",
        ))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: false,
            tools: true,
            vision: true,
            system_messages: true,
            max_context: Some(128000),
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
    #[serde(default)]
    id: Option<String>,
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
    #[serde(default)]
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
        let provider = OpenRouterProvider::new("test-key").unwrap();
        assert_eq!(provider.name(), "openrouter");
    }

    #[test]
    fn test_empty_key_rejected() {
        assert!(OpenRouterProvider::new("").is_err());
    }
}