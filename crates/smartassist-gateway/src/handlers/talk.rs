//! Talk / Voice RPC method handlers.
//!
//! Handles voice session lifecycle and push-to-talk operations.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Voice session info in list response.
#[derive(Debug, Serialize)]
pub struct TalkSessionInfo {
    /// Session ID.
    pub session_id: String,
    /// Current state.
    pub state: String,
    /// Source (local, mobile, etc.).
    pub source: String,
    /// Whether PTT is pressed.
    pub ptt_pressed: bool,
    /// Whether wake word is enabled.
    pub wake_word_enabled: bool,
    /// Session duration in seconds.
    pub duration_secs: u64,
}

/// Parameters for talk.start method.
#[derive(Debug, Deserialize)]
pub struct TalkStartParams {
    /// Source of the session (local, mobile, channel).
    pub source: Option<String>,
    /// Device ID if source is mobile.
    pub device_id: Option<String>,
    /// Enable wake word detection.
    pub wake_word: Option<String>,
}

/// Talk start handler.
pub struct TalkStartHandler {
    context: Arc<HandlerContext>,
}

impl TalkStartHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for TalkStartHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: TalkStartParams = params
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        debug!("Talk start request");

        let runtime = self
            .context
            .talk_runtime
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Talk runtime not available".to_string()))?;

        let source = match params.source.as_deref() {
            Some("mobile") => smartassist_talk::TalkSource::Mobile {
                device_id: params.device_id.unwrap_or_default(),
            },
            Some("channel") => smartassist_talk::TalkSource::Channel {
                channel_type: "voice".to_string(),
                channel_id: params.device_id.unwrap_or_default(),
            },
            Some("automation") => smartassist_talk::TalkSource::Automation {
                job_id: params.device_id.unwrap_or_default(),
            },
            _ => smartassist_talk::TalkSource::Local,
        };

        let session = runtime.spawn_session(source).await.map_err(|e| {
            GatewayError::Internal(format!("Failed to spawn voice session: {}", e))
        })?;

        session.start().await.map_err(|e| {
            GatewayError::Internal(format!("Failed to start voice session: {}", e))
        })?;

        if let Some(wake_word) = params.wake_word {
            session.enable_wake_word(wake_word).await.map_err(|e| {
                GatewayError::Internal(format!("Failed to enable wake word: {}", e))
            })?;
        }

        Ok(serde_json::json!({
            "session_id": session.id(),
            "state": "listening",
            "started": true,
        }))
    }
}

/// Parameters for talk.stop method.
#[derive(Debug, Deserialize)]
pub struct TalkStopParams {
    /// Session ID to stop.
    pub session_id: String,
    /// Reason for ending.
    pub reason: Option<String>,
}

/// Talk stop handler.
pub struct TalkStopHandler {
    context: Arc<HandlerContext>,
}

impl TalkStopHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for TalkStopHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: TalkStopParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Talk stop request: {}", params.session_id);

        let runtime = self
            .context
            .talk_runtime
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Talk runtime not available".to_string()))?;

        let reason = match params.reason.as_deref() {
            Some("timeout") => smartassist_talk::EndReason::Timeout,
            Some("error") => smartassist_talk::EndReason::Error {
                message: "User requested stop with error reason".to_string(),
            },
            _ => smartassist_talk::EndReason::UserStopped,
        };

        runtime
            .end_session(&params.session_id, reason)
            .await
            .map_err(|e| GatewayError::NotFound(format!("Session not found: {}", e)))?;

        Ok(serde_json::json!({
            "session_id": params.session_id,
            "stopped": true,
        }))
    }
}

/// Parameters for talk.ptt_press method.
#[derive(Debug, Deserialize)]
pub struct TalkPttPressParams {
    /// Session ID.
    pub session_id: String,
}

/// Talk PTT press handler.
pub struct TalkPttPressHandler {
    context: Arc<HandlerContext>,
}

impl TalkPttPressHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for TalkPttPressHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: TalkPttPressParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Talk PTT press: {}", params.session_id);

        let runtime = self
            .context
            .talk_runtime
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Talk runtime not available".to_string()))?;

        runtime
            .handle_ptt_press(&params.session_id)
            .await
            .map_err(|e| GatewayError::NotFound(format!("Session not found: {}", e)))?;

        Ok(serde_json::json!({
            "session_id": params.session_id,
            "ptt_pressed": true,
        }))
    }
}

/// Parameters for talk.ptt_release method.
#[derive(Debug, Deserialize)]
pub struct TalkPttReleaseParams {
    /// Session ID.
    pub session_id: String,
    /// Duration of the press in milliseconds.
    pub duration_ms: Option<u64>,
}

/// Talk PTT release handler.
pub struct TalkPttReleaseHandler {
    context: Arc<HandlerContext>,
}

