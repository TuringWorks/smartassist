//! Local CLI TTS provider (fallback).
//!
//! Uses system CLI tools like `say` (macOS), `espeak` (Linux), or `piper` (local NN).

use base64::Engine;
use crate::media::{
    GeneratedMedia, MediaModelInfo, MediaModelKind, MediaProviderCapabilities, TtsProvider,
    TtsRequest,
};
use crate::Result;
use async_trait::async_trait;
use std::process::Stdio;
use tokio::process::Command;

/// Local CLI TTS provider.
pub struct LocalTtsProvider;

impl LocalTtsProvider {
    /// Create a new provider.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalTtsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TtsProvider for LocalTtsProvider {
    fn name(&self) -> &str {
        "local_tts"
    }

    async fn speak(&self, request: TtsRequest) -> Result<GeneratedMedia> {
        let output_path = format!("/tmp/smartassist_tts_{}.wav", uuid::Uuid::new_v4());

        // Detect available CLI tool
        if which("say").await {
            // macOS say
            let _ = Command::new("say")
                .arg(&request.text)
                .arg("-o")
                .arg(&output_path)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        } else if which("espeak").await {
            // Linux espeak
            let _ = Command::new("espeak")
                .arg("-w")
                .arg(&output_path)
                .arg(&request.text)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        } else if which("piper").await {
            // Piper local neural TTS
            let mut child = match Command::new("piper")
                .arg("--output_file")
                .arg(&output_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => {
                    return Err(crate::ProviderError::unsupported(
                        "Failed to spawn piper TTS process",
                    ));
                }
            };
            {
                use tokio::io::AsyncWriteExt;
                if let Some(mut stdin) = child.stdin.take() {
                    let text = request.text.clone();
                    tokio::spawn(async move {
                        let _ = stdin.write_all(text.as_bytes()).await;
                    });
                }
            }
            let _ = child.wait().await;
        } else {
            return Err(crate::ProviderError::unsupported(
                "No local TTS tool found (tried say, espeak, piper)",
            ));
        }

        let bytes = tokio::fs::read(&output_path).await.map_err(|e| {
            crate::ProviderError::internal(format!("Failed to read TTS output: {}", e))
        })?;
        let byte_count = bytes.len();

        Ok(GeneratedMedia {
            url: None,
            path: Some(output_path),
            base64_data: Some(base64::engine::general_purpose::STANDARD.encode(&bytes)),
            mime_type: "audio/wav".to_string(),
            size_bytes: Some(byte_count),
            metadata: std::collections::HashMap::new(),
        })
    }

    async fn list_voices(&self) -> Result<Vec<MediaModelInfo>> {
        Ok(vec![
            MediaModelInfo {
                id: "default".to_string(),
                name: "System Default".to_string(),
                kind: MediaModelKind::Tts,
                formats: vec!["wav".to_string()],
                max_input_length: Some(10000),
            },
        ])
    }

    fn capabilities(&self) -> MediaProviderCapabilities {
        MediaProviderCapabilities {
            streaming: false,
            batch: false,
            formats: vec!["wav".to_string()],
            max_prompt_length: Some(10000),
        }
    }
}

async fn which(cmd: &str) -> bool {
    Command::new("which")
        .arg(cmd)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_tts_provider_creation() {
        let provider = LocalTtsProvider::new();
        assert_eq!(provider.name(), "local_tts");
    }

    #[test]
    fn test_local_tts_default() {
        let provider = LocalTtsProvider::default();
        assert_eq!(provider.name(), "local_tts");
    }

    #[test]
    fn test_capabilities() {
        let provider = LocalTtsProvider::new();
        let caps = provider.capabilities();
        assert_eq!(caps.max_prompt_length, Some(10000));
    }

    #[tokio::test]
    async fn test_list_voices() {
        let provider = LocalTtsProvider::new();
        let voices = provider.list_voices().await.unwrap();
        assert_eq!(voices.len(), 1);
    }
}
