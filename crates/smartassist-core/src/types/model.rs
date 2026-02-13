//! Model reference and metadata types.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Reference to a model (provider/model-id).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelRef {
    /// Provider name (anthropic, openai, etc.).
    pub provider: String,

    /// Model ID within the provider.
    pub model_id: String,
}

impl ModelRef {
    /// Create a new model reference.
    pub fn new(provider: impl Into<String>, model_id: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model_id: model_id.into(),
        }
    }

    /// Parse a model reference from "provider/model-id" format.
    pub fn parse(s: &str) -> Result<Self, ModelRefParseError> {
        let parts: Vec<&str> = s.splitn(2, '/').collect();
        if parts.len() != 2 {
            return Err(ModelRefParseError::InvalidFormat(s.to_string()));
        }
        if parts[0].is_empty() || parts[1].is_empty() {
            return Err(ModelRefParseError::InvalidFormat(s.to_string()));
        }
        Ok(Self::new(parts[0], parts[1]))
    }
}

impl fmt::Display for ModelRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.provider, self.model_id)
    }
}

impl TryFrom<&str> for ModelRef {
    type Error = ModelRefParseError;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::parse(s)
    }
}

impl TryFrom<String> for ModelRef {
    type Error = ModelRefParseError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::parse(&s)
    }
}

/// Error parsing a model reference.
#[derive(Debug, Clone)]
pub enum ModelRefParseError {
    InvalidFormat(String),
}

impl fmt::Display for ModelRefParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFormat(s) => {
                write!(f, "Invalid model reference format: '{}', expected 'provider/model-id'", s)
            }
        }
    }
}

impl std::error::Error for ModelRefParseError {}

/// Information about a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model ID.
    pub id: String,

    /// Provider name.
    pub provider: String,

    /// Display name.
    pub display_name: String,

    /// Model capabilities.
    #[serde(default)]
    pub capabilities: ModelCapabilities,

    /// Context window size (in tokens).
    pub context_window: usize,

    /// Maximum output tokens.
    pub max_output_tokens: usize,

    /// Pricing information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing: Option<ModelPricing>,
}

/// Model capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    /// Supports vision/images.
    #[serde(default)]
    pub vision: bool,

    /// Supports tool use.
    #[serde(default)]
    pub tool_use: bool,

    /// Supports extended thinking.
    #[serde(default)]
    pub extended_thinking: bool,

    /// Supports streaming.
    #[serde(default)]
    pub streaming: bool,

    /// Supports JSON mode.
    #[serde(default)]
    pub json_mode: bool,
}

/// Model pricing (per million tokens).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Price per 1M input tokens.
    pub input_per_1m: f64,

    /// Price per 1M output tokens.
    pub output_per_1m: f64,

    /// Price per 1M cache creation tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_per_1m: Option<f64>,

    /// Price per 1M cache read tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_per_1m: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_ref_parse() {
        let ref1 = ModelRef::parse("anthropic/claude-3-opus").unwrap();
        assert_eq!(ref1.provider, "anthropic");
        assert_eq!(ref1.model_id, "claude-3-opus");

        let ref2 = ModelRef::parse("openai/gpt-4-turbo").unwrap();
        assert_eq!(ref2.provider, "openai");
        assert_eq!(ref2.model_id, "gpt-4-turbo");
    }

    #[test]
    fn test_model_ref_parse_invalid() {
        assert!(ModelRef::parse("invalid").is_err());
        assert!(ModelRef::parse("/model").is_err());
        assert!(ModelRef::parse("provider/").is_err());
    }

    #[test]
    fn test_model_ref_display() {
        let ref1 = ModelRef::new("anthropic", "claude-3-opus");
        assert_eq!(ref1.to_string(), "anthropic/claude-3-opus");
    }
}
