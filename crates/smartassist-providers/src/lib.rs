//! Model provider implementations for SmartAssist.
//!
//! This crate provides implementations for various AI model providers,
//! using canonical types from `smartassist_core::types` for the
//! provider trait interface.
//!
//! # Example
//!
//! ```rust,ignore
//! use smartassist_providers::{Provider, AnthropicProvider};
//! use smartassist_core::types::{Message, ChatOptions, ToolChoice};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let provider = AnthropicProvider::new("your-api-key")?;
//!
//!     let messages = vec![
//!         Message::user("Hello, Claude!"),
//!     ];
//!
//!     let response = provider.chat("claude-sonnet-4-20250514", &messages, None).await?;
//!     println!("Response: {}", response.to_text());
//!
//!     Ok(())
//! }
//! ```

mod error;
mod types;

#[cfg(feature = "anthropic")]
pub mod anthropic;

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "google")]
pub mod google;

#[cfg(feature = "image-openai")]
pub mod image_openai;

#[cfg(feature = "image-ollama")]
pub mod image_ollama;

#[cfg(feature = "image-fal")]
pub mod image_fal;

#[cfg(feature = "video-openai")]
pub mod video_openai;

#[cfg(feature = "video-dashscope")]
pub mod video_dashscope;

#[cfg(feature = "music-openai")]
pub mod music_openai;

#[cfg(feature = "tts-openai")]
pub mod tts_openai;

#[cfg(feature = "tts-elevenlabs")]
pub mod tts_elevenlabs;

#[cfg(feature = "tts-azure")]
pub mod tts_azure;

#[cfg(feature = "tts-local")]
pub mod tts_local;

#[cfg(feature = "ollama")]
pub mod ollama;

#[cfg(feature = "deepseek")]
pub mod deepseek;

#[cfg(feature = "moonshot")]
pub mod moonshot;

#[cfg(feature = "openrouter")]
pub mod openrouter;

#[cfg(feature = "qwen")]
pub mod qwen;

#[cfg(feature = "zhipu")]
pub mod zhipu;

pub mod media;

pub use error::{ProviderError, Result};

// Re-export canonical types from smartassist_core for convenience.
// These are THE canonical types — the providers crate does not define its own.
pub use smartassist_core::types::{
    ChatOptions, ChatResponse, ContentBlock, ImageSource, ImageSourceType, Message, MessageContent,
    ModelCapabilities, ModelInfo, ModelPricing, ProviderCapabilities, Role, StopReason,
    StreamEvent, TokenCount, TokenUsage, ToolChoice, ToolDefinition,
};

// Re-export provider-specific types that have no core equivalent.
pub use types::{CompletionStream};

use async_trait::async_trait;

/// Stream of completion events for streaming responses.
// Note: CompletionStream is defined in types.rs as a type alias.

/// A model provider that can generate completions.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Get provider name.
    fn name(&self) -> &str;

    /// List available models.
    async fn list_models(&self) -> Result<Vec<ModelInfo>>;

    /// Check if a model is available.
    async fn is_model_available(&self, model: &str) -> Result<bool> {
        let models = self.list_models().await?;
        Ok(models.iter().any(|m| m.id == model))
    }

    /// Generate a chat completion.
    async fn chat(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<ChatResponse>;

    /// Generate a streaming chat completion.
    async fn chat_stream(
        &self,
        model: &str,
        messages: &[Message],
        options: Option<ChatOptions>,
    ) -> Result<CompletionStream>;

    /// Count tokens in a message.
    async fn count_tokens(&self, model: &str, messages: &[Message]) -> Result<TokenCount>;

    /// Get model capabilities.
    fn capabilities(&self) -> ProviderCapabilities;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_capabilities_default() {
        let caps = ProviderCapabilities::default();
        assert!(!caps.streaming);
        assert!(!caps.tools);
        assert!(caps.max_context.is_none());
    }
}