//! OpenAI-compatible music generation provider.
//!
//! Supports music generation models via OpenAI-compatible APIs.

use crate::media::{
    GeneratedMedia, MediaModelInfo, MediaModelKind, MediaProviderCapabilities,
    MusicGenerationProvider, MusicGenerationRequest,
};
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// OpenAI-compatible music generation provider.
pub struct OpenAiMusicProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiMusicProvider {
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
impl MusicGenerationProvider for OpenAiMusicProvider {
    fn name(&self) -> &str {
        "openai_music"
    }

    async fn generate(&self, request: MusicGenerationRequest) -> Result<Vec<GeneratedMedia>> {
        let url = format!("{}/v1/audio/generations", self.base_url);
        let model = request.model.unwrap_or_else(|| "musicgen".to_string());

        let body = OpenAiMusicRequest {
            model,
            prompt: request.prompt,
            duration_seconds: request.duration_seconds,
            genre: request.genre,
            tempo: request.tempo,
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
                format!("OpenAI music generation failed: {}", error_body),
            ));
        }

        let result: OpenAiMusicResponse = response.json().await.map_err(|e| {
            crate::ProviderError::Network(e)
        })?;

        let mut media = Vec::new();
        for item in result.data {
            media.push(GeneratedMedia {
                url: item.url,
                path: None,
                base64_data: None,
                mime_type: "audio/mpeg".to_string(),
                size_bytes: None,
                metadata: std::collections::HashMap::new(),
            });
        }

        Ok(media)
    }

    async fn list_models(&self) -> Result<Vec<MediaModelInfo>> {
        Ok(vec![
            MediaModelInfo {
                id: "musicgen".to_string(),
                name: "MusicGen".to_string(),
                kind: MediaModelKind::Music,
                formats: vec!["mp3".to_string(), "wav".to_string()],
                max_input_length: Some(4000),
            },
        ])
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: false,
            batch: false,
            formats: vec!["mp3".to_string(), "wav".to_string()],
            max_prompt_length: Some(4000),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct OpenAiMusicRequest {
    model: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tempo: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiMusicResponse {
    #[serde(default)]
    data: Vec<OpenAiMusicItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiMusicItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_music_provider_creation() {
        let provider = OpenAiMusicProvider::new("test-key");
        assert_eq!(provider.name(), "openai_music");
    }

    #[test]
    fn test_capabilities() {
        let provider = OpenAiMusicProvider::new("test-key");
        let caps = provider.capabilities();
        assert!(!caps.streaming);
        assert_eq!(caps.max_prompt_length, Some(4000));
    }

    #[tokio::test]
    async fn test_list_models() {
        let provider = OpenAiMusicProvider::new("test-key");
        let models = provider.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "musicgen");
    }
}
