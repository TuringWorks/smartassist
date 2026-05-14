//! DashScope (Alibaba) video generation provider.
//!
//! Supports Wanx/Wan2.1 and other video models via DashScope API.

use crate::media::{
    GeneratedMedia, MediaModelInfo, MediaModelKind, MediaProviderCapabilities,
    VideoGenerationProvider, VideoGenerationRequest,
};
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// DashScope video generation provider.
pub struct DashScopeVideoProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl DashScopeVideoProvider {
    /// Create a new provider.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://dashscope.aliyuncs.com".to_string(),
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
impl VideoGenerationProvider for DashScopeVideoProvider {
    fn name(&self) -> &str {
        "dashscope_video"
    }

    async fn generate(&self, request: VideoGenerationRequest) -> Result<Vec<GeneratedMedia>> {
        let url = format!("{}/api/v1/services/aigc/video-generation/video-synthesis", self.base_url);
        let model = request.model.unwrap_or_else(|| "wanx2.1-t2v-plus".to_string());

        let body = DashScopeVideoRequest {
            model,
            input: DashScopeVideoInput {
                prompt: request.prompt,
            },
            parameters: DashScopeVideoParams {
                size: request.resolution,
                duration: request.duration_seconds,
                n: request.n.unwrap_or(1) as u8,
            },
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
                format!("DashScope video generation failed: {}", error_body),
            ));
        }

        let result: DashScopeVideoResponse = response.json().await.map_err(|e| {
            crate::ProviderError::Network(e)
        })?;

        let mut media = Vec::new();
        for item in result.output.video_url {
            media.push(GeneratedMedia {
                url: Some(item),
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
                id: "wanx2.1-t2v-plus".to_string(),
                name: "Wanx 2.1 T2V Plus".to_string(),
                kind: MediaModelKind::Video,
                formats: vec!["mp4".to_string()],
                max_input_length: Some(10000),
            },
            MediaModelInfo {
                id: "wan2.1-i2v-plus".to_string(),
                name: "Wan 2.1 I2V Plus".to_string(),
                kind: MediaModelKind::Video,
                formats: vec!["mp4".to_string()],
                max_input_length: Some(10000),
            },
        ])
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: false,
            batch: true,
            formats: vec!["mp4".to_string()],
            max_prompt_length: Some(10000),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct DashScopeVideoRequest {
    model: String,
    input: DashScopeVideoInput,
    parameters: DashScopeVideoParams,
}

#[derive(Debug, Clone, Serialize)]
struct DashScopeVideoInput {
    prompt: String,
}

#[derive(Debug, Clone, Serialize)]
struct DashScopeVideoParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration: Option<u32>,
    n: u8,
}

#[derive(Debug, Clone, Deserialize)]
struct DashScopeVideoResponse {
    output: DashScopeVideoOutput,
}

#[derive(Debug, Clone, Deserialize)]
struct DashScopeVideoOutput {
    #[serde(default)]
    video_url: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashscope_video_provider_creation() {
        let provider = DashScopeVideoProvider::new("test-key");
        assert_eq!(provider.name(), "dashscope_video");
    }

    #[test]
    fn test_capabilities() {
        let provider = DashScopeVideoProvider::new("test-key");
        let caps = provider.capabilities();
        assert!(!caps.streaming);
        assert!(caps.batch);
        assert_eq!(caps.max_prompt_length, Some(10000));
    }

    #[tokio::test]
    async fn test_list_models() {
        let provider = DashScopeVideoProvider::new("test-key");
        let models = provider.list_models().await.unwrap();
        assert_eq!(models.len(), 2);
    }
}
