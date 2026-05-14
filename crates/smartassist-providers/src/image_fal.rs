//! Fal.ai image generation provider.
//!
//! Supports Fal.ai's fast image generation APIs.

use crate::media::{
    GeneratedMedia, ImageGenerationProvider, ImageGenerationRequest, MediaModelInfo,
    MediaModelKind, MediaProviderCapabilities,
};
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Fal.ai image generation provider.
pub struct FalImageProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl FalImageProvider {
    /// Create a new Fal image provider.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://queue.fal.run".to_string(),
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
impl ImageGenerationProvider for FalImageProvider {
    fn name(&self) -> &str {
        "fal_image"
    }

    async fn generate(&self, request: ImageGenerationRequest) -> Result<Vec<GeneratedMedia>> {
        let model = request.model.unwrap_or_else(|| "fal-ai/flux/dev".to_string());
        let url = format!("{}/{}", self.base_url, model);

        let body = FalImageRequest {
            prompt: request.prompt,
            image_size: request.size,
            num_images: request.n.map(|n| n as u8),
            safety_tolerance: Some("2".to_string()),
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Key {}", self.api_key))
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
                format!("Fal image generation failed: {}", error_body),
            ));
        }

        let result: FalImageResponse = response.json().await.map_err(|e| {
            crate::ProviderError::Network(e)
        })?;

        let mut media = Vec::new();
        for item in result.images {
            media.push(GeneratedMedia {
                url: Some(item.url),
                path: None,
                base64_data: None,
                mime_type: item.content_type.unwrap_or_else(|| "image/png".to_string()),
                size_bytes: item.width.map(|w| (w * 4) as usize),
                metadata: {
                    let mut m = std::collections::HashMap::new();
                    if let Some(w) = item.width {
                        m.insert("width".to_string(), serde_json::Value::Number(w.into()));
                    }
                    if let Some(h) = item.height {
                        m.insert("height".to_string(), serde_json::Value::Number(h.into()));
                    }
                    m
                },
            });
        }

        Ok(media)
    }

    async fn list_models(&self) -> Result<Vec<MediaModelInfo>> {
        Ok(vec![
            MediaModelInfo {
                id: "fal-ai/flux/dev".to_string(),
                name: "FLUX.1 [dev]".to_string(),
                kind: MediaModelKind::Image,
                formats: vec!["png".to_string(), "jpeg".to_string()],
                max_input_length: Some(10000),
            },
            MediaModelInfo {
                id: "fal-ai/flux/schnell".to_string(),
                name: "FLUX.1 [schnell]".to_string(),
                kind: MediaModelKind::Image,
                formats: vec!["png".to_string(), "jpeg".to_string()],
                max_input_length: Some(10000),
            },
            MediaModelInfo {
                id: "fal-ai/fast-sdxl".to_string(),
                name: "Fast SDXL".to_string(),
                kind: MediaModelKind::Image,
                formats: vec!["png".to_string(), "jpeg".to_string()],
                max_input_length: Some(10000),
            },
        ])
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: false,
            batch: true,
            formats: vec!["png".to_string(), "jpeg".to_string()],
            max_prompt_length: Some(10000),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct FalImageRequest {
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    image_size: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    num_images: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    safety_tolerance: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct FalImageResponse {
    #[serde(default)]
    images: Vec<FalImageItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct FalImageItem {
    url: String,
    #[serde(rename = "content_type", skip_serializing_if = "Option::is_none")]
    content_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fal_image_provider_creation() {
        let provider = FalImageProvider::new("test-key");
        assert_eq!(provider.name(), "fal_image");
        assert_eq!(provider.base_url, "https://queue.fal.run");
    }

    #[test]
    fn test_fal_image_provider_custom_url() {
        let provider = FalImageProvider::new("test-key")
            .with_base_url("https://internal.fal.run");
        assert_eq!(provider.base_url, "https://internal.fal.run");
    }

    #[test]
    fn test_capabilities() {
        let provider = FalImageProvider::new("test-key");
        let caps = provider.capabilities();
        assert!(!caps.streaming);
        assert!(caps.batch);
        assert_eq!(caps.max_prompt_length, Some(10000));
    }

    #[tokio::test]
    async fn test_list_models() {
        let provider = FalImageProvider::new("test-key");
        let models = provider.list_models().await.unwrap();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].id, "fal-ai/flux/dev");
    }
}
