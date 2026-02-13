//! Chat RPC method handlers.

use super::{HandlerContext, SessionData};
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use smartassist_providers::{ChatOptions, Message as ProviderMessage};
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
    pub usage: Option<TokenUsage>,

    /// Message ID.
    pub message_id: Option<String>,
}

/// Token usage statistics.
#[derive(Debug, Serialize)]
pub struct TokenUsage {
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

        // Get or create session and build message history
        let messages = {
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
                session.messages.push(serde_json::json!({
                    "role": "user",
                    "content": params.message,
                }));
                session.last_activity = Some(chrono::Utc::now());
            }

            // Build provider messages from session history
            let session = sessions.get(&session_key).unwrap();
            session.messages.iter().filter_map(|m| {
                let role = m.get("role")?.as_str()?;
                let content = m.get("content")?.as_str()?;
                match role {
                    "user" => Some(ProviderMessage::user(content)),
                    "assistant" => Some(ProviderMessage::assistant(content)),
                    "system" => Some(ProviderMessage::system(content)),
                    _ => None,
                }
            }).collect::<Vec<_>>()
        };

        // Try to use the provider if available
        let (response_message, usage) = if let Some(provider) = &self.context.provider {
            let model = params.model.as_deref().unwrap_or(&self.context.default_model);
            let options = ChatOptions::with_max_tokens(4096);

            match provider.chat(model, &messages, Some(options)).await {
                Ok(response) => {
                    // Store assistant message in session
                    {
                        let mut sessions = self.context.sessions.write().await;
                        if let Some(session) = sessions.get_mut(&session_key) {
                            session.messages.push(serde_json::json!({
                                "role": "assistant",
                                "content": response.content,
                            }));
                        }
                    }

                    (
                        response.content,
                        Some(TokenUsage {
                            input: response.usage.input_tokens as u64,
                            output: response.usage.output_tokens as u64,
                        }),
                    )
                }
                Err(e) => {
                    warn!("Provider error: {}", e);
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
    _context: Arc<HandlerContext>,
}

impl ChatAbortHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
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

        // TODO: Actually abort the running agent
        // For now, just acknowledge the request

        Ok(serde_json::json!({
            "session_key": params.session_key,
            "aborted": true,
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
}