impl TalkPttReleaseHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for TalkPttReleaseHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: TalkPttReleaseParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Talk PTT release: {}", params.session_id);

        let runtime = self
            .context
            .talk_runtime
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Talk runtime not available".to_string()))?;

        let duration_ms = params.duration_ms.unwrap_or(0);

        runtime
            .handle_ptt_release(&params.session_id, duration_ms)
            .await
            .map_err(|e| GatewayError::NotFound(format!("Session not found: {}", e)))?;

        Ok(serde_json::json!({
            "session_id": params.session_id,
            "ptt_released": true,
            "duration_ms": duration_ms,
        }))
    }
}

/// Parameters for talk.audio method.
#[derive(Debug, Deserialize)]
pub struct TalkAudioParams {
    /// Session ID.
    pub session_id: String,
    /// Audio data (base64 encoded).
    pub audio: String,
    /// Audio format (pcm_s16le, opus, mp3, wav). Optional, defaults to pcm_s16le.
    pub format: Option<String>,
}

/// Talk audio handler — receives audio chunk from a client.
pub struct TalkAudioHandler {
    context: Arc<HandlerContext>,
}

impl TalkAudioHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for TalkAudioHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: TalkAudioParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Talk audio: {} ({} bytes)", params.session_id, params.audio.len());

        let runtime = self
            .context
            .talk_runtime
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Talk runtime not available".to_string()))?;

        let audio_bytes = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            &params.audio,
        )
        .map_err(|e| GatewayError::InvalidParams(format!("Invalid base64 audio: {}", e)))?;

        runtime
            .handle_audio(&params.session_id, audio_bytes)
            .await
            .map_err(|e| GatewayError::NotFound(format!("Session not found: {}", e)))?;

        Ok(serde_json::json!({
            "session_id": params.session_id,
            "received": true,
            "bytes": params.audio.len(),
        }))
    }
}

/// Parameters for talk.status method.
#[derive(Debug, Deserialize)]
pub struct TalkStatusParams {
    /// Session ID.
    pub session_id: String,
}

/// Talk status handler.
pub struct TalkStatusHandler {
    context: Arc<HandlerContext>,
}

impl TalkStatusHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for TalkStatusHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: TalkStatusParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        let runtime = self
            .context
            .talk_runtime
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Talk runtime not available".to_string()))?;

        let session = runtime
            .get_session(&params.session_id)
            .ok_or_else(|| GatewayError::NotFound(format!("Session '{}' not found", params.session_id)))?;

        let state = session.state().await;
        let state_str = format!("{:?}", state).to_lowercase();

        Ok(serde_json::json!({
            "session_id": session.id(),
            "state": state_str,
            "source": format!("{:?}", session.source()),
            "ptt_pressed": session.is_ptt_pressed(),
            "wake_word_enabled": session.is_wake_word_enabled(),
            "duration_secs": session.duration().as_secs(),
        }))
    }
}

/// Talk list handler.
pub struct TalkListHandler {
    context: Arc<HandlerContext>,
}

