//! Provider-related types for model interactions.

use serde::{Deserialize, Serialize};

/// Provider capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderCapabilities {
    /// Supports streaming responses.
    #[serde(default)]
    pub streaming: bool,

    /// Supports function/tool calling.
    #[serde(default)]
    pub tools: bool,

    /// Supports vision/image input.
    #[serde(default)]
    pub vision: bool,

    /// Supports system messages.
    #[serde(default)]
    pub system_messages: bool,

    /// Maximum context window (tokens).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_context: Option<usize>,

    /// Maximum output tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output: Option<usize>,
}

/// Token count result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenCount {
    /// Number of tokens.
    pub count: usize,

    /// Model used for counting.
    pub model: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_capabilities_default() {
        let caps = ProviderCapabilities::default();
        assert!(!caps.streaming);
        assert!(!caps.tools);
        assert!(!caps.vision);
        assert!(!caps.system_messages);
        assert!(caps.max_context.is_none());
        assert!(caps.max_output.is_none());
    }
}