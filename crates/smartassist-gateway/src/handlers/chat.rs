//! Chat RPC method handlers.

use super::{HandlerContext, SessionData};
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{Message, Role};
use smartassist_providers::{ChatOptions, ErrorClassifier};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};

/// Parameters for chat method.
#[derive(Debug, Deserialize)]
pub struct ChatParams {
    /// Message content.
    pub message: String,

    /// Session key (optional, uses default if not provided).
    pub session_key: Option<String>,

    /// Agent ID (optional).
    pub agent_id: Option<String>,

    /// Model override (optional).
    pub model: Option<String>,

    /// Enable streaming (optional).
    pub stream: Option<bool>,
}

/// Response from chat method.
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    /// Session key used.
    pub session_key: String,

    /// Response message.
    pub message: String,

    /// Token usage.
    pub usage: Option<TokenUsageInfo>,

    /// Message ID.
    pub message_id: Option<String>,
}

/// Token usage statistics.
#[derive(Debug, Serialize)]
pub struct TokenUsageInfo {
    pub input: u64,
    pub output: u64,
}

/// Chat method handler.
pub struct ChatHandler {
    context: Arc<HandlerContext>,
}

impl ChatHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ChatHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ChatParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Chat request: {} chars", params.message.len());

        let session_key = params.session_key.unwrap_or_else(|| "default".to_string());

        // Get or create session and add user message
        {
            let mut sessions = self.context.sessions.write().await;
            sessions.entry(session_key.clone()).or_insert_with(|| SessionData {
                key: session_key.clone(),
                agent_id: params.agent_id.clone(),
                status: "active".to_string(),
                messages: Vec::new(),
                created_at: chrono::Utc::now(),
                last_activity: Some(chrono::Utc::now()),
            });

            // Add user message
            if let Some(session) = sessions.get_mut(&session_key) {
                session.messages.push(Message::user(&params.message));
                session.last_activity = Some(chrono::Utc::now());
            }
        }

        // Build provider messages from session history
        let mut messages = {
            let sessions = self.context.sessions.read().await;
            let session = sessions.get(&session_key).unwrap();
            session.messages.clone()
        };

        // Apply context compression if configured
        if let Some(ref engine) = self.context.compression_engine {
            if engine.should_compact(&messages) {
                debug!("Context compression triggered for session {}", session_key);
                match engine.compact(&messages).await {
                    Ok((compacted, result)) => {
                        if let Some(r) = result {
                            debug!(
                                "Compressed: {} -> {} messages",
                                r.messages_before, r.messages_after
                            );
                        }
                        messages = compacted;
                    }
                    Err(e) => {
                        warn!("Context compression failed, using original messages: {}", e);
                    }
                }
            }
        }

        // Try to use the provider if available
        let (response_message, usage) = if let Some(provider) = &self.context.provider {
            let model = params.model.as_deref().unwrap_or(&self.context.default_model);
            let options = ChatOptions::with_max_tokens(4096);

            match provider.chat(model, &messages, Some(options)).await {
                Ok(response) => {
                    // Report success to credential pool
                    if let Some(ref pool) = self.context.credential_pool {
                        pool.report_success(provider.name()).await;
                    }

                    let text = response.to_text();

                    // Store assistant message in session
                    {
                        let mut sessions = self.context.sessions.write().await;
                        if let Some(session) = sessions.get_mut(&session_key) {
                            session.messages.push(Message {
                                role: Role::Assistant,
                                content: response.content,
                                name: None,
                                tool_use_id: None,
                                timestamp: chrono::Utc::now(),
                            });
                        }
                    }

                    (
                        text,
                        Some(TokenUsageInfo {
                            input: response.usage.input,
                            output: response.usage.output,
                        }),
                    )
                }
                Err(e) => {
                    warn!("Provider error: {}", e);

                    // Classify error and report to credential pool
                    if let Some(ref pool) = self.context.credential_pool {
                        let classified = ErrorClassifier::classify(&e);
                        let provider_name = provider.name();
                        match classified.action {
                            smartassist_providers::RecommendedAction::RotateCredential => {
                                pool.report_auth_failure(provider_name, classified.retry_after_secs.map(std::time::Duration::from_secs)).await;
                                warn!("Reported auth failure for provider '{}'", provider_name);
                            }
                            smartassist_providers::RecommendedAction::RetryWithBackoff => {
                                if classified.category == smartassist_providers::ErrorClass::RateLimit {
                                    pool.report_rate_limit(provider_name, std::time::Duration::from_secs(classified.retry_after_secs.unwrap_or(60))).await;
                                }
                            }
                            _ => {}
                        }
                    }

                    (format!("Error: {}", e), None)
                }
            }
        } else {
            // No provider configured, return echo
            (format!("Echo: {} (no provider configured)", params.message), None)
        };

        let response = ChatResponse {
            session_key: session_key.clone(),
            message: response_message,
            usage,
            message_id: Some(uuid::Uuid::new_v4().to_string()),
        };

        serde_json::to_value(response).map_err(|e| GatewayError::Internal(e.to_string()))
    }
}

