//! Models RPC method handlers.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use tracing::debug;

/// Model information.
#[derive(Debug, Serialize)]
pub struct ModelInfo {
    /// Model ID.
    pub id: String,

    /// Model name.
    pub name: String,

    /// Provider name.
    pub provider: String,

    /// Model description.
    pub description: Option<String>,

    /// Context window size.
    pub context_window: Option<u32>,

    /// Maximum output tokens.
    pub max_output_tokens: Option<u32>,

    /// Whether the model supports vision.
    pub supports_vision: bool,

    /// Whether the model supports tool use.
    pub supports_tools: bool,
}

/// Models list response.
#[derive(Debug, Serialize)]
pub struct ModelsListResponse {
    /// Available models.
    pub models: Vec<ModelInfo>,

    /// Default model ID.
    pub default_model: Option<String>,
}

/// Models list method handler.
pub struct ModelsListHandler {
    _context: Arc<HandlerContext>,
}

impl ModelsListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }

    /// Get list of available models.
    ///
    /// This is a static fallback shown when no live provider is queried.
    /// Anthropic, DeepSeek, Zhipu, and Poolside model ids are sourced from
    /// each provider's own `list_models()`; Ollama entries are a sample of
    /// the current `ollama.com/library` catalog (pullable, not a claim about
    /// what's actually installed -- `ollama.rs::list_models()` queries the
    /// live `/api/tags` for that). Qwen/Moonshot ids are unchanged pending a
    /// verified current source for those two providers specifically.
    fn get_available_models() -> Vec<ModelInfo> {
        vec![
            // Anthropic models (newest-capability first)
            ModelInfo {
                id: "claude-opus-5".to_string(),
                name: "Claude Opus 5".to_string(),
                provider: "anthropic".to_string(),
                description: Some("Most intelligent model, best for complex tasks".to_string()),
                context_window: Some(200000),
                max_output_tokens: Some(64000),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-sonnet-5".to_string(),
                name: "Claude Sonnet 5".to_string(),
                provider: "anthropic".to_string(),
                description: Some("Balanced model for most tasks".to_string()),
                context_window: Some(200000),
                max_output_tokens: Some(64000),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-haiku-4-5".to_string(),
                name: "Claude Haiku 4.5".to_string(),
                provider: "anthropic".to_string(),
                description: Some("Fastest model, good for simple tasks".to_string()),
                context_window: Some(200000),
                max_output_tokens: Some(8192),
                supports_vision: true,
                supports_tools: true,
            },
            // OpenAI models
            ModelInfo {
                id: "gpt-5.6-sol".to_string(),
                name: "GPT-5.6 Sol".to_string(),
                provider: "openai".to_string(),
                description: Some("OpenAI's flagship reasoning model".to_string()),
                context_window: Some(400000),
                max_output_tokens: Some(128000),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "gpt-5.6-terra".to_string(),
                name: "GPT-5.6 Terra".to_string(),
                provider: "openai".to_string(),
                description: Some("Balanced GPT-5.6 tier".to_string()),
                context_window: Some(400000),
                max_output_tokens: Some(64000),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "gpt-5.6-luna".to_string(),
                name: "GPT-5.6 Luna".to_string(),
                provider: "openai".to_string(),
                description: Some("Fast, low-cost GPT-5.6 tier".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(32000),
                supports_vision: true,
                supports_tools: true,
            },
            // Google Gemini models
            ModelInfo {
                id: "gemini-3.6-flash".to_string(),
                name: "Gemini 3.6 Flash".to_string(),
                provider: "google".to_string(),
                description: Some("Google's fast, current-generation model".to_string()),
                context_window: Some(1000000),
                max_output_tokens: Some(8192),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "gemini-3.1-pro-preview".to_string(),
                name: "Gemini 3.1 Pro".to_string(),
                provider: "google".to_string(),
                description: Some("Google's most capable model".to_string()),
                context_window: Some(1000000),
                max_output_tokens: Some(8192),
                supports_vision: true,
                supports_tools: true,
            },
            // DeepSeek models
            ModelInfo {
                id: "deepseek-v4-pro".to_string(),
                name: "DeepSeek V4 Pro".to_string(),
                provider: "deepseek".to_string(),
                description: Some("DeepSeek's most capable model".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(8192),
                supports_vision: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "deepseek-v4-flash".to_string(),
                name: "DeepSeek V4 Flash".to_string(),
                provider: "deepseek".to_string(),
                description: Some("DeepSeek's fast, low-cost model".to_string()),
                context_window: Some(64000),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
            // Qwen models
            ModelInfo {
                id: "qwen-max".to_string(),
                name: "Qwen Max".to_string(),
                provider: "qwen".to_string(),
                description: Some("Alibaba's most capable model".to_string()),
                context_window: Some(32000),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
            // Moonshot models
            ModelInfo {
                id: "moonshot-v1-128k".to_string(),
                name: "Moonshot 128K".to_string(),
                provider: "moonshot".to_string(),
                description: Some("Moonshot with 128k context".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
            // Zhipu/GLM models
            ModelInfo {
                id: "glm-5.2".to_string(),
                name: "GLM-5.2".to_string(),
                provider: "zhipu".to_string(),
                description: Some("Zhipu's latest, most capable GLM model".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(8192),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "glm-4.7-flash".to_string(),
                name: "GLM-4.7 Flash".to_string(),
                provider: "zhipu".to_string(),
                description: Some("Zhipu's fast, low-cost model".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
            // Poolside AI models (purpose-built coding models)
            ModelInfo {
                id: "poolside/laguna-s-2.1".to_string(),
                name: "Poolside Laguna S 2.1".to_string(),
                provider: "poolside".to_string(),
                description: Some("Poolside's purpose-built coding model".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(8192),
                supports_vision: false,
                supports_tools: true,
            },
            // Local models via Ollama (pullable; live-installed models come
            // from ollama.rs::list_models() querying /api/tags instead)
            ModelInfo {
                id: "llama4".to_string(),
                name: "Llama 4".to_string(),
                provider: "ollama".to_string(),
                description: Some("Meta's Llama 4 (local)".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "qwen3-coder".to_string(),
                name: "Qwen3 Coder".to_string(),
                provider: "ollama".to_string(),
                description: Some("Alibaba's Qwen3 Coder (local)".to_string()),
                context_window: Some(32000),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "deepseek-v4-flash:cloud".to_string(),
                name: "DeepSeek V4 Flash (Ollama Cloud)".to_string(),
                provider: "ollama".to_string(),
                description: Some("Datacenter-hosted via Ollama Cloud".to_string()),
                context_window: Some(64000),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
        ]
    }
}

#[async_trait]
impl MethodHandler for ModelsListHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Models list request");

        let response = ModelsListResponse {
            models: Self::get_available_models(),
            default_model: Some("claude-sonnet-5".to_string()),
        };

        serde_json::to_value(response).map_err(|e| GatewayError::Internal(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_models_list() {
        let models = ModelsListHandler::get_available_models();
        assert!(!models.is_empty());

        // Check that we have models from different providers
        let providers: Vec<_> = models.iter().map(|m| m.provider.as_str()).collect();
        assert!(providers.contains(&"anthropic"));
        assert!(providers.contains(&"openai"));
        assert!(providers.contains(&"deepseek"));
    }
}
