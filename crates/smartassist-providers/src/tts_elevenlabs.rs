//! ElevenLabs TTS provider.

use base64::Engine;
use crate::media::{
    GeneratedMedia, MediaModelInfo, MediaModelKind, MediaProviderCapabilities, TtsProvider,
    TtsRequest,
};
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// ElevenLabs TTS provider.
pub struct ElevenLabsTtsProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl ElevenLabsTtsProvider {
    /// Create a new provider.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.elevenlabs.io".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Set a custom base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

#[async_trait]
impl TtsProvider for ElevenLabsTtsProvider {
    fn name(&self) -> &str {
        "elevenlabs_tts"
    }

    async fn speak(&self, request: TtsRequest) -> Result<GeneratedMedia> {
        let voice_id = request.voice.unwrap_or_else(|| "21m00Tcm4TlvDq8ikWAM".to_string());
        let url = format!("{}/v1/text-to-speech/{}", self.base_url, voice_id);
        let model = request.model.unwrap_or_else(|| "eleven_monolingual_v1".to_string());

        let body = ElevenLabsTtsRequest {
            text: request.text,
            model_id: model,
            voice_settings: ElevenLabsVoiceSettings {
                speed: request.speed.unwrap_or(1.0),
            },
        };

        let response = self
            .client
            .post(&url)
            .header("xi-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| crate::ProviderError::Network(e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(crate::ProviderError::server_error(
                status.as_u16(),
                format!("ElevenLabs TTS failed: {}", error_body),
            ));
        }

        let bytes = response.bytes().await.map_err(|e| {
            crate::ProviderError::Network(e)
        })?;
        let byte_count = bytes.len();

        Ok(GeneratedMedia {
            url: None,
            path: None,
            base64_data: Some(base64::engine::general_purpose::STANDARD.encode(&bytes)),
            mime_type: "audio/mpeg".to_string(),
            size_bytes: Some(byte_count),
            metadata: std::collections::HashMap::new(),
        })
    }

    async fn list_voices(&self) -> Result<Vec<MediaModelInfo>> {
        let url = format!("{}/v1/voices", self.base_url);
        let response = self
            .client
            .get(&url)
            .header("xi-api-key", &self.api_key)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let result: ElevenLabsVoicesResponse = resp.json().await.unwrap_or_default();
                Ok(result
                    .voices
                    .into_iter()
                    .map(|v| MediaModelInfo {
                        id: v.voice_id,
                        name: v.name,
                        kind: MediaModelKind::Tts,
                        formats: vec!["mp3".to_string()],
                        max_input_length: Some(5000),
                    })
                    .collect())
            }
            _ => Ok(vec![
                MediaModelInfo {
                    id: "21m00Tcm4TlvDq8ikWAM".to_string(),
                    name: "Rachel".to_string(),
                    kind: MediaModelKind::Tts,
                    formats: vec!["mp3".to_string()],
                    max_input_length: Some(5000),
                },
            ]),
        }
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: true,
            batch: false,
            formats: vec!["mp3".to_string(), "wav".to_string()],
            max_prompt_length: Some(5000),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct ElevenLabsTtsRequest {
    text: String,
    model_id: String,
    voice_settings: ElevenLabsVoiceSettings,
}

#[derive(Debug, Clone, Serialize)]
struct ElevenLabsVoiceSettings {
    speed: f32,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ElevenLabsVoicesResponse {
    #[serde(default)]
    voices: Vec<ElevenLabsVoice>,
}

#[derive(Debug, Clone, Deserialize)]
struct ElevenLabsVoice {
    voice_id: String,
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elevenlabs_tts_provider_creation() {
        let provider = ElevenLabsTtsProvider::new("test-key");
        assert_eq!(provider.name(), "elevenlabs_tts");
    }

    #[test]
    fn test_capabilities() {
        let provider = ElevenLabsTtsProvider::new("test-key");
        let caps = provider.capabilities();
        assert!(caps.streaming);
        assert_eq!(caps.max_prompt_length, Some(5000));
    }

    #[tokio::test]
    async fn test_list_voices_fallback() {
        let provider = ElevenLabsTtsProvider::new("test-key");
        let voices = provider.list_voices().await.unwrap();
        assert!(!voices.is_empty());
    }
}
