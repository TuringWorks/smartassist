//! Ollama image generation provider.
//!
//! Supports local image generation models served via Ollama's API.

use crate::media::{
    GeneratedMedia, ImageGenerationProvider, ImageGenerationRequest, MediaModelInfo,
    MediaModelKind, MediaProviderCapabilities,
};
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Ollama image generation provider.
pub struct OllamaImageProvider {
    base_url: String,
    client: reqwest::Client,
}

impl OllamaImageProvider {
    /// Create a new Ollama image provider.
    pub fn new() -> Self {
        Self {
            base_url: "http://localhost:11434".to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Set a custom base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

impl Default for OllamaImageProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ImageGenerationProvider for OllamaImageProvider {
    fn name(&self) -> &str {
        "ollama_image"
    }

    async fn generate(&self, request: ImageGenerationRequest) -> Result<Vec<GeneratedMedia>> {
        let url = format!("{}/api/generate", self.base_url);
        let model = request.model.unwrap_or_else(|| "llava".to_string());

        let body = OllamaImageRequest {
            model,
            prompt: request.prompt,
            stream: false,
        };

        let response = self
            .client
            .post(&url)
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
                format!("Ollama image generation failed: {}", error_body),
            ));
        }

        let result: OllamaImageResponse = response.json().await.map_err(|e| {
            crate::ProviderError::Network(e)
        })?;

        let mut media = Vec::new();
        if let Some(image_b64) = result.images.first() {
            media.push(GeneratedMedia {
                url: None,
                path: None,
                base64_data: Some(image_b64.clone()),
                mime_type: "image/png".to_string(),
                size_bytes: None,
                metadata: std::collections::HashMap::new(),
            });
        }

        Ok(media)
    }

    async fn list_models(&self) -> Result<Vec<MediaModelInfo>> {
        let url = format!("{}/api/tags", self.base_url);
        let response = self.client.get(&url).send().await;

        let models = match response {
            Ok(resp) if resp.status().is_success() => {
                let result: OllamaTagResponse = resp.json().await.unwrap_or_default();
                result
                    .models
                    .into_iter()
                    .filter(|m| m.name.contains("llava") || m.name.contains("image"))
                    .map(|m| MediaModelInfo {
                        id: m.name.clone(),
                        name: m.name,
                        kind: MediaModelKind::Image,
                        formats: vec!["png".to_string()],
                        max_input_length: Some(4096),
                    })
                    .collect()
            }
            _ => vec![],
        };

        if models.is_empty() {
            Ok(vec![MediaModelInfo {
                id: "llava".to_string(),
                name: "LLaVA".to_string(),
                kind: MediaModelKind::Image,
                formats: vec!["png".to_string()],
                max_input_length: Some(4096),
            }])
        } else {
            Ok(models)
        }
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: false,
            batch: false,
            formats: vec!["png".to_string()],
            max_prompt_length: Some(4096),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct OllamaImageRequest {
    model: String,
    prompt: String,
    stream: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaImageResponse {
    #[serde(default)]
    images: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct OllamaTagResponse {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Debug, Clone, Deserialize)]
struct OllamaModel {
    name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_image_provider_creation() {
        let provider = OllamaImageProvider::new();
        assert_eq!(provider.name(), "ollama_image");
        assert_eq!(provider.base_url, "http://localhost:11434");
    }

    #[test]
    fn test_ollama_image_provider_default() {
        let provider = OllamaImageProvider::default();
        assert_eq!(provider.base_url, "http://localhost:11434");
    }

    #[test]
    fn test_ollama_image_provider_custom_url() {
        let provider = OllamaImageProvider::new()
            .with_base_url("http://ollama.internal:11434");
        assert_eq!(provider.base_url, "http://ollama.internal:11434");
    }

    #[test]
    fn test_capabilities() {
        let provider = OllamaImageProvider::new();
        let caps = provider.capabilities();
        assert!(!caps.streaming);
        assert!(!caps.batch);
        assert_eq!(caps.max_prompt_length, Some(4096));
    }
}
