//! Media generation provider traits and types.
//!
//! Provides abstractions for image, video, music, TTS, and transcription providers.

use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Common types
// ---------------------------------------------------------------------------

/// A generated media asset (image, video, audio).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedMedia {
    /// Asset URL (if remote).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    /// Local file path (if saved to disk).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,

    /// Base64-encoded data (if inline).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64_data: Option<String>,

    /// MIME type.
    pub mime_type: String,

    /// Size in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<usize>,

    /// Generation metadata.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Image generation request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageGenerationRequest {
    /// Text prompt.
    pub prompt: String,

    /// Negative prompt (what to avoid).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub negative_prompt: Option<String>,

    /// Image size (e.g. "1024x1024").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// Number of images to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<usize>,

    /// Quality hint (e.g. "standard", "hd").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,

    /// Style hint (e.g. "vivid", "natural").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,

    /// Model to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Provider-specific options.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Video generation request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoGenerationRequest {
    /// Text prompt.
    pub prompt: String,

    /// Duration in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,

    /// Resolution (e.g. "720p", "1080p").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,

    /// Aspect ratio (e.g. "16:9", "9:16").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aspect_ratio: Option<String>,

    /// Number of videos to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<usize>,

    /// Model to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Provider-specific options.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Music generation request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MusicGenerationRequest {
    /// Text prompt describing the music.
    pub prompt: String,

    /// Duration in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u32>,

    /// Genre or style hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,

    /// Tempo (BPM).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tempo: Option<u32>,

    /// Instrument hints.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instruments: Option<Vec<String>>,

    /// Model to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Provider-specific options.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// TTS (text-to-speech) request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TtsRequest {
    /// Text to convert.
    pub text: String,

    /// Voice identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,

 /// Speech speed multiplier (0.25 - 4.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,

    /// Output format (e.g. "mp3", "wav", "ogg").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,

    /// Model to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Provider-specific options.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Transcription (STT) request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranscriptionRequest {
    /// Audio data (bytes or base64).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_data: Option<String>,

    /// Audio file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_path: Option<String>,

    /// Audio URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_url: Option<String>,

    /// Language hint (ISO 639-1, e.g. "en").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Model to use.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Provider-specific options.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Transcription result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionResult {
    /// Full transcript text.
    pub text: String,

    /// Segments with timestamps.
    #[serde(default)]
    pub segments: Vec<TranscriptSegment>,

    /// Detected language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Duration in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
}

/// A single transcript segment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptSegment {
    /// Start time in seconds.
    pub start: f64,

    /// End time in seconds.
    pub end: f64,

    /// Transcript text.
    pub text: String,

    /// Confidence score (0.0 - 1.0).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
}

/// Media model info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaModelInfo {
    /// Model ID.
    pub id: String,

    /// Display name.
    pub name: String,

    /// Model kind.
    pub kind: MediaModelKind,

    /// Supported formats.
    #[serde(default)]
    pub formats: Vec<String>,

    /// Maximum input length (for text prompts).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_input_length: Option<usize>,
}

/// Kind of media model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaModelKind {
    Image,
    Video,
    Music,
    Tts,
    Transcription,
}

impl std::fmt::Display for MediaModelKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::Music => "music",
            Self::Tts => "tts",
            Self::Transcription => "transcription",
        };
        write!(f, "{}", s)
    }
}

/// Media provider capabilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaProviderCapabilities {
    /// Supports streaming generation.
    pub streaming: bool,

    /// Supports batch generation.
    pub batch: bool,

    /// Supported output formats.
    #[serde(default)]
    pub formats: Vec<String>,

    /// Maximum prompt length.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_prompt_length: Option<usize>,
}

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// Provider that can generate images from text prompts.
#[async_trait]
pub trait ImageGenerationProvider: Send + Sync {
    /// Provider name.
    fn name(&self) -> &str;

    /// Generate one or more images.
    async fn generate(&self, request: ImageGenerationRequest) -> Result<Vec<GeneratedMedia>>;

    /// List available image generation models.
    async fn list_models(&self) -> Result<Vec<MediaModelInfo>>;

    /// Get provider capabilities.
    fn capabilities(&self) -> MediaProviderCapabilities;
}

/// Provider that can generate videos from text prompts.
#[async_trait]
pub trait VideoGenerationProvider: Send + Sync {
    /// Provider name.
    fn name(&self) -> &str;

    /// Generate one or more videos.
    async fn generate(&self, request: VideoGenerationRequest) -> Result<Vec<GeneratedMedia>>;

    /// List available video generation models.
    async fn list_models(&self) -> Result<Vec<MediaModelInfo>>;

    /// Get provider capabilities.
    fn capabilities(&self) -> MediaProviderCapabilities;
}

/// Provider that can generate music/audio from text prompts.
#[async_trait]
pub trait MusicGenerationProvider: Send + Sync {
    /// Provider name.
    fn name(&self) -> &str;

    /// Generate one or more music tracks.
    async fn generate(&self, request: MusicGenerationRequest) -> Result<Vec<GeneratedMedia>>;

