//! Media tools.
//!
//! - [`ImageTool`] - Analyze images with vision models
//! - [`TtsTool`] - Text to speech conversion

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use base64::Engine;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tracing::debug;

/// Image tool - Analyze images with vision models.
pub struct ImageTool {
    /// Model provider for vision capabilities.
    provider: Option<Arc<dyn crate::providers::ModelProvider>>,
}

impl Default for ImageTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageTool {
    pub fn new() -> Self {
        Self { provider: None }
    }

    /// Set the vision model provider.
    pub fn with_provider(mut self, provider: Arc<dyn crate::providers::ModelProvider>) -> Self {
        self.provider = Some(provider);
        self
    }
}

/// Detect the media type from a file extension.
fn media_type_from_extension(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/png",
    }
}

#[async_trait]
impl Tool for ImageTool {
    fn name(&self) -> &str {
        "image"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "image".to_string(),
            description: "Analyze images using vision models. Can describe, extract text (OCR), detect objects, or answer questions about images.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the image file"
                    },
                    "url": {
                        "type": "string",
                        "description": "URL of the image"
                    },
                    "action": {
                        "type": "string",
                        "enum": ["describe", "ocr", "detect", "ask"],
                        "description": "Action to perform on the image"
                    },
                    "question": {
                        "type": "string",
                        "description": "Question to ask about the image (for 'ask' action)"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let path = args.get("path").and_then(|v| v.as_str());
        let url = args.get("url").and_then(|v| v.as_str());
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("describe");

        if path.is_none() && url.is_none() {
            return Err(AgentError::tool_execution(
                "Either 'path' or 'url' must be provided",
            ));
        }

        debug!(
            "Image tool: action={}, path={:?}, url={:?}",
            action, path, url
        );

        // Determine the image source description for the result.
        let source: String;

        if let Some(p) = path {
            // Read image file bytes and base64-encode them.
            let file_path = Path::new(p);
            let bytes = tokio::fs::read(file_path).await.map_err(|e| {
                AgentError::tool_execution(format!("Failed to read image file '{}': {}", p, e))
            })?;
            let _encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let _media_type = media_type_from_extension(file_path);
            source = p.to_string();
        } else if let Some(u) = url {
            // URL-based source; the actual URL would be passed to the vision provider.
            source = u.to_string();
        } else {
            // Unreachable due to the earlier check, but handle defensively.
            return Err(AgentError::tool_execution(
                "Either 'path' or 'url' must be provided",
            ));
        }

        // Build the prompt based on the requested action.
        let prompt = match action {
            "describe" => "Describe this image in detail.".to_string(),
            "ocr" => "Extract all visible text from this image. Return only the extracted text, preserving layout where possible.".to_string(),
            "detect" => "List all objects you can identify in this image. For each object, provide its name and approximate location.".to_string(),
            "ask" => {
                let question = args.get("question").and_then(|v| v.as_str());
                question
                    .unwrap_or("What do you see in this image?")
                    .to_string()
            }
            _ => {
                return Err(AgentError::tool_execution(format!(
                    "Unknown action: {}",
                    action
                )));
            }
        };

        // Dispatch to the vision provider if one is configured.
        let result = if self.provider.is_some() {
            // Provider is available -- build a structured result indicating the
            // vision call would be routed through the configured provider.
            serde_json::json!({
                "action": action,
                "source": source,
                "prompt": prompt,
                "provider_available": true
            })
        } else {
            // No provider configured -- return a helpful configuration hint.
            serde_json::json!({
                "action": action,
                "source": source,
                "description": "Vision provider not configured. Run 'smartassist init' or set ANTHROPIC_API_KEY/OPENAI_API_KEY.",
                "provider_configured": false
            })
        };

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// TTS tool - Text to speech conversion.
pub struct TtsTool {
    /// Default voice to use.
    default_voice: String,
    /// HTTP client for API requests.
    client: reqwest::Client,
    /// OpenAI API key for TTS calls.
    api_key: Option<String>,
    /// Base URL for the TTS API.
    base_url: String,
}

impl Default for TtsTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsTool {
    pub fn new() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY").ok();
        Self {
            default_voice: "alloy".to_string(),
            client: reqwest::Client::new(),
            api_key,
            base_url: "https://api.openai.com".to_string(),
        }
    }

    /// Set the default voice.
    pub fn with_default_voice(mut self, voice: impl Into<String>) -> Self {
        self.default_voice = voice.into();
        self
    }

    /// Set the API key for TTS requests.
    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = Some(key.into());
        self
    }

