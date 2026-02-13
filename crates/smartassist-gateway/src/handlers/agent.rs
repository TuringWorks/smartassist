//! Agent RPC method handlers.
//!
//! Handles agent execution and streaming.

use super::{HandlerContext, SessionData};
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Agent turn result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTurnResult {
    /// Session key.
    pub session_key: String,
    /// Agent response.
    pub response: String,
    /// Tool calls made.
    pub tool_calls: Vec<ToolCallInfo>,
    /// Token usage.
    pub usage: Option<TokenUsage>,
    /// Whether agent is done.
    pub done: bool,
    /// Stop reason.
    pub stop_reason: Option<String>,
}

/// Tool call info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    /// Tool use ID.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Tool input.
    pub input: serde_json::Value,
    /// Tool output.
    pub output: Option<serde_json::Value>,
    /// Whether tool execution succeeded.
    pub success: bool,
}

/// Token usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    /// Input tokens.
    pub input: u64,
    /// Output tokens.
    pub output: u64,
    /// Cache read tokens.
    pub cache_read: Option<u64>,
    /// Cache write tokens.
    pub cache_write: Option<u64>,
}

/// Parameters for agent method.
#[derive(Debug, Deserialize)]
pub struct AgentParams {
    /// Message to send.
    pub message: String,
    /// Session key.
    pub session_key: Option<String>,
    /// Agent ID.
    pub agent_id: Option<String>,
    /// Model override.
    pub model: Option<String>,
    /// Maximum turns.
    pub max_turns: Option<u32>,
    /// Tools to enable.
    pub tools: Option<Vec<String>>,
    /// System prompt override.
    pub system: Option<String>,
}

/// Agent handler.
pub struct AgentHandler {
    context: Arc<HandlerContext>,
}

impl AgentHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for AgentHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: AgentParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Agent request: {} chars", params.message.len());

        let session_key = params.session_key.unwrap_or_else(|| "default".to_string());

        // Get or create session
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
                session.messages.push(serde_json::json!({
                    "role": "user",
                    "content": params.message,
                }));
                session.last_activity = Some(chrono::Utc::now());
            }
        }

        // TODO: Actually run the agent
        // For now, return a placeholder response

        let result = AgentTurnResult {
            session_key: session_key.clone(),
            response: format!("I received your message: {}", params.message),
            tool_calls: vec![],
            usage: Some(TokenUsage {
                input: params.message.len() as u64,
                output: 50,
                cache_read: None,
                cache_write: None,
            }),
            done: true,
            stop_reason: Some("end_turn".to_string()),
        };

        // Add assistant message to session
        {
            let mut sessions = self.context.sessions.write().await;
            if let Some(session) = sessions.get_mut(&session_key) {
                session.messages.push(serde_json::json!({
                    "role": "assistant",
                    "content": result.response,
                }));
            }
        }

        Ok(serde_json::to_value(result).unwrap())
    }
}

/// Agent stream handler - for streaming responses.
pub struct AgentStreamHandler {
    _context: Arc<HandlerContext>,
}

impl AgentStreamHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for AgentStreamHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: AgentParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Agent stream request: {} chars", params.message.len());

        // TODO: Implement actual streaming
        // For now, return an error indicating streaming should be done via WebSocket events

        Ok(serde_json::json!({
            "streaming": true,
            "message": "Streaming responses are delivered via WebSocket events",
            "session_key": params.session_key.unwrap_or_else(|| "default".to_string()),
        }))
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for AgentParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_turn_result_serialization() {
        let result = AgentTurnResult {
            session_key: "test-session".to_string(),
            response: "Hello!".to_string(),
            tool_calls: vec![],
            usage: Some(TokenUsage {
                input: 10,
                output: 5,
                cache_read: None,
                cache_write: None,
            }),
            done: true,
            stop_reason: Some("end_turn".to_string()),
        };

        let json = serde_json::to_value(&result).unwrap();
        assert_eq!(json["session_key"], "test-session");
        assert_eq!(json["done"], true);
    }

    #[test]
    fn test_tool_call_info_serialization() {
        let tool_call = ToolCallInfo {
            id: "tc-1".to_string(),
            name: "read".to_string(),
            input: serde_json::json!({"path": "/tmp/test.txt"}),
            output: Some(serde_json::json!({"content": "Hello"})),
            success: true,
        };

        let json = serde_json::to_value(&tool_call).unwrap();
        assert_eq!(json["name"], "read");
        assert_eq!(json["success"], true);
    }
}