    /// List available music generation models.
    async fn list_models(&self) -> Result<Vec<MediaModelInfo>>;

    /// Get provider capabilities.
    fn capabilities(&self) -> MediaProviderCapabilities;
}

/// Provider that can convert text to speech.
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Provider name.
    fn name(&self) -> &str;

    /// Convert text to speech audio.
    async fn speak(&self, request: TtsRequest) -> Result<GeneratedMedia>;

    /// List available voices/models.
    async fn list_voices(&self) -> Result<Vec<MediaModelInfo>>;

    /// Get provider capabilities.
    fn capabilities(&self) -> MediaProviderCapabilities;
}

/// Provider that can transcribe speech to text.
#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    /// Provider name.
    fn name(&self) -> &str;

    /// Transcribe audio to text.
    async fn transcribe(&self, request: TranscriptionRequest) -> Result<TranscriptionResult>;

    /// List available transcription models.
    async fn list_models(&self) -> Result<Vec<MediaModelInfo>>;

    /// Get provider capabilities.
    fn capabilities(&self) -> MediaProviderCapabilities;
}

// ---------------------------------------------------------------------------
// Registries
// ---------------------------------------------------------------------------

use std::sync::Arc;
use tokio::sync::RwLock;

/// Registry of image generation providers.
#[derive(Default)]
pub struct ImageProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn ImageGenerationProvider>>>,
}

impl ImageProviderRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a provider.
    pub async fn register(&self, provider: Arc<dyn ImageGenerationProvider>) {
        let mut providers = self.providers.write().await;
        providers.insert(provider.name().to_string(), provider);
    }

    /// Get a provider by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn ImageGenerationProvider>> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }

    /// List all registered provider names.
    pub async fn list(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
}

/// Registry of video generation providers.
#[derive(Default)]
pub struct VideoProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn VideoGenerationProvider>>>,
}

impl VideoProviderRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a provider.
    pub async fn register(&self, provider: Arc<dyn VideoGenerationProvider>) {
        let mut providers = self.providers.write().await;
        providers.insert(provider.name().to_string(), provider);
    }

    /// Get a provider by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn VideoGenerationProvider>> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }

    /// List all registered provider names.
    pub async fn list(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
}

/// Registry of music generation providers.
#[derive(Default)]
pub struct MusicProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn MusicGenerationProvider>>>,
}

impl MusicProviderRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a provider.
    pub async fn register(&self, provider: Arc<dyn MusicGenerationProvider>) {
        let mut providers = self.providers.write().await;
        providers.insert(provider.name().to_string(), provider);
    }

    /// Get a provider by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn MusicGenerationProvider>> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }

    /// List all registered provider names.
    pub async fn list(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
}

/// Registry of TTS providers.
#[derive(Default)]
pub struct TtsProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn TtsProvider>>>,
}

impl TtsProviderRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a provider.
    pub async fn register(&self, provider: Arc<dyn TtsProvider>) {
        let mut providers = self.providers.write().await;
        providers.insert(provider.name().to_string(), provider);
    }

    /// Get a provider by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn TtsProvider>> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }

    /// List all registered provider names.
    pub async fn list(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
}

/// Registry of transcription providers.
#[derive(Default)]
pub struct TranscriptionProviderRegistry {
    providers: RwLock<HashMap<String, Arc<dyn TranscriptionProvider>>>,
}

impl TranscriptionProviderRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a provider.
    pub async fn register(&self, provider: Arc<dyn TranscriptionProvider>) {
        let mut providers = self.providers.write().await;
        providers.insert(provider.name().to_string(), provider);
    }

    /// Get a provider by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn TranscriptionProvider>> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }

    /// List all registered provider names.
    pub async fn list(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_media_model_kind() {
        assert_eq!(MediaModelKind::Image.to_string(), "image");
        assert_eq!(MediaModelKind::Video.to_string(), "video");
    }

    #[test]
    fn test_generated_media_creation() {
        let media = GeneratedMedia {
            url: Some("https://example.com/img.png".to_string()),
            path: None,
            base64_data: None,
            mime_type: "image/png".to_string(),
            size_bytes: Some(12345),
            metadata: HashMap::new(),
        };
        assert_eq!(media.mime_type, "image/png");
        assert_eq!(media.size_bytes, Some(12345));
    }

    #[test]
    fn test_image_generation_request() {
        let req = ImageGenerationRequest {
            prompt: "A cat".to_string(),
            size: Some("1024x1024".to_string()),
            ..Default::default()
        };
        assert_eq!(req.prompt, "A cat");
        assert_eq!(req.size, Some("1024x1024".to_string()));
    }

    #[test]
    fn test_transcription_result() {
        let result = TranscriptionResult {
            text: "Hello world".to_string(),
            segments: vec![TranscriptSegment {
                start: 0.0,
                end: 1.5,
                text: "Hello world".to_string(),
                confidence: Some(0.95),
            }],
            language: Some("en".to_string()),
            duration_seconds: Some(1.5),
        };
        assert_eq!(result.text, "Hello world");
        assert_eq!(result.segments.len(), 1);
    }
}
