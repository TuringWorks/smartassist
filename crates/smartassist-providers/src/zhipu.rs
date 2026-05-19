//! Zhipu AI (GLM) API provider.
//!
//! Supports GLM-4, GLM-4V, and ChatGLM models.
//! See: https://open.bigmodel.cn/dev/api

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

/// Default Zhipu API base URL.
const DEFAULT_API_BASE: &str = "https://open.bigmodel.cn/api/paas/v4";

/// Zhipu AI (GLM) provider.
pub struct ZhipuProvider {
    /// HTTP client.
    client: Client,

    /// API key.
    api_key: SecretString,

    /// API base URL.
    api_base: String,

    /// Default model.
    default_model: String,
}

impl ZhipuProvider {
    /// Create a new Zhipu provider with an API key.
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
            default_model: "glm-4".to_string(),
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

    /// Convert messages to Zhipu API format.
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

        let (content, tool_calls) = match &message.content {
            MessageContent::Text(text) => (Some(ApiContent::Text(text.clone())), None),
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
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                (
                    if text.is_empty() { None } else { Some(ApiContent::Text(text)) },
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

    /// Parse a Zhipu API response.
    fn parse_response(&self, response: ApiResponse, model: &str) -> Result<ChatResponse> {
        let choice = response
            .choices
            .first()
            .ok_or_else(|| ProviderError::internal("No choices in response"))?;

        let mut content_blocks: Vec<ContentBlock> = Vec::new();

        if let Some(ApiContent::Text(text)) = &choice.message.content {
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
                cache_read: 0,
                cache_creation: 0,
            },
            tool_calls: vec![],
            metadata: std::collections::HashMap::new(),
        })
    }
}

#[async_trait]
impl Provider for ZhipuProvider {
    fn name(&self) -> &str {
        "zhipu"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "glm-4".to_string(),
                provider: "zhipu".to_string(),
                display_name: "GLM-4".to_string(),
                capabilities: ModelCapabilities {
                    vision: false,
                    tool_use: true,
                    extended_thinking: false,
                    streaming: true,
                    json_mode: true,
                },
                context_window: 128000,
                max_output_tokens: 4096,
                pricing: None,
            },
            ModelInfo {
                id: "glm-4v".to_string(),
                provider: "zhipu".to_string(),
                display_name: "GLM-4V (Vision)".to_string(),
                capabilities: ModelCapabilities {
                    vision: true,
                    tool_use: true,
                    extended_thinking: false,
                    streaming: true,
                    json_mode: true,
                },
                context_window: 128000,
                max_output_tokens: 4096,
                pricing: None,
            },
            ModelInfo {
                id: "glm-4-flash".to_string(),
                provider: "zhipu".to_string(),
                display_name: "GLM-4 Flash".to_string(),
                capabilities: ModelCapabilities {
                    vision: false,
                    tool_use: true,
                    extended_thinking: false,
                    streaming: true,
                    json_mode: true,
                },
                context_window: 128000,
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

        debug!("Sending request to Zhipu API (model: {})", model);

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
        // Streaming not yet implemented for Zhipu
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
            "Token counting not supported by Zhipu",
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
    content: Option<ApiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ApiToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ApiContent {
    Text(String),
    Parts(Vec<ApiContentPart>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ApiContentPart {
    Text { text: String },
    Image { image_url: ApiImageUrl },
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
    usage: ApiUsage,
}

#[derive(Debug, Deserialize)]
struct ApiChoice {
    message: ApiResponseMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ApiResponseMessage {
    content: Option<ApiContent>,
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
        let provider = ZhipuProvider::new("test-key").unwrap();
        assert_eq!(provider.name(), "zhipu");
    }

    #[test]
    fn test_empty_key_rejected() {
        assert!(ZhipuProvider::new("").is_err());
    }
}