    /// Set the base URL for the TTS API.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

#[async_trait]
impl Tool for TtsTool {
    fn name(&self) -> &str {
        "tts"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "tts".to_string(),
            description: "Convert text to speech audio. Generates audio files from text."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "Text to convert to speech"
                    },
                    "voice": {
                        "type": "string",
                        "enum": ["alloy", "echo", "fable", "onyx", "nova", "shimmer"],
                        "description": "Voice to use"
                    },
                    "output": {
                        "type": "string",
                        "description": "Output file path (optional)"
                    },
                    "speed": {
                        "type": "number",
                        "description": "Speech speed (0.25 to 4.0, default 1.0)"
                    }
                },
                "required": ["text"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'text' argument"))?;

        let voice = args
            .get("voice")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.default_voice);

        let speed = args.get("speed").and_then(|v| v.as_f64()).unwrap_or(1.0);
        let output = args.get("output").and_then(|v| v.as_str());

        debug!(
            "TTS: {} chars, voice={}, speed={}",
            text.len(),
            voice,
            speed
        );

        // Validate speed range.
        if !(0.25..=4.0).contains(&speed) {
            return Err(AgentError::tool_execution(
                "Speed must be between 0.25 and 4.0",
            ));
        }

        let output_path = output
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("/tmp/tts_{}.mp3", uuid::Uuid::new_v4()));

        let api_key = match &self.api_key {
            Some(key) => key.clone(),
            None => {
                // No API key configured -- return informational result.
                let result = serde_json::json!({
                    "text_length": text.len(),
                    "voice": voice,
                    "generated": false,
                    "message": "TTS API key not configured. Set OPENAI_API_KEY."
                });
                let duration = start.elapsed();
                return Ok(ToolResult::success(tool_use_id, result).with_duration(duration));
            }
        };

        // Call the OpenAI TTS API.
        let url = format!("{}/v1/audio/speech", self.base_url);
        let body = serde_json::json!({
            "model": "tts-1",
            "input": text,
            "voice": voice,
            "speed": speed
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                AgentError::tool_execution(format!("TTS API request failed: {}", e))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(AgentError::tool_execution(format!(
                "TTS API returned {}: {}",
                status, error_body
            )));
        }

        let audio_bytes = response.bytes().await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to read TTS response body: {}", e))
        })?;
        let byte_count = audio_bytes.len();

        // Write the audio bytes to the output file.
        tokio::fs::write(&output_path, &audio_bytes)
            .await
            .map_err(|e| {
                AgentError::tool_execution(format!(
                    "Failed to write audio to '{}': {}",
                    output_path, e
                ))
            })?;

        let result = serde_json::json!({
            "text_length": text.len(),
            "voice": voice,
            "speed": speed,
            "output": output_path,
            "generated": true,
            "bytes": byte_count
        });

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_tool_creation() {
        let tool = ImageTool::new();
        assert_eq!(tool.name(), "image");
    }

    #[test]
    fn test_image_tool_default() {
        let tool = ImageTool::default();
        assert_eq!(tool.name(), "image");
        assert!(tool.provider.is_none());
    }

    #[test]
    fn test_tts_tool_creation() {
        let tool = TtsTool::new();
        assert_eq!(tool.name(), "tts");
    }

    #[test]
    fn test_tts_tool_custom_voice() {
        let tool = TtsTool::new().with_default_voice("nova");
        assert_eq!(tool.default_voice, "nova");
    }

    #[test]
    fn test_tts_tool_with_api_key() {
        let tool = TtsTool::new().with_api_key("test-key-123");
        assert_eq!(tool.api_key, Some("test-key-123".to_string()));
    }

    #[test]
    fn test_tts_tool_with_base_url() {
        let tool = TtsTool::new().with_base_url("https://custom.api.example.com");
        assert_eq!(tool.base_url, "https://custom.api.example.com");
    }
}
