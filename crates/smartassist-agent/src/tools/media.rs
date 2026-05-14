//! Media tools.
//!
//! - [`ImageTool`] - Analyze images with vision models
//! - [`ImageGenerateTool`] - Generate images from text prompts
//! - [`VideoGenerateTool`] - Generate videos from text prompts
//! - [`MusicGenerateTool`] - Generate music from text prompts
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

        let source: String;

        if let Some(p) = path {
            let file_path = Path::new(p);
            let bytes = tokio::fs::read(file_path).await.map_err(|e| {
                AgentError::tool_execution(format!("Failed to read image file '{}': {}", p, e))
            })?;
            let _encoded = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let _media_type = media_type_from_extension(file_path);
            source = p.to_string();
        } else if let Some(u) = url {
            source = u.to_string();
        } else {
            return Err(AgentError::tool_execution(
                "Either 'path' or 'url' must be provided",
            ));
        }

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

        let result = if self.provider.is_some() {
            serde_json::json!({
                "action": action,
                "source": source,
                "prompt": prompt,
                "provider_available": true
            })
        } else {
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

/// Image generation tool - Generate images from text prompts.
pub struct ImageGenerateTool {
    registry: Arc<smartassist_providers::media::ImageProviderRegistry>,
}

impl ImageGenerateTool {
    /// Create a new image generation tool.
    pub fn new(registry: Arc<smartassist_providers::media::ImageProviderRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for ImageGenerateTool {
    fn name(&self) -> &str {
        "image_generate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "image_generate".to_string(),
            description: "Generate images from text descriptions using AI models. Supports DALL-E, FLUX, and local models.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "Text description of the image to generate"
                    },
                    "provider": {
                        "type": "string",
                        "enum": ["openai_image", "fal_image", "ollama_image"],
                        "description": "Provider to use (defaults to first available)"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model ID (e.g. dall-e-3, fal-ai/flux/dev)"
                    },
                    "size": {
                        "type": "string",
                        "enum": ["256x256", "512x512", "1024x1024", "1024x1792", "1792x1024"],
                        "description": "Image size"
                    },
                    "n": {
                        "type": "integer",
                        "description": "Number of images to generate (1-10)"
                    },
                    "quality": {
                        "type": "string",
                        "enum": ["standard", "hd"],
                        "description": "Image quality"
                    },
                    "style": {
                        "type": "string",
                        "enum": ["vivid", "natural"],
                        "description": "Image style"
                    }
                },
                "required": ["prompt"]
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

        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'prompt' argument"))?;

        let provider_name = args.get("provider").and_then(|v| v.as_str());
        let model = args.get("model").and_then(|v| v.as_str());
        let size = args.get("size").and_then(|v| v.as_str());
        let n = args.get("n").and_then(|v| v.as_u64()).map(|n| n as usize);
        let quality = args.get("quality").and_then(|v| v.as_str());
        let style = args.get("style").and_then(|v| v.as_str());

        let providers = self.registry.list().await;
        if providers.is_empty() {
            return Ok(ToolResult::success(tool_use_id, serde_json::json!({
                "generated": false,
                "message": "No image generation providers configured. Set OPENAI_API_KEY or FAL_API_KEY."
            })).with_duration(start.elapsed()));
        }

        let provider_name = provider_name
            .or_else(|| providers.first().map(|s| s.as_str()))
            .unwrap_or("openai_image");

        let provider = match self.registry.get(provider_name).await {
            Some(p) => p,
            None => {
                return Err(AgentError::tool_execution(format!(
                    "Provider '{}' not found. Available: {}",
                    provider_name,
                    providers.join(", ")
                )));
            }
        };

        let request = smartassist_providers::media::ImageGenerationRequest {
            prompt: prompt.to_string(),
            size: size.map(|s| s.to_string()),
            n,
            quality: quality.map(|s| s.to_string()),
            style: style.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            ..Default::default()
        };

        let media = provider.generate(request).await.map_err(|e| {
            AgentError::tool_execution(format!("Image generation failed: {}", e))
        })?;

        let urls: Vec<String> = media
            .iter()
            .filter_map(|m| m.url.clone())
            .collect();
        let paths: Vec<String> = media
            .iter()
            .filter_map(|m| m.path.clone())
            .collect();

        let result = serde_json::json!({
            "generated": !media.is_empty(),
            "count": media.len(),
            "urls": urls,
            "paths": paths,
            "provider": provider_name,
        });

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Video generation tool - Generate videos from text prompts.
pub struct VideoGenerateTool {
    registry: Arc<smartassist_providers::media::VideoProviderRegistry>,
}

impl VideoGenerateTool {
    /// Create a new video generation tool.
    pub fn new(registry: Arc<smartassist_providers::media::VideoProviderRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for VideoGenerateTool {
    fn name(&self) -> &str {
        "video_generate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "video_generate".to_string(),
            description: "Generate videos from text descriptions using AI models. Supports Sora and DashScope.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "Text description of the video to generate"
                    },
                    "provider": {
                        "type": "string",
                        "enum": ["openai_video", "dashscope_video"],
                        "description": "Provider to use"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model ID"
                    },
                    "duration": {
                        "type": "integer",
                        "description": "Duration in seconds"
                    },
                    "resolution": {
                        "type": "string",
                        "enum": ["360p", "480p", "720p", "1080p"],
                        "description": "Video resolution"
                    },
                    "aspect_ratio": {
                        "type": "string",
                        "enum": ["16:9", "9:16", "1:1", "4:3"],
                        "description": "Aspect ratio"
                    }
                },
                "required": ["prompt"]
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

        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'prompt' argument"))?;

        let provider_name = args.get("provider").and_then(|v| v.as_str());
        let model = args.get("model").and_then(|v| v.as_str());
        let duration = args.get("duration").and_then(|v| v.as_u64()).map(|d| d as u32);
        let resolution = args.get("resolution").and_then(|v| v.as_str());
        let aspect_ratio = args.get("aspect_ratio").and_then(|v| v.as_str());

        let providers = self.registry.list().await;
        if providers.is_empty() {
            return Ok(ToolResult::success(tool_use_id, serde_json::json!({
                "generated": false,
                "message": "No video generation providers configured."
            })).with_duration(start.elapsed()));
        }

        let provider_name = provider_name
            .or_else(|| providers.first().map(|s| s.as_str()))
            .unwrap_or("openai_video");

        let provider = match self.registry.get(provider_name).await {
            Some(p) => p,
            None => {
                return Err(AgentError::tool_execution(format!(
                    "Provider '{}' not found. Available: {}",
                    provider_name,
                    providers.join(", ")
                )));
            }
        };

        let request = smartassist_providers::media::VideoGenerationRequest {
            prompt: prompt.to_string(),
            duration_seconds: duration,
            resolution: resolution.map(|s| s.to_string()),
            aspect_ratio: aspect_ratio.map(|s| s.to_string()),
            model: model.map(|s| s.to_string()),
            ..Default::default()
        };

        let media = provider.generate(request).await.map_err(|e| {
            AgentError::tool_execution(format!("Video generation failed: {}", e))
        })?;

        let urls: Vec<String> = media
            .iter()
            .filter_map(|m| m.url.clone())
            .collect();

        let result = serde_json::json!({
            "generated": !media.is_empty(),
            "count": media.len(),
            "urls": urls,
            "provider": provider_name,
        });

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Music generation tool - Generate music from text prompts.
pub struct MusicGenerateTool {
    registry: Arc<smartassist_providers::media::MusicProviderRegistry>,
}

impl MusicGenerateTool {
    /// Create a new music generation tool.
    pub fn new(registry: Arc<smartassist_providers::media::MusicProviderRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for MusicGenerateTool {
    fn name(&self) -> &str {
        "music_generate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "music_generate".to_string(),
            description: "Generate music and audio from text descriptions using AI models.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "Text description of the music to generate"
                    },
                    "provider": {
                        "type": "string",
                        "enum": ["openai_music"],
                        "description": "Provider to use"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model ID"
                    },
                    "duration": {
                        "type": "integer",
                        "description": "Duration in seconds"
                    },
                    "genre": {
                        "type": "string",
                        "description": "Music genre"
                    },
                    "tempo": {
                        "type": "integer",
                        "description": "Tempo in BPM"
                    }
                },
                "required": ["prompt"]
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

        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'prompt' argument"))?;

        let provider_name = args.get("provider").and_then(|v| v.as_str());
        let model = args.get("model").and_then(|v| v.as_str());
        let duration = args.get("duration").and_then(|v| v.as_u64()).map(|d| d as u32);
        let genre = args.get("genre").and_then(|v| v.as_str());
        let tempo = args.get("tempo").and_then(|v| v.as_u64()).map(|t| t as u32);

        let providers = self.registry.list().await;
        if providers.is_empty() {
            return Ok(ToolResult::success(tool_use_id, serde_json::json!({
                "generated": false,
                "message": "No music generation providers configured."
            })).with_duration(start.elapsed()));
        }

        let provider_name = provider_name
            .or_else(|| providers.first().map(|s| s.as_str()))
            .unwrap_or("openai_music");

        let provider = match self.registry.get(provider_name).await {
            Some(p) => p,
            None => {
                return Err(AgentError::tool_execution(format!(
                    "Provider '{}' not found. Available: {}",
                    provider_name,
                    providers.join(", ")
                )));
            }
        };

        let request = smartassist_providers::media::MusicGenerationRequest {
            prompt: prompt.to_string(),
            duration_seconds: duration,
            genre: genre.map(|s| s.to_string()),
            tempo,
            model: model.map(|s| s.to_string()),
            ..Default::default()
        };

        let media = provider.generate(request).await.map_err(|e| {
            AgentError::tool_execution(format!("Music generation failed: {}", e))
        })?;

        let urls: Vec<String> = media
            .iter()
            .filter_map(|m| m.url.clone())
            .collect();

        let result = serde_json::json!({
            "generated": !media.is_empty(),
            "count": media.len(),
            "urls": urls,
            "provider": provider_name,
        });

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
    /// Optional TTS provider registry.
    registry: Option<Arc<smartassist_providers::media::TtsProviderRegistry>>,
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
            registry: None,
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

    /// Set the TTS provider registry.
    pub fn with_registry(
        mut self,
        registry: Arc<smartassist_providers::media::TtsProviderRegistry>,
    ) -> Self {
        self.registry = Some(registry);
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
            description: "Convert text to speech audio. Generates audio files from text. Supports OpenAI, ElevenLabs, Azure, and local TTS."
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
                        "description": "Voice to use"
                    },
                    "provider": {
                        "type": "string",
                        "enum": ["openai_tts", "elevenlabs_tts", "azure_tts", "local_tts"],
                        "description": "TTS provider to use"
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
        let provider_name = args.get("provider").and_then(|v| v.as_str());

        debug!(
            "TTS: {} chars, voice={}, speed={}",
            text.len(),
            voice,
            speed
        );

        if !(0.25..=4.0).contains(&speed) {
            return Err(AgentError::tool_execution(
                "Speed must be between 0.25 and 4.0",
            ));
        }

        // Try provider registry first if available and a provider is specified
        if let Some(registry) = &self.registry {
            let providers = registry.list().await;
            if !providers.is_empty() {
                let name = provider_name
                    .or_else(|| providers.first().map(|s| s.as_str()))
                    .unwrap_or("openai_tts");

                if let Some(provider) = registry.get(name).await {
                    let request = smartassist_providers::media::TtsRequest {
                        text: text.to_string(),
                        voice: Some(voice.to_string()),
                        speed: Some(speed as f32),
                        format: output.map(|s| {
                            std::path::Path::new(s)
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("mp3")
                                .to_string()
                        }),
                        model: None,
                        extra: std::collections::HashMap::new(),
                    };

                    let media = provider.speak(request).await.map_err(|e| {
                        AgentError::tool_execution(format!("TTS failed: {}", e))
                    })?;

                    let result = serde_json::json!({
                        "text_length": text.len(),
                        "voice": voice,
                        "speed": speed,
                        "provider": name,
                        "generated": true,
                        "url": media.url,
                        "path": media.path,
                        "mime_type": media.mime_type,
                        "size_bytes": media.size_bytes,
                    });
                    let duration = start.elapsed();
                    return Ok(ToolResult::success(tool_use_id, result).with_duration(duration));
                }
            }
        }

        // Fallback to direct OpenAI API implementation
        let output_path = output
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("/tmp/tts_{}.mp3", uuid::Uuid::new_v4()));

        let api_key = match &self.api_key {
            Some(key) => key.clone(),
            None => {
                let result = serde_json::json!({
                    "text_length": text.len(),
                    "voice": voice,
                    "generated": false,
                    "message": "TTS API key not configured. Set OPENAI_API_KEY or configure a provider registry."
                });
                let duration = start.elapsed();
                return Ok(ToolResult::success(tool_use_id, result).with_duration(duration));
            }
        };

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

    #[test]
    fn test_image_generate_tool_creation() {
        let registry = Arc::new(smartassist_providers::media::ImageProviderRegistry::new());
        let tool = ImageGenerateTool::new(registry);
        assert_eq!(tool.name(), "image_generate");
    }

    #[test]
    fn test_video_generate_tool_creation() {
        let registry = Arc::new(smartassist_providers::media::VideoProviderRegistry::new());
        let tool = VideoGenerateTool::new(registry);
        assert_eq!(tool.name(), "video_generate");
    }

    #[test]
    fn test_music_generate_tool_creation() {
        let registry = Arc::new(smartassist_providers::media::MusicProviderRegistry::new());
        let tool = MusicGenerateTool::new(registry);
        assert_eq!(tool.name(), "music_generate");
    }
}
