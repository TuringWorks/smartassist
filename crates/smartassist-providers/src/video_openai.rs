//! OpenAI-compatible video generation provider.
//!
//! Supports Sora and other video generation models via OpenAI-compatible APIs.

use crate::media::{
    GeneratedMedia, MediaModelInfo, MediaModelKind, MediaProviderCapabilities,
    VideoGenerationProvider, VideoGenerationRequest,
};
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// OpenAI-compatible video generation provider.
pub struct OpenAiVideoProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiVideoProvider {
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
impl VideoGenerationProvider for OpenAiVideoProvider {
    fn name(&self) -> &str {
        "openai_video"
    }

    async fn generate(&self, request: VideoGenerationRequest) -> Result<Vec<GeneratedMedia>> {
        let url = format!("{}/v1/videos/generations", self.base_url);

        let body = OpenAiVideoRequest {
            model: request.model.unwrap_or_else(|| "sora".to_string()),
            prompt: request.prompt,
            n: request.n.unwrap_or(1).clamp(1, 4) as u8,
            duration_seconds: request.duration_seconds,
            resolution: request.resolution,
            aspect_ratio: request.aspect_ratio,
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
                format!("OpenAI video generation failed: {}", error_body),
            ));
        }

        let result: OpenAiVideoResponse = response.json().await.map_err(|e| {
            crate::ProviderError::Network(e)
        })?;

        let mut media = Vec::new();
        for item in result.data {
            media.push(GeneratedMedia {
                url: item.url,
                path: None,
                base64_data: None,
                mime_type: "video/mp4".to_string(),
                size_bytes: None,
                metadata: std::collections::HashMap::new(),
            });
        }

        Ok(media)
    }

    async fn list_models(&self) -> Result<Vec<MediaModelInfo>> {
        Ok(vec![
            MediaModelInfo {
                id: "sora".to_string(),
                name: "Sora".to_string(),
                kind: MediaModelKind::Video,
                formats: vec!["mp4".to_string()],
                max_input_length: Some(4000),
            },
        ])
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: false,
            batch: true,
            formats: vec!["mp4".to_string()],
            max_prompt_length: Some(4000),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct OpenAiVideoRequest {
    model: String,
    prompt: String,
    n: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_seconds: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    aspect_ratio: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiVideoResponse {
    data: Vec<OpenAiVideoItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiVideoItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_video_provider_creation() {
        let provider = OpenAiVideoProvider::new("test-key");
        assert_eq!(provider.name(), "openai_video");
    }

    #[test]
    fn test_capabilities() {
        let provider = OpenAiVideoProvider::new("test-key");
        let caps = provider.capabilities();
        assert!(!caps.streaming);
        assert!(caps.batch);
        assert_eq!(caps.max_prompt_length, Some(4000));
    }

    #[tokio::test]
    async fn test_list_models() {
        let provider = OpenAiVideoProvider::new("test-key");
        let models = provider.list_models().await.unwrap();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].id, "sora");
    }
}