impl TalkListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for TalkListHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Talk list request");

        let runtime = self
            .context
            .talk_runtime
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Talk runtime not available".to_string()))?;

        let ids = runtime.list_sessions();
        let mut sessions = Vec::new();

        for id in ids {
            if let Some(session) = runtime.get_session(&id) {
                sessions.push(TalkSessionInfo {
                    session_id: session.id().to_string(),
                    state: format!("{:?}", session.state().await).to_lowercase(),
                    source: format!("{:?}", session.source()),
                    ptt_pressed: session.is_ptt_pressed(),
                    wake_word_enabled: session.is_wake_word_enabled(),
                    duration_secs: session.duration().as_secs(),
                });
            }
        }

        Ok(serde_json::json!({
            "sessions": sessions,
            "count": sessions.len(),
        }))
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for TalkStopParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for TalkPttPressParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for TalkPttReleaseParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for TalkAudioParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for TalkStatusParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl Default for TalkStartParams {
    fn default() -> Self {
        Self {
            source: None,
            device_id: None,
            wake_word: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_context_with_runtime() -> Arc<HandlerContext> {
        let (runtime, _rx) = smartassist_talk::TalkRuntimeBuilder::default().build();
        let mut ctx = HandlerContext::new();
        ctx.talk_runtime = Some(Arc::new(runtime));
        Arc::new(ctx)
    }

    #[tokio::test]
    async fn test_talk_start_local() {
        let ctx = test_context_with_runtime();
        let handler = TalkStartHandler::new(ctx);
        let result = handler.call(None).await.unwrap();
        assert_eq!(result["started"], true);
        assert_eq!(result["state"], "listening");
        assert!(result["session_id"].as_str().unwrap().starts_with("talk-"));
    }

    #[tokio::test]
    async fn test_talk_start_mobile() {
        let ctx = test_context_with_runtime();
        let handler = TalkStartHandler::new(ctx);
        let params = serde_json::json!({
            "source": "mobile",
            "device_id": "iphone-123"
        });
        let result = handler.call(Some(params)).await.unwrap();
        assert_eq!(result["started"], true);
    }

    #[tokio::test]
    async fn test_talk_start_with_wake_word() {
        let ctx = test_context_with_runtime();
        let handler = TalkStartHandler::new(ctx);
        let params = serde_json::json!({
            "wake_word": "Hey SmartAssist"
        });
        let result = handler.call(Some(params)).await.unwrap();
        assert_eq!(result["started"], true);
    }

    #[tokio::test]
    async fn test_talk_stop() {
        let ctx = test_context_with_runtime();

        // Start a session
        let start = TalkStartHandler::new(ctx.clone());
        let result = start.call(None).await.unwrap();
        let session_id = result["session_id"].as_str().unwrap().to_string();

        // Stop it
        let stop = TalkStopHandler::new(ctx);
        let params = serde_json::json!({ "session_id": session_id });
        let result = stop.call(Some(params)).await.unwrap();
        assert_eq!(result["stopped"], true);
    }

    #[tokio::test]
    async fn test_talk_stop_missing_session() {
        let ctx = test_context_with_runtime();
        let handler = TalkStopHandler::new(ctx);
        let params = serde_json::json!({ "session_id": "nonexistent" });
        let result = handler.call(Some(params)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_talk_ptt_press_release() {
        let ctx = test_context_with_runtime();

        // Start a session
        let start = TalkStartHandler::new(ctx.clone());
        let result = start.call(None).await.unwrap();
        let session_id = result["session_id"].as_str().unwrap().to_string();

        // Press PTT
        let press = TalkPttPressHandler::new(ctx.clone());
        let params = serde_json::json!({ "session_id": session_id });
        let result = press.call(Some(params.clone())).await.unwrap();
        assert_eq!(result["ptt_pressed"], true);

        // Release PTT
        let release = TalkPttReleaseHandler::new(ctx);
        let params = serde_json::json!({
            "session_id": session_id,
            "duration_ms": 500
        });
        let result = release.call(Some(params)).await.unwrap();
        assert_eq!(result["ptt_released"], true);
        assert_eq!(result["duration_ms"], 500);
    }

    #[tokio::test]
    async fn test_talk_status() {
        let ctx = test_context_with_runtime();

        // Start a session
        let start = TalkStartHandler::new(ctx.clone());
        let result = start.call(None).await.unwrap();
        let session_id = result["session_id"].as_str().unwrap().to_string();

        let status = TalkStatusHandler::new(ctx);
        let params = serde_json::json!({ "session_id": session_id });
        let result = status.call(Some(params)).await.unwrap();
        assert_eq!(result["session_id"], session_id);
        assert_eq!(result["state"], "listening");
        assert_eq!(result["ptt_pressed"], false);
        assert_eq!(result["wake_word_enabled"], false);
    }

    #[tokio::test]
    async fn test_talk_status_not_found() {
        let ctx = test_context_with_runtime();
        let handler = TalkStatusHandler::new(ctx);
        let params = serde_json::json!({ "session_id": "missing" });
        let result = handler.call(Some(params)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_talk_list() {
        let ctx = test_context_with_runtime();

        // Start two sessions
        let start = TalkStartHandler::new(ctx.clone());
        start.call(None).await.unwrap();
        start.call(None).await.unwrap();

        let list = TalkListHandler::new(ctx);
        let result = list.call(None).await.unwrap();
        assert_eq!(result["count"], 2);
        let sessions = result["sessions"].as_array().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[tokio::test]
    async fn test_talk_list_empty() {
        let ctx = test_context_with_runtime();
        let handler = TalkListHandler::new(ctx);
        let result = handler.call(None).await.unwrap();
        assert_eq!(result["count"], 0);
        let sessions = result["sessions"].as_array().unwrap();
        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn test_talk_audio() {
        let ctx = test_context_with_runtime();

        // Start a session
        let start = TalkStartHandler::new(ctx.clone());
        let result = start.call(None).await.unwrap();
        let session_id = result["session_id"].as_str().unwrap().to_string();

        let audio = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            b"fake_audio_data",
        );

        let audio_handler = TalkAudioHandler::new(ctx);
        let params = serde_json::json!({
            "session_id": session_id,
            "audio": audio,
            "format": "pcm_s16le"
        });
        let result = audio_handler.call(Some(params)).await.unwrap();
        assert_eq!(result["received"], true);
        assert_eq!(result["session_id"], session_id);
    }

    #[tokio::test]
    async fn test_talk_start_no_runtime() {
        let ctx = Arc::new(HandlerContext::new());
        let handler = TalkStartHandler::new(ctx);
        let result = handler.call(None).await;
        assert!(result.is_err());
    }
}
