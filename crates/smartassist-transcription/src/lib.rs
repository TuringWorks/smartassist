//! Realtime speech-to-text transcription for SmartAssist.
//!
//! Provides a WebSocket-based transcription runtime with provider registry
//! supporting OpenAI Whisper and Deepgram.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Audio format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    /// PCM 16-bit signed little-endian.
    PcmS16Le,
    /// PCM 16-bit signed big-endian.
    PcmS16Be,
    /// PCM float 32-bit little-endian.
    PcmF32Le,
    /// Opus encoded audio.
    Opus,
    /// MP3 encoded audio.
    Mp3,
    /// WAV container.
    Wav,
    /// OGG container.
    Ogg,
    /// FLAC encoded audio.
    Flac,
}

impl AudioFormat {
    /// Get the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::PcmS16Le => "audio/pcm;rate=16000;format=S16LE",
            Self::PcmS16Be => "audio/pcm;rate=16000;format=S16BE",
            Self::PcmF32Le => "audio/pcm;rate=16000;format=F32LE",
            Self::Opus => "audio/opus",
            Self::Mp3 => "audio/mpeg",
            Self::Wav => "audio/wav",
            Self::Ogg => "audio/ogg",
            Self::Flac => "audio/flac",
        }
    }
}

/// Transcription configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionConfig {
    /// Audio sample rate in Hz.
    pub sample_rate: u32,

    /// Number of audio channels.
    pub channels: u8,

    /// Audio format.
    pub format: AudioFormat,

    /// Language hint (ISO 639-1, e.g. "en").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,

    /// Whether to enable interim (partial) results.
    #[serde(default)]
    pub interim_results: bool,

    /// Whether to enable punctuation.
    #[serde(default = "bool_true")]
    pub punctuate: bool,

    /// Whether to enable profanity filtering.
    #[serde(default)]
    pub profanity_filter: bool,

    /// Vocabulary hints (words expected in the audio).
    #[serde(default)]
    pub vocabulary: Vec<String>,

    /// Model to use (provider-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Additional provider-specific options.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

impl Default for TranscriptionConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            format: AudioFormat::PcmS16Le,
            language: None,
            interim_results: true,
            punctuate: true,
            profanity_filter: false,
            vocabulary: Vec::new(),
            model: None,
            extra: HashMap::new(),
        }
    }
}

fn bool_true() -> bool {
    true
}

/// A transcript event from the transcription stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TranscriptEvent {
    /// Transcription has started.
    Started {
        /// Session ID.
        session_id: String,
    },

    /// A partial (interim) transcript result.
    Partial {
        /// Transcript text so far.
        text: String,

        /// Confidence score (0.0 - 1.0).
        #[serde(skip_serializing_if = "Option::is_none")]
        confidence: Option<f32>,

        /// Audio start time in seconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        start_time: Option<f64>,

        /// Audio end time in seconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        end_time: Option<f64>,
    },

    /// A final transcript result.
    Final {
        /// Transcript text.
        text: String,

        /// Confidence score (0.0 - 1.0).
        #[serde(skip_serializing_if = "Option::is_none")]
        confidence: Option<f32>,

        /// Audio start time in seconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        start_time: Option<f64>,

        /// Audio end time in seconds.
        #[serde(skip_serializing_if = "Option::is_none")]
        end_time: Option<f64>,

        /// Individual words with timestamps.
        #[serde(default)]
        words: Vec<WordInfo>,
    },

    /// Transcription has ended.
    Ended {
        /// Session ID.
        session_id: String,

        /// Final transcript.
        #[serde(skip_serializing_if = "Option::is_none")]
        final_text: Option<String>,
    },

    /// An error occurred.
    Error {
        /// Error message.
        message: String,
    },
}

/// Word-level information with timestamps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WordInfo {
    /// The word.
    pub word: String,

    /// Start time in seconds.
    pub start: f64,

    /// End time in seconds.
    pub end: f64,

    /// Confidence score.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
}

/// A transcription session.
pub struct TranscriptionSession {
    /// Unique session ID.
    pub id: String,

    /// Configuration for this session.
    pub config: TranscriptionConfig,

    /// Event channel sender.
    pub event_tx: mpsc::Sender<TranscriptEvent>,
}

impl TranscriptionSession {
    /// Create a new transcription session.
    pub fn new(config: TranscriptionConfig) -> (Self, mpsc::Receiver<TranscriptEvent>) {
        let id = uuid::Uuid::new_v4().to_string();
        let (event_tx, event_rx) = mpsc::channel(256);
        (Self { id, config, event_tx }, event_rx)
    }
}

/// A transcription provider that can process audio streams.
#[async_trait]
pub trait TranscriptionProvider: Send + Sync {
    /// Get provider name.
    fn name(&self) -> &str;

    /// Start a new transcription session.
    async fn start_session(
        &self,
        config: TranscriptionConfig,
    ) -> anyhow::Result<TranscriptionSession>;

