//! OpenAI DALL-E image generation provider.

use crate::media::{
    GeneratedMedia, ImageGenerationProvider, ImageGenerationRequest, MediaModelInfo,
    MediaModelKind, MediaProviderCapabilities,
};
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// OpenAI image generation provider (DALL-E).
pub struct OpenAiImageProvider {
    api_key: String,
    base_url: String,
    client: reqwest::Client,
}

impl OpenAiImageProvider {
    /// Create a new OpenAI image provider.
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
impl ImageGenerationProvider for OpenAiImageProvider {
    fn name(&self) -> &str {
        "openai_image"
    }

    async fn generate(&self, request: ImageGenerationRequest) -> Result<Vec<GeneratedMedia>> {
        let url = format!("{}/v1/images/generations", self.base_url);

        let body = OpenAiImageRequest {
            model: request.model.unwrap_or_else(|| "dall-e-3".to_string()),
            prompt: request.prompt,
            n: request.n.unwrap_or(1).clamp(1, 10) as u8,
            size: request.size.unwrap_or_else(|| "1024x1024".to_string()),
            quality: request.quality,
            style: request.style,
            response_format: Some("url".to_string()),
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
                format!("OpenAI image generation failed: {}", error_body),
            ));
        }

        let result: OpenAiImageResponse = response.json().await.map_err(|e| {
            crate::ProviderError::Network(e)
        })?;

        let mut media = Vec::new();
        for item in result.data {
            let url = item.url.unwrap_or_default();
            media.push(GeneratedMedia {
                url: Some(url),
                path: None,
                base64_data: item.b64_json,
                mime_type: "image/png".to_string(),
                size_bytes: None,
                metadata: {
                    let mut m = std::collections::HashMap::new();
                    if let Some(revised) = item.revised_prompt {
                        m.insert("revised_prompt".to_string(), serde_json::Value::String(revised));
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
                id: "dall-e-3".to_string(),
                name: "DALL-E 3".to_string(),
                kind: MediaModelKind::Image,
                formats: vec!["png".to_string(), "jpeg".to_string()],
                max_input_length: Some(4000),
            },
            MediaModelInfo {
                id: "dall-e-2".to_string(),
                name: "DALL-E 2".to_string(),
                kind: MediaModelKind::Image,
                formats: vec!["png".to_string(), "jpeg".to_string()],
                max_input_length: Some(1000),
            },
        ])
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: false,
            batch: true,
            formats: vec!["png".to_string(), "jpeg".to_string()],
            max_prompt_length: Some(4000),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct OpenAiImageRequest {
    model: String,
    prompt: String,
    n: u8,
    size: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    style: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiImageResponse {
    data: Vec<OpenAiImageItem>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiImageItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(rename = "b64_json", skip_serializing_if = "Option::is_none")]
    b64_json: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    revised_prompt: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_image_provider_creation() {
        let provider = OpenAiImageProvider::new("test-key");
        assert_eq!(provider.name(), "openai_image");
        assert_eq!(provider.base_url, "https://api.openai.com");
    }

    #[test]
    fn test_openai_image_provider_custom_url() {
        let provider = OpenAiImageProvider::new("test-key")
            .with_base_url("https://custom.openai.example.com");
        assert_eq!(provider.base_url, "https://custom.openai.example.com");
    }

    #[test]
    fn test_capabilities() {
        let provider = OpenAiImageProvider::new("test-key");
        let caps = provider.capabilities();
        assert!(!caps.streaming);
        assert!(caps.batch);
        assert_eq!(caps.max_prompt_length, Some(4000));
    }

    #[tokio::test]
    async fn test_list_models() {
        let provider = OpenAiImageProvider::new("test-key");
        let models = provider.list_models().await.unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "dall-e-3");
        assert_eq!(models[1].id, "dall-e-2");
    }
}