/// Parameters for chat.history method.
#[derive(Debug, Deserialize)]
pub struct ChatHistoryParams {
    /// Session key.
    pub session_key: String,

    /// Maximum messages to return.
    pub limit: Option<usize>,

    /// Offset for pagination.
    pub offset: Option<usize>,
}

/// Chat history method handler.
pub struct ChatHistoryHandler {
    context: Arc<HandlerContext>,
}

impl ChatHistoryHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ChatHistoryHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ChatHistoryParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Chat history request for session: {}", params.session_key);

        let sessions = self.context.sessions.read().await;
        let session = sessions
            .get(&params.session_key)
            .ok_or_else(|| GatewayError::NotFound(format!("Session '{}' not found", params.session_key)))?;

        let limit = params.limit.unwrap_or(100);
        let offset = params.offset.unwrap_or(0);

        let messages: Vec<_> = session.messages
            .iter()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect();

        Ok(serde_json::json!({
            "session_key": params.session_key,
            "messages": messages,
            "total": session.messages.len(),
        }))
    }
}

/// Parameters for chat.abort method.
#[derive(Debug, Deserialize)]
pub struct ChatAbortParams {
    /// Session key.
    pub session_key: String,
}

/// Chat abort method handler.
pub struct ChatAbortHandler {
    context: Arc<HandlerContext>,
}

impl ChatAbortHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ChatAbortHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ChatAbortParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Chat abort request for session: {}", params.session_key);

        let mut sessions = self.context.sessions.write().await;
        let aborted = if let Some(session) = sessions.get_mut(&params.session_key) {
            session.status = "aborted".to_string();
            session.last_activity = Some(chrono::Utc::now());
            true
        } else {
            false
        };

        Ok(serde_json::json!({
            "session_key": params.session_key,
            "aborted": aborted,
        }))
    }
}

impl TryFrom<serde_json::Value> for ChatParams {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ChatHistoryParams {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ChatAbortParams {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_params_deserialize() {
        let json = serde_json::json!({
            "message": "Hello, world!",
            "session_key": "test-session"
        });

        let params: ChatParams = serde_json::from_value(json).unwrap();
        assert_eq!(params.message, "Hello, world!");
        assert_eq!(params.session_key, Some("test-session".to_string()));
    }

    #[tokio::test]
    async fn test_chat_abort_existing_session() {
        let ctx = Arc::new(HandlerContext::new());

        // Create a session
        {
            let mut sessions = ctx.sessions.write().await;
            sessions.insert("sess-1".to_string(), super::SessionData {
                key: "sess-1".to_string(),
                agent_id: None,
                status: "active".to_string(),
                messages: vec![],
                created_at: chrono::Utc::now(),
                last_activity: None,
            });
        }

        let handler = ChatAbortHandler::new(ctx.clone());
        let params = serde_json::json!({"session_key": "sess-1"});
        let result = handler.call(Some(params)).await.unwrap();
        assert_eq!(result["aborted"], true);

        // Verify session status changed
        let sessions = ctx.sessions.read().await;
        assert_eq!(sessions.get("sess-1").unwrap().status, "aborted");
    }

    #[tokio::test]
    async fn test_chat_abort_missing_session() {
        let ctx = Arc::new(HandlerContext::new());
        let handler = ChatAbortHandler::new(ctx);
        let params = serde_json::json!({"session_key": "missing"});
        let result = handler.call(Some(params)).await.unwrap();
        assert_eq!(result["aborted"], false);
    }
}