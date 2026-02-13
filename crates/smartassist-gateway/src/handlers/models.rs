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
    fn get_available_models() -> Vec<ModelInfo> {
        vec![
            // Anthropic models
            ModelInfo {
                id: "claude-3-5-sonnet-20241022".to_string(),
                name: "Claude 3.5 Sonnet".to_string(),
                provider: "anthropic".to_string(),
                description: Some("Most intelligent model, best for complex tasks".to_string()),
                context_window: Some(200000),
                max_output_tokens: Some(8192),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-3-5-haiku-20241022".to_string(),
                name: "Claude 3.5 Haiku".to_string(),
                provider: "anthropic".to_string(),
                description: Some("Fastest model, good for simple tasks".to_string()),
                context_window: Some(200000),
                max_output_tokens: Some(8192),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "claude-3-opus-20240229".to_string(),
                name: "Claude 3 Opus".to_string(),
                provider: "anthropic".to_string(),
                description: Some("Powerful model for nuanced tasks".to_string()),
                context_window: Some(200000),
                max_output_tokens: Some(4096),
                supports_vision: true,
                supports_tools: true,
            },
            // OpenAI models
            ModelInfo {
                id: "gpt-4o".to_string(),
                name: "GPT-4o".to_string(),
                provider: "openai".to_string(),
                description: Some("Latest GPT-4 model with vision".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(4096),
                supports_vision: true,
                supports_tools: true,
            },
            ModelInfo {
                id: "gpt-4-turbo".to_string(),
                name: "GPT-4 Turbo".to_string(),
                provider: "openai".to_string(),
                description: Some("GPT-4 Turbo with 128k context".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(4096),
                supports_vision: true,
                supports_tools: true,
            },
            // DeepSeek models
            ModelInfo {
                id: "deepseek-chat".to_string(),
                name: "DeepSeek Chat".to_string(),
                provider: "deepseek".to_string(),
                description: Some("DeepSeek's general chat model".to_string()),
                context_window: Some(64000),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "deepseek-coder".to_string(),
                name: "DeepSeek Coder".to_string(),
                provider: "deepseek".to_string(),
                description: Some("DeepSeek's coding-specialized model".to_string()),
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
                id: "glm-4".to_string(),
                name: "GLM-4".to_string(),
                provider: "zhipu".to_string(),
                description: Some("Zhipu's latest GLM model".to_string()),
                context_window: Some(128000),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "glm-4v".to_string(),
                name: "GLM-4V".to_string(),
                provider: "zhipu".to_string(),
                description: Some("GLM-4 with vision support".to_string()),
                context_window: Some(2000),
                max_output_tokens: Some(1024),
                supports_vision: true,
                supports_tools: true,
            },
            // Local models via Ollama
            ModelInfo {
                id: "llama3.2".to_string(),
                name: "Llama 3.2".to_string(),
                provider: "ollama".to_string(),
                description: Some("Meta's Llama 3.2 (local)".to_string()),
                context_window: Some(8192),
                max_output_tokens: Some(4096),
                supports_vision: false,
                supports_tools: true,
            },
            ModelInfo {
                id: "qwen2.5".to_string(),
                name: "Qwen 2.5".to_string(),
                provider: "ollama".to_string(),
                description: Some("Alibaba's Qwen 2.5 (local)".to_string()),
                context_window: Some(32000),
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
            default_model: Some("claude-3-5-sonnet-20241022".to_string()),
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
