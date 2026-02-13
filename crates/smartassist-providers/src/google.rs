//! Google Gemini provider implementation.
//!
//! This module provides integration with Google's Gemini models.

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

/// Default Google AI API base URL.
const DEFAULT_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Google Gemini provider.
pub struct GoogleProvider {
    /// HTTP client.
    client: Client,

    /// API key.
    api_key: SecretString,

    /// API base URL.
    api_base: String,

    /// Default model to use.
    default_model: String,
}

impl GoogleProvider {
    /// Create a new Google provider with an API key.
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
            default_model: "gemini-2.0-flash".to_string(),
        })
    }

    /// Create a new provider from environment variable.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GOOGLE_API_KEY")
            .or_else(|_| std::env::var("GEMINI_API_KEY"))
            .map_err(|_| {
                ProviderError::config(
                    "GOOGLE_API_KEY or GEMINI_API_KEY environment variable not set",
                )
            })?;
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

    /// Convert messages to Gemini format.
    fn convert_messages(
        &self,
        messages: &[Message],
    ) -> Result<(Option<GeminiSystemInstruction>, Vec<GeminiContent>)> {
        let mut system_instruction = None;
        let mut contents = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // Gemini uses system_instruction for system messages
                    if let Some(text) = msg.text() {
                        system_instruction = Some(GeminiSystemInstruction {
                            parts: vec![GeminiPart::Text { text: text.to_string() }],
                        });
                    }
                }
                MessageRole::User => {
                    contents.push(GeminiContent {
                        role: "user".to_string(),
                        parts: self.convert_content(&msg.content)?,
                    });
                }
                MessageRole::Assistant => {
                    contents.push(GeminiContent {
                        role: "model".to_string(),
                        parts: self.convert_content(&msg.content)?,
                    });
                }
                MessageRole::Tool => {
                    // Tool results in Gemini format
                    if let Some(tool_call_id) = &msg.tool_call_id {
                        contents.push(GeminiContent {
                            role: "user".to_string(),
                            parts: vec![GeminiPart::FunctionResponse {
                                function_response: GeminiFunctionResponse {
                                    name: tool_call_id.clone(),
                                    response: serde_json::json!({
                                        "result": msg.text().unwrap_or("")
                                    }),
                                },
                            }],
                        });
                    }
                }
            }
        }

        Ok((system_instruction, contents))
    }

    /// Convert content to Gemini parts.
    fn convert_content(&self, content: &MessageContent) -> Result<Vec<GeminiPart>> {
        match content {
            MessageContent::Text(s) => Ok(vec![GeminiPart::Text { text: s.clone() }]),
            MessageContent::Parts(parts) => {
                let mut gemini_parts = Vec::new();
                for part in parts {
                    match part {
                        crate::ContentPart::Text(s) => {
                            gemini_parts.push(GeminiPart::Text { text: s.clone() });
                        }
                        crate::ContentPart::Image(img) => {
                            gemini_parts.push(GeminiPart::InlineData {
                                inline_data: InlineData {
                                    mime_type: img.media_type.clone(),
                                    data: img.data.clone(),
                                },
                            });
                        }
                        crate::ContentPart::ToolUse(tool) => {
                            gemini_parts.push(GeminiPart::FunctionCall {
                                function_call: GeminiFunctionCall {
                                    name: tool.name.clone(),
                                    args: tool.input.clone(),
                                },
                            });
                        }
                        _ => {}
                    }
                }
                Ok(gemini_parts)
            }
        }
    }

    /// Convert tools to Gemini format.
    fn convert_tools(&self, tools: &[crate::ToolDefinition]) -> Vec<GeminiTool> {
        vec![GeminiTool {
            function_declarations: tools
                .iter()
                .map(|t| GeminiFunctionDeclaration {
                    name: t.name.clone(),
                    description: t.description.clone(),
                    parameters: t.input_schema.clone(),
                })
                .collect(),
        }]
    }

    /// Parse Gemini response.
    fn parse_response(&self, response: GeminiResponse, model: &str) -> Result<ChatResponse> {
        let candidate = response
            .candidates
            .into_iter()
            .next()
            .ok_or_else(|| ProviderError::internal("No candidates in response"))?;

        let mut content = String::new();
        let mut tool_calls = Vec::new();

        for part in candidate.content.parts {
            match part {
                GeminiPart::Text { text } => {
                    content.push_str(&text);
                }
                GeminiPart::FunctionCall { function_call } => {
                    tool_calls.push(ToolUse {
                        id: uuid::Uuid::new_v4().to_string(),
                        name: function_call.name,
                        input: function_call.args,
                    });
                }
                _ => {}
            }
        }

        let stop_reason = match candidate.finish_reason.as_deref() {
            Some("STOP") => StopReason::EndTurn,
            Some("MAX_TOKENS") => StopReason::MaxTokens,
            Some("SAFETY") => StopReason::ContentFilter,
            Some("TOOL_USE") => StopReason::ToolUse,
            _ => StopReason::Unknown,
        };

        let usage = response.usage_metadata.unwrap_or_default();

        Ok(ChatResponse {
            id: uuid::Uuid::new_v4().to_string(),
            model: model.to_string(),
            content,
            tool_calls,
            stop_reason,
            usage: Usage {
                input_tokens: usage.prompt_token_count,
                output_tokens: usage.candidates_token_count,
                cache_read_tokens: usage.cached_content_token_count.unwrap_or(0),
                cache_creation_tokens: 0,
            },
            metadata: HashMap::new(),
        })
    }
}

