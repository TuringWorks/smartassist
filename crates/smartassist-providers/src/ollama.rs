//! Ollama chat provider implementation.
//!
//! Supports local LLM inference via Ollama's native API.

use crate::{
    ChatOptions, ChatResponse, CompletionStream, Message, MessageContent, MessageRole, ModelInfo,
    Provider, ProviderCapabilities, ProviderError, Result, StopReason, StreamEvent, TokenCount,
    Usage,
};
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Default Ollama API base URL.
const DEFAULT_BASE_URL: &str = "http://localhost:11434";

/// Ollama chat provider.
pub struct OllamaProvider {
    client: Client,
    base_url: String,
    default_model: String,
}

impl OllamaProvider {
    /// Create a new Ollama provider.
    pub fn new() -> Self {
        let base_url = std::env::var("OLLAMA_HOST")
            .unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            base_url,
            default_model: "llama3".to_string(),
        }
    }

    /// Create from environment (always succeeds if Ollama is reachable).
    pub fn from_env() -> Result<Self> {
        Ok(Self::new())
    }

    /// Set the base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the default model.
    pub fn with_default_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    fn convert_messages(&self, messages: &[Message]) -> Vec<OllamaMessage> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                    MessageRole::Tool => "user",
                };

                let content = match &msg.content {
                    MessageContent::Text(s) => s.clone(),
                    MessageContent::Parts(parts) => {
                        parts
                            .iter()
                            .filter_map(|p| match p {
                                crate::ContentPart::Text(s) => Some(s.clone()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("")
                    }
                };

                OllamaMessage {
                    role: role.to_string(),
                    content,
                }
            })
            .collect()
    }

    fn parse_response(&self, response: OllamaChatResponse) -> Result<ChatResponse> {
        let content = response.message.content;

        Ok(ChatResponse {
            id: format!("ollama-{}-{}", response.model, response.created_at.unwrap_or_default()),
            model: response.model,
            content,
            tool_calls: vec![],
            stop_reason: if response.done {
                StopReason::EndTurn
            } else {
                StopReason::Unknown
            },
            usage: Usage {
                input_tokens: response.prompt_eval_count.unwrap_or(0),
                output_tokens: response.eval_count.unwrap_or(0),
                cache_read_tokens: 0,
                cache_creation_tokens: 0,
            },
            metadata: HashMap::new(),
        })
    }
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>> {
        let url = format!("{}/api/tags", self.base_url);

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e))?;

        if !response.status().is_success() {
            return Err(ProviderError::server_error(
                response.status().as_u16(),
                "Failed to list Ollama models",
            ));
        }

        let result: OllamaTagsResponse = response.json().await?;

        let models = result
            .models
            .into_iter()
            .map(|m| ModelInfo {
                id: m.name.clone(),
                name: m.name.clone(),
                description: m.details.and_then(|d| d.family).unwrap_or_default(),
                context_window: 128_000,
                max_output: 16_384,
                input_price: 0.0,
                output_price: 0.0,
                capabilities: vec![],
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
        let converted = self.convert_messages(messages);

        let request = OllamaChatRequest {
            model: model.to_string(),
            messages: converted,
            stream: false,
            options: options.temperature.map(|t| OllamaOptions {
                temperature: Some(t),
                ..Default::default()
            }),
        };

        debug!("Sending request to Ollama: model={}", model);

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::server_error(status.as_u16(), error_body));
        }

        let result: OllamaChatResponse = response.json().await?;
        self.parse_response(result)
    }

    async fn chat_stream(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<CompletionStream> {
        let options = options.unwrap_or_default();
        let converted = self.convert_messages(messages);

        let request = OllamaChatRequest {
            model: model.to_string(),
            messages: converted,
            stream: true,
            options: options.temperature.map(|t| OllamaOptions {
                temperature: Some(t),
                ..Default::default()
            }),
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_body = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::server_error(status.as_u16(), error_body));
        }

        let byte_stream = response.bytes_stream();

        let stream = byte_stream.filter_map(move |result| async move {
            match result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    for line in text.lines() {
                        if line.is_empty() {
                            continue;
                        }
                        let parsed: std::result::Result<OllamaStreamChunk, _> =
                            serde_json::from_str(line);
                        match parsed {
                            Ok(chunk) => {
                                if chunk.done {
                                    return Some(Ok(StreamEvent::End {
                                        stop_reason: StopReason::EndTurn,
                                        usage: Usage {
                                            input_tokens: chunk.prompt_eval_count.unwrap_or(0),
                                            output_tokens: chunk.eval_count.unwrap_or(0),
                                            cache_read_tokens: 0,
                                            cache_creation_tokens: 0,
                                        },
                                    }));
                                }
                                if !chunk.message.content.is_empty() {
                                    return Some(Ok(StreamEvent::ContentDelta {
                                        delta: chunk.message.content,
                                    }));
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse Ollama stream chunk: {}", e);
                            }
                        }
                    }
                    None
                }
                Err(e) => Some(Err(ProviderError::stream(e.to_string()))),
            }
        });

        Ok(Box::pin(stream))
    }

    async fn count_tokens(&self, _model: &str, messages: &[Message]) -> Result<TokenCount> {
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
            tools: false,
            vision: false,
            system_messages: true,
            max_context: Some(128_000),
            max_output: Some(16_384),
        }
    }
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<OllamaOptions>,
}

#[derive(Serialize, Deserialize)]
struct OllamaMessage {
    role: String,
    content: String,
}

#[derive(Serialize, Default)]
struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    model: String,
    created_at: Option<String>,
    message: OllamaMessage,
    done: bool,
    prompt_eval_count: Option<usize>,
    eval_count: Option<usize>,
}

#[derive(Deserialize)]
struct OllamaStreamChunk {
    model: String,
    created_at: Option<String>,
    message: OllamaMessage,
    done: bool,
    prompt_eval_count: Option<usize>,
    eval_count: Option<usize>,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaTagModel>,
}

#[derive(Deserialize)]
struct OllamaTagModel {
    name: String,
    details: Option<OllamaModelDetails>,
}

#[derive(Deserialize)]
struct OllamaModelDetails {
    family: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        let _guard = std::env::remove_var("OLLAMA_HOST");
        let provider = OllamaProvider::new();
        assert_eq!(provider.name(), "ollama");
        assert_eq!(provider.base_url, "http://localhost:11434");
    }

    #[test]
    fn test_provider_custom_url() {
        let provider = OllamaProvider::new().with_base_url("http://ollama.internal:11434");
        assert_eq!(provider.base_url, "http://ollama.internal:11434");
    }

    #[test]
    fn test_capabilities() {
        let provider = OllamaProvider::new();
        let caps = provider.capabilities();
        assert!(caps.streaming);
        assert!(!caps.tools);
        assert!(caps.system_messages);
    }
}