    /// Send audio data to an active session.
    async fn send_audio(
        &self,
        session_id: &str,
        audio: bytes::Bytes,
    ) -> anyhow::Result<()>;

    /// Stop a transcription session.
    async fn stop_session(&self, session_id: &str) -> anyhow::Result<()>;

    /// List available models.
    async fn list_models(&self) -> anyhow::Result<Vec<TranscriptionModelInfo>>;
}

/// Transcription model information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionModelInfo {
    /// Model ID.
    pub id: String,

    /// Display name.
    pub name: String,

    /// Supported languages.
    #[serde(default)]
    pub languages: Vec<String>,

    /// Maximum audio duration in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_duration_seconds: Option<u32>,
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
    pub async fn register(&self,
        provider: Arc<dyn TranscriptionProvider>,
    ) {
        let mut providers = self.providers.write().await;
        providers.insert(provider.name().to_string(), provider);
    }

    /// Get a provider by name.
    pub async fn get(
        &self,
        name: &str,
    ) -> Option<Arc<dyn TranscriptionProvider>> {
        let providers = self.providers.read().await;
        providers.get(name).cloned()
    }

    /// List all registered provider names.
    pub async fn list(&self) -> Vec<String> {
        let providers = self.providers.read().await;
        providers.keys().cloned().collect()
    }
}

/// Shared transcription runtime.
pub struct TranscriptionRuntime {
    registry: Arc<TranscriptionProviderRegistry>,
    active_sessions: RwLock<HashMap<String, TranscriptionSession>>,
}

impl TranscriptionRuntime {
    /// Create a new transcription runtime.
    pub fn new(registry: Arc<TranscriptionProviderRegistry>) -> Self {
        Self {
            registry,
            active_sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Start a transcription session with the given provider.
    pub async fn start(
        &self,
        provider_name: &str,
        config: TranscriptionConfig,
    ) -> anyhow::Result<mpsc::Receiver<TranscriptEvent>> {
        let provider = self
            .registry
            .get(provider_name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Provider '{}' not found", provider_name))?;

        let session = provider.start_session(config).await?;
        let session_id = session.id.clone();
        let event_rx = {
            let (s, rx) = TranscriptionSession::new(session.config.clone());
            let mut sessions = self.active_sessions.write().await;
            sessions.insert(session_id.clone(), s);
            rx
        };

        // Forward events from the provider session to our runtime channel
        // In a real implementation, this would bridge the provider's event stream.
        // For now, we emit a started event.
        {
            let sessions = self.active_sessions.read().await;
            if let Some(s) = sessions.get(&session_id) {
                let _ = s.event_tx.send(TranscriptEvent::Started {
                    session_id: session_id.clone(),
                }).await;
            }
        }

        Ok(event_rx)
    }

    /// Send audio data to an active session.
    pub async fn send_audio(
        &self,
        session_id: &str,
        audio: bytes::Bytes,
    ) -> anyhow::Result<()> {
        let provider_name = "whisper"; // In a real impl, look up from session
        if let Some(provider) = self.registry.get(provider_name).await {
            provider.send_audio(session_id, audio).await
        } else {
            Err(anyhow::anyhow!("No provider available for session"))
        }
    }

    /// Stop a transcription session.
    pub async fn stop(&self, session_id: &str) -> anyhow::Result<()> {
        {
            let mut sessions = self.active_sessions.write().await;
            sessions.remove(session_id);
        }
        let provider_name = "whisper";
        if let Some(provider) = self.registry.get(provider_name).await {
            provider.stop_session(session_id).await
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_mime_type() {
        assert_eq!(AudioFormat::PcmS16Le.mime_type(), "audio/pcm;rate=16000;format=S16LE");
        assert_eq!(AudioFormat::Wav.mime_type(), "audio/wav");
        assert_eq!(AudioFormat::Mp3.mime_type(), "audio/mpeg");
    }

    #[test]
    fn test_transcription_config_default() {
        let config = TranscriptionConfig::default();
        assert_eq!(config.sample_rate, 16000);
        assert_eq!(config.channels, 1);
        assert!(config.punctuate);
        assert!(!config.profanity_filter);
    }

    #[test]
    fn test_transcript_event_serialization() {
        let event = TranscriptEvent::Partial {
            text: "Hello".to_string(),
            confidence: Some(0.95),
            start_time: Some(0.0),
            end_time: Some(1.0),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("partial"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn test_transcription_session_creation() {
        let config = TranscriptionConfig::default();
        let (session, _rx) = TranscriptionSession::new(config);
        assert!(!session.id.is_empty());
    }

    #[test]
    fn test_provider_registry() {
        let registry = TranscriptionProviderRegistry::new();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let names = registry.list().await;
            assert!(names.is_empty());
        });
    }

    #[tokio::test]
    async fn test_transcription_runtime() {
        let registry = Arc::new(TranscriptionProviderRegistry::new());
        let runtime = TranscriptionRuntime::new(registry);
        assert_eq!(runtime.registry.list().await.len(), 0);
    }
}
