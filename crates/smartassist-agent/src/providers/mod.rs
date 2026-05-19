//! Model provider integrations.
//!
//! This module re-exports provider types and implementations from
//! `smartassist_providers`, which is the canonical home for all provider
//! logic. The agent runtime uses the `Provider` trait from that crate.

// Re-export the Provider trait and all core types.
pub use smartassist_providers::{
    ChatOptions, ChatResponse, CompletionStream, ContentBlock, ImageSource, ImageSourceType,
    Message, MessageContent, ModelCapabilities, ModelInfo, ModelPricing, Provider,
    ProviderCapabilities, ProviderError, Result, Role, StopReason, StreamEvent,
    TokenCount, TokenUsage, ToolChoice, ToolDefinition,
};

// Re-export concrete provider types when their features are enabled.
#[cfg(feature = "anthropic")]
pub use smartassist_providers::anthropic::AnthropicProvider;

#[cfg(feature = "openai")]
pub use smartassist_providers::openai::OpenAIProvider;

#[cfg(feature = "google")]
pub use smartassist_providers::google::GoogleProvider;

#[cfg(feature = "ollama")]
pub use smartassist_providers::ollama::OllamaProvider;

#[cfg(feature = "deepseek")]
pub use smartassist_providers::deepseek::DeepSeekProvider;

#[cfg(feature = "moonshot")]
pub use smartassist_providers::moonshot::MoonshotProvider;

#[cfg(feature = "openrouter")]
pub use smartassist_providers::openrouter::OpenRouterProvider;

#[cfg(feature = "qwen")]
pub use smartassist_providers::qwen::QwenProvider;

#[cfg(feature = "zhipu")]
pub use smartassist_providers::zhipu::ZhipuProvider;