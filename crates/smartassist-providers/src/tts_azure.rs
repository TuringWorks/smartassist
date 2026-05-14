//! Azure Speech TTS provider.

use base64::Engine;
use crate::media::{
    GeneratedMedia, MediaModelInfo, MediaModelKind, MediaProviderCapabilities, TtsProvider,
    TtsRequest,
};
use crate::Result;
use async_trait::async_trait;

/// Azure Speech TTS provider.
pub struct AzureTtsProvider {
    subscription_key: String,
    region: String,
    client: reqwest::Client,
}

impl AzureTtsProvider {
    /// Create a new provider.
    pub fn new(subscription_key: impl Into<String>, region: impl Into<String>) -> Self {
        Self {
            subscription_key: subscription_key.into(),
            region: region.into(),
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl TtsProvider for AzureTtsProvider {
    fn name(&self) -> &str {
        "azure_tts"
    }

    async fn speak(&self, request: TtsRequest) -> Result<GeneratedMedia> {
        let url = format!(
            "https://{}.tts.speech.microsoft.com/cognitiveservices/v1",
            self.region
        );
        let voice = request.voice.unwrap_or_else(|| "en-US-AriaNeural".to_string());
        let speed = request.speed.unwrap_or(1.0);
        let rate = format!("{:.0}%", (speed - 1.0) * 100.0);

        let ssml = format!(
            r#"<speak version="1.0" xmlns="http://www.w3.org/2001/10/synthesis" xml:lang="en-US">
                <voice name="{}">
                    <prosody rate="{}">{}</prosody>
                </voice>
            </speak>
            "#,
            voice, rate, request.text
        );

        let response = self
            .client
            .post(&url)
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .header("Content-Type", "application/ssml+xml")
            .header("X-Microsoft-OutputFormat", "audio-16khz-128kbitrate-mono-mp3")
            .body(ssml)
            .send()
            .await
            .map_err(|e| crate::ProviderError::Network(e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().await.unwrap_or_default();
            return Err(crate::ProviderError::server_error(
                status.as_u16(),
                format!("Azure TTS failed: {}", error_body),
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
        let url = format!(
            "https://{}.tts.speech.microsoft.com/cognitiveservices/voices/list",
            self.region
        );
        let response = self
            .client
            .get(&url)
            .header("Ocp-Apim-Subscription-Key", &self.subscription_key)
            .send()
            .await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let result: Vec<AzureVoice> = resp.json().await.unwrap_or_default();
                Ok(result
                    .into_iter()
                    .map(|v| MediaModelInfo {
                        id: v.short_name,
                        name: v.display_name,
                        kind: MediaModelKind::Tts,
                        formats: vec!["mp3".to_string()],
                        max_input_length: Some(10000),
                    })
                    .collect())
            }
            _ => Ok(vec![
                MediaModelInfo {
                    id: "en-US-AriaNeural".to_string(),
                    name: "Aria (US English)".to_string(),
                    kind: MediaModelKind::Tts,
                    formats: vec!["mp3".to_string()],
                    max_input_length: Some(10000),
                },
            ]),
        }
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: false,
            batch: false,
            formats: vec!["mp3".to_string(), "wav".to_string(), "ogg".to_string()],
            max_prompt_length: Some(10000),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
struct AzureVoice {
    #[serde(rename = "ShortName")]
    short_name: String,
    #[serde(rename = "DisplayName")]
    display_name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_tts_provider_creation() {
        let provider = AzureTtsProvider::new("test-key", "westus2");
        assert_eq!(provider.name(), "azure_tts");
        assert_eq!(provider.region, "westus2");
    }

    #[test]
    fn test_capabilities() {
        let provider = AzureTtsProvider::new("test-key", "westus2");
        let caps = provider.capabilities();
        assert_eq!(caps.max_prompt_length, Some(10000));
    }

    #[tokio::test]
    async fn test_list_voices_fallback() {
        let provider = AzureTtsProvider::new("test-key", "westus2");
        let voices = provider.list_voices().await.unwrap();
        assert!(!voices.is_empty());
    }
}
