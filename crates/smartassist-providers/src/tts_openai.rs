//! OpenAI TTS provider.

use base64::Engine;
use crate::media::{
    GeneratedMedia, MediaModelInfo, MediaModelKind, MediaProviderCapabilities, TtsProvider,
    TtsRequest,
};
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// OpenAI TTS provider.
pub struct OpenAiTtsProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiTtsProvider {
    /// Create a new provider.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com".to_string(),
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
impl TtsProvider for OpenAiTtsProvider {
    fn name(&self) -> &str {
        "openai_tts"
    }

    async fn speak(&self, request: TtsRequest) -> Result<GeneratedMedia> {
        let url = format!("{}/v1/audio/speech", self.base_url);
        let voice = request.voice.unwrap_or_else(|| "alloy".to_string());
        let speed = request.speed.unwrap_or(1.0).clamp(0.25, 4.0);
        let model = request.model.unwrap_or_else(|| "tts-1".to_string());
        let response_format = request.format.unwrap_or_else(|| "mp3".to_string());

        let body = OpenAiTtsRequest {
            model,
            input: request.text,
            voice,
            speed,
            response_format: response_format.clone(),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
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
                format!("OpenAI TTS failed: {}", error_body),
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
            mime_type: format!("audio/{}", response_format),
            size_bytes: Some(byte_count),
            metadata: std::collections::HashMap::new(),
        })
    }

    async fn list_voices(&self) -> Result<Vec<MediaModelInfo>> {
        Ok(vec![
            voice_model("alloy", "Alloy"),
            voice_model("echo", "Echo"),
            voice_model("fable", "Fable"),
            voice_model("onyx", "Onyx"),
            voice_model("nova", "Nova"),
            voice_model("shimmer", "Shimmer"),
        ])
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: false,
            batch: false,
            formats: vec!["mp3".to_string(), "opus".to_string(), "aac".to_string(), "flac".to_string()],
            max_prompt_length: Some(4096),
        }
    }
}

fn voice_model(id: &str, name: &str) -> MediaModelInfo {
    MediaModelInfo {
        id: id.to_string(),
        name: name.to_string(),
        kind: MediaModelKind::Tts,
        formats: vec!["mp3".to_string()],
        max_input_length: Some(4096),
    }
}

#[derive(Debug, Clone, Serialize)]
struct OpenAiTtsRequest {
    model: String,
    input: String,
    voice: String,
    speed: f32,
    response_format: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_tts_provider_creation() {
        let provider = OpenAiTtsProvider::new("test-key");
        assert_eq!(provider.name(), "openai_tts");
    }

    #[test]
    fn test_capabilities() {
        let provider = OpenAiTtsProvider::new("test-key");
        let caps = provider.capabilities();
        assert_eq!(caps.max_prompt_length, Some(4096));
    }

    #[tokio::test]
    async fn test_list_voices() {
        let provider = OpenAiTtsProvider::new("test-key");
        let voices = provider.list_voices().await.unwrap();
        assert_eq!(voices.len(), 6);
        assert_eq!(voices[0].id, "alloy");
    }
}
