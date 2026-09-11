//! Poolside AI provider.
//!
//! OpenAI-compatible chat completions API for Poolside's purpose-built coding
//! models. API keys start with `sky_`. See:
//! <https://docs.poolside.ai/get-started/supported-models>

use crate::{
    ChatOptions, ChatResponse, CompletionStream, ContentBlock, Message, MessageContent,
    ModelCapabilities, ModelInfo, ModelPricing, Provider, ProviderCapabilities, ProviderError,
    Result, Role, StopReason, StreamEvent, TokenCount, TokenUsage, ToolDefinition,
};
use async_trait::async_trait;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use tracing::debug;

/// Default Poolside API base URL.
const DEFAULT_API_BASE: &str = "https://inference.poolside.ai/v1";

/// Poolside AI provider.
pub struct PoolsideProvider {
    /// HTTP client.
    client: Client,

    /// API key.
    api_key: SecretString,

    /// API base URL.
    api_base: String,

    /// Default model.
    default_model: String,
}

impl PoolsideProvider {
    /// Create a new Poolside provider with an API key.
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
            api_key: SecretString::new(api_key.into()),
            api_base: DEFAULT_API_BASE.to_string(),
            default_model: "poolside/laguna-s-2.1".to_string(),
        })
    }

    /// Create a new provider from the `POOLSIDE_API_KEY` environment variable.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("POOLSIDE_API_KEY")
            .map_err(|_| ProviderError::config("POOLSIDE_API_KEY environment variable not set"))?;
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

    /// Convert messages to Poolside/OpenAI-compatible API format.
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
                    if tool_calls.is_empty() {
                        None
                    } else {
                        Some(tool_calls)
                    },
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

    /// Parse a Poolside API response.
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
                cache_read: 0,
                cache_creation: 0,
            },
            tool_calls: vec![],
            metadata: std::collections::HashMap::new(),
        })
    }
}

#[async_trait]
impl Provider for PoolsideProvider {
    fn name(&self) -> &str {
        "poolside"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        // Poolside has no /models endpoint; these are documented at
        // docs.poolside.ai/get-started/supported-models. Ids are fully
        // qualified (`poolside/laguna-s-2.1`, not `laguna-s-2.1`) -- the API
        // rejects the bare form.
        Ok(vec![
            ModelInfo {
                id: "poolside/laguna-s-2.1".to_string(),
                provider: "poolside".to_string(),
                display_name: "Poolside Laguna S 2.1".to_string(),
                capabilities: ModelCapabilities {
                    vision: false,
                    tool_use: true,
                    extended_thinking: false,
                    streaming: true,
                    json_mode: false,
                },
                context_window: 128_000,
                max_output_tokens: 8192,
                pricing: Some(ModelPricing {
                    input_per_1m: 1.0,
                    output_per_1m: 4.0,
                    cache_creation_per_1m: None,
                    cache_read_per_1m: None,
                }),
            },
            ModelInfo {
                id: "poolside/laguna-xs-2.1".to_string(),
                provider: "poolside".to_string(),
                display_name: "Poolside Laguna XS 2.1".to_string(),
                capabilities: ModelCapabilities {
                    vision: false,
                    tool_use: true,
                    extended_thinking: false,
                    streaming: true,
                    json_mode: false,
                },
                context_window: 128_000,
                max_output_tokens: 8192,
                pricing: Some(ModelPricing {
                    input_per_1m: 1.0,
                    output_per_1m: 4.0,
                    cache_creation_per_1m: None,
                    cache_read_per_1m: None,
                }),
            },
            ModelInfo {
                id: "poolside/laguna-m-1".to_string(),
                provider: "poolside".to_string(),
                display_name: "Poolside Laguna M 1".to_string(),
                capabilities: ModelCapabilities {
                    vision: false,
                    tool_use: true,
                    extended_thinking: false,
                    streaming: true,
                    json_mode: false,
                },
                context_window: 128_000,
                max_output_tokens: 8192,
                pricing: Some(ModelPricing {
                    input_per_1m: 1.0,
                    output_per_1m: 4.0,
                    cache_creation_per_1m: None,
                    cache_read_per_1m: None,
                }),
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

        debug!("Sending request to Poolside API (model: {})", model);

        let response = self
            .client
            .post(format!("{}/chat/completions", self.api_base))
            .header(
                "Authorization",
                format!("Bearer {}", self.api_key.expose_secret()),
            )
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
        // Streaming not yet implemented for Poolside.
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
            "Token counting not supported by Poolside",
        ))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: false,
            tools: true,
            vision: false,
            system_messages: true,
            max_context: Some(128_000),
            max_output: Some(8192),
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = PoolsideProvider::new("sky_test_key").unwrap();
        assert_eq!(provider.name(), "poolside");
    }

    #[test]
    fn test_empty_key_rejected() {
        assert!(PoolsideProvider::new("").is_err());
    }

    #[tokio::test]
    async fn test_list_models() {
        let provider = PoolsideProvider::new("sky_test_key").unwrap();
        let models = provider.list_models().await.unwrap();
        assert_eq!(models.len(), 3);
        assert!(models.iter().all(|m| m.id.starts_with("poolside/")));
    }

    #[test]
    fn test_capabilities() {
        let provider = PoolsideProvider::new("sky_test_key").unwrap();
        let caps = provider.capabilities();
        assert!(caps.tools);
        assert!(!caps.vision);
    }
}