#[async_trait]
impl Provider for GoogleProvider {
    fn name(&self) -> &str {
        "google"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let response = self
            .client
            .get(format!(
                "{}/models?key={}",
                self.api_base,
                self.api_key.expose_secret()
            ))
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
            models: Vec<GeminiModel>,
        }

        #[derive(Deserialize)]
        struct GeminiModel {
            name: String,
            #[serde(default)]
            display_name: String,
            #[serde(default)]
            description: String,
            #[serde(default)]
            input_token_limit: usize,
            #[serde(default)]
            output_token_limit: usize,
        }

        let result: ModelsResponse = response.json().await?;

        let models: Vec<ModelInfo> = result
            .models
            .into_iter()
            .filter(|m| m.name.contains("gemini"))
            .map(|m| {
                let id = m.name.replace("models/", "");
                ModelInfo {
                    id: id.clone(),
                    name: if m.display_name.is_empty() {
                        id
                    } else {
                        m.display_name
                    },
                    description: m.description,
                    context_window: m.input_token_limit,
                    max_output: m.output_token_limit,
                    input_price: 0.0,
                    output_price: 0.0,
                    capabilities: vec!["tools".to_string(), "vision".to_string()],
                }
            })
            .collect();

        Ok(models)
    }

    async fn chat(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<ChatResponse> {
        let options = options.unwrap_or_default();
        let (system_instruction, contents) = self.convert_messages(messages)?;

        let request = GeminiRequest {
            contents,
            system_instruction,
            generation_config: Some(GeminiGenerationConfig {
                max_output_tokens: options.max_tokens,
                temperature: options.temperature,
                top_p: options.top_p,
                top_k: options.top_k,
                stop_sequences: options.stop,
            }),
            tools: options.tools.as_ref().map(|t| self.convert_tools(t)),
        };

        debug!("Sending request to Google: model={}", model);

        let response = self
            .client
            .post(format!(
                "{}/models/{}:generateContent?key={}",
                self.api_base,
                model,
                self.api_key.expose_secret()
            ))
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body: GeminiError = response.json().await.unwrap_or_else(|_| GeminiError {
                error: GeminiErrorDetail {
                    code: status.as_u16() as i32,
                    message: "Unknown error".to_string(),
                    status: "UNKNOWN".to_string(),
                },
            });

            return match status.as_u16() {
                401 | 403 => Err(ProviderError::auth(error_body.error.message)),
                429 => Err(ProviderError::rate_limit(error_body.error.message, None)),
                400 => Err(ProviderError::invalid_request(error_body.error.message)),
                _ => Err(ProviderError::server_error(
                    status.as_u16(),
                    error_body.error.message,
                )),
            };
        }

        let response: GeminiResponse = response.json().await?;
        self.parse_response(response, model)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<CompletionStream> {
        let options = options.unwrap_or_default();
        let (system_instruction, contents) = self.convert_messages(messages)?;

        let request = GeminiRequest {
            contents,
            system_instruction,
            generation_config: Some(GeminiGenerationConfig {
                max_output_tokens: options.max_tokens,
                temperature: options.temperature,
                top_p: options.top_p,
                top_k: options.top_k,
                stop_sequences: options.stop,
            }),
            tools: options.tools.as_ref().map(|t| self.convert_tools(t)),
        };

        let response = self
            .client
            .post(format!(
                "{}/models/{}:streamGenerateContent?key={}&alt=sse",
                self.api_base,
                model,
                self.api_key.expose_secret()
            ))
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body: GeminiError = response.json().await.unwrap_or_else(|_| GeminiError {
                error: GeminiErrorDetail {
                    code: status.as_u16() as i32,
                    message: "Unknown error".to_string(),
                    status: "UNKNOWN".to_string(),
                },
            });

            return match status.as_u16() {
                401 | 403 => Err(ProviderError::auth(error_body.error.message)),
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
            let _model = model.clone();
            async move {
                match result {
                    Ok(event) => {
                        if event.data.is_empty() {
                            return None;
                        }

                        let parsed: std::result::Result<GeminiStreamChunk, _> =
                            serde_json::from_str(&event.data);

                        match parsed {
                            Ok(chunk) => {
                                if let Some(candidate) = chunk.candidates.into_iter().next() {
                                    for part in candidate.content.parts {
                                        if let GeminiPart::Text { text } = part {
                                            return Some(Ok(StreamEvent::ContentDelta {
                                                delta: text,
                                            }));
                                        }
                                    }

                                    if let Some(finish_reason) = candidate.finish_reason {
                                        let stop_reason = match finish_reason.as_str() {
                                            "STOP" => StopReason::EndTurn,
                                            "MAX_TOKENS" => StopReason::MaxTokens,
                                            "SAFETY" => StopReason::ContentFilter,
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

    async fn count_tokens(&self, model: &str, messages: &[Message]) -> Result<TokenCount> {
        let (system_instruction, contents) = self.convert_messages(messages)?;

        let request = serde_json::json!({
            "contents": contents,
            "system_instruction": system_instruction,
        });

        let response = self
            .client
            .post(format!(
                "{}/models/{}:countTokens?key={}",
                self.api_base,
                model,
                self.api_key.expose_secret()
            ))
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
            #[serde(rename = "totalTokens")]
            total_tokens: usize,
        }

        let result: TokenCountResponse = response.json().await?;
        Ok(TokenCount {
            count: result.total_tokens,
            model: model.to_string(),
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tools: true,
            vision: true,
            system_messages: true,
            max_context: Some(2_000_000), // Gemini 1.5 Pro
            max_output: Some(8192),
        }
    }
}

// Internal types for Gemini API

#[derive(Serialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiSystemInstruction>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "generationConfig")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
}

#[derive(Serialize, Deserialize)]
struct GeminiContent {
    role: String,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize)]
struct GeminiSystemInstruction {
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
    InlineData { inline_data: InlineData },
    FunctionCall { function_call: GeminiFunctionCall },
    FunctionResponse { function_response: GeminiFunctionResponse },
}

#[derive(Serialize, Deserialize)]
struct InlineData {
    mime_type: String,
    data: String,
}

#[derive(Serialize, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}

#[derive(Serialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none", rename = "maxOutputTokens")]
    max_output_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topP")]
    top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "topK")]
    top_k: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "stopSequences")]
    stop_sequences: Option<Vec<String>>,
}

#[derive(Serialize)]
struct GeminiTool {
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Serialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount", default)]
    prompt_token_count: usize,
    #[serde(rename = "candidatesTokenCount", default)]
    candidates_token_count: usize,
    #[serde(rename = "cachedContentTokenCount")]
    cached_content_token_count: Option<usize>,
}

#[derive(Deserialize)]
struct GeminiError {
    error: GeminiErrorDetail,
}

#[derive(Deserialize)]
struct GeminiErrorDetail {
    code: i32,
    message: String,
    status: String,
}

// Streaming types

#[derive(Deserialize)]
struct GeminiStreamChunk {
    candidates: Vec<GeminiCandidate>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let provider = GoogleProvider::new("test-key").unwrap();
        assert_eq!(provider.name(), "google");
    }

    #[test]
    fn test_provider_empty_key() {
        let result = GoogleProvider::new("");
        assert!(result.is_err());
    }

    #[test]
    fn test_capabilities() {
        let provider = GoogleProvider::new("test-key").unwrap();
        let caps = provider.capabilities();

        assert!(caps.streaming);
        assert!(caps.tools);
        assert!(caps.vision);
        assert_eq!(caps.max_context, Some(2_000_000));
    }
}
