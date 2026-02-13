//! Model provider integrations.
//!
//! This module provides integrations with various AI model providers:
//!
//! - [`AnthropicProvider`] - Claude models (Opus, Sonnet, Haiku)
//! - [`OpenAIProvider`] - GPT-4o, GPT-4, GPT-3.5
//! - [`OllamaProvider`] - Local models (Llama, Mistral, Qwen, etc.)
//! - [`OpenRouterProvider`] - Unified access to 100+ models
//! - [`DeepSeekProvider`] - DeepSeek-V3, DeepSeek Coder, DeepSeek Reasoner
//! - [`MoonshotProvider`] - Moonshot/Kimi models (8k, 32k, 128k context)
//! - [`QwenProvider`] - Alibaba Qwen models via DashScope
//! - [`ZhipuProvider`] - Zhipu AI GLM-4 models

pub mod anthropic;
pub mod deepseek;
pub mod moonshot;
pub mod ollama;
pub mod openai;
pub mod openrouter;
pub mod qwen;
pub mod zhipu;

use crate::Result;
use async_trait::async_trait;
use futures::Stream;
use smartassist_core::types::{Message, MessageContent, TokenUsage, ToolDefinition};
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Response from a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    /// Generated content.
    pub content: MessageContent,

    /// Stop reason.
    pub stop_reason: Option<String>,

    /// Token usage.
    pub token_usage: TokenUsage,
}

/// Streaming event from model generation.
#[derive(Debug, Clone)]
pub enum StreamEvent {
    /// Stream started.
    Start,

    /// Text content.
    Text(String),

    /// Thinking text (for extended thinking).
    Thinking(String),

    /// Tool use.
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Token usage update.
    Usage(TokenUsage),

    /// Stream completed.
    Done,

    /// Error occurred.
    Error(String),
}

/// Trait for model providers.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Get the provider name.
    fn name(&self) -> &str;

    /// Get the current model.
    fn model(&self) -> &str;

    /// Generate a response (non-streaming).
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Result<ModelResponse>;

    /// Generate a response (streaming).
    fn complete_stream(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + '_>>;

    /// Get the context window size for this model (in tokens).
    fn context_limit(&self) -> usize {
        100_000 // Conservative default
    }
}

pub use anthropic::AnthropicProvider;
pub use deepseek::DeepSeekProvider;
pub use moonshot::MoonshotProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;
pub use openrouter::OpenRouterProvider;
pub use qwen::QwenProvider;
pub use zhipu::ZhipuProvider;
