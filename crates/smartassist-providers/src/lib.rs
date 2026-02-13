//! Model provider implementations for SmartAssist.
//!
//! This crate provides implementations for various AI model providers:
//! - Anthropic (Claude models)
//! - OpenAI (GPT models)
//! - Google (Gemini models)
//!
//! # Example
//!
//! ```rust,ignore
//! use smartassist_providers::{Provider, AnthropicProvider, Message, MessageRole};
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
//!     println!("Response: {}", response.content);
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

pub use error::{ProviderError, Result};
pub use types::*;

use async_trait::async_trait;
use futures::Stream;
use std::pin::Pin;

/// Stream of completion events for streaming responses.
pub type CompletionStream = Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send>>;

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

/// Provider capabilities.
#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilities {
    /// Supports streaming responses.
    pub streaming: bool,

    /// Supports function/tool calling.
    pub tools: bool,

    /// Supports vision/image input.
    pub vision: bool,

    /// Supports system messages.
    pub system_messages: bool,

    /// Maximum context window (tokens).
    pub max_context: Option<usize>,

    /// Maximum output tokens.
    pub max_output: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_capabilities() {
        let caps = ProviderCapabilities {
            streaming: true,
            tools: true,
            vision: true,
            system_messages: true,
            max_context: Some(200_000),
            max_output: Some(8192),
        };

        assert!(caps.streaming);
        assert!(caps.tools);
        assert_eq!(caps.max_context, Some(200_000));
    }
}
