//! Agent RPC method handlers with tool-use loop.
//!
//! Handles agent execution and streaming, matching OpenClaw's agent
//! command runtime capabilities.

use super::{HandlerContext, SessionData};
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use smartassist_agent::ToolContext;
use smartassist_core::types::{ContentBlock, Message as CoreMessage, Role, ToolResult};
use smartassist_providers::{ChatOptions, Message, StopReason, ToolChoice, ToolDefinition};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};

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
    pub usage: Option<TokenUsageInfo>,
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

/// Token usage info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsageInfo {
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

/// Agent handler with tool-use loop.
pub struct AgentHandler {
    context: Arc<HandlerContext>,
}

impl AgentHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }

    /// Build provider messages from session history.
    fn build_messages(session: &SessionData, system: Option<&str>) -> Vec<Message> {
        let mut messages = Vec::new();
        if let Some(system) = system {
            messages.push(CoreMessage::system(system));
        }
        for msg in &session.messages {
            match msg.role {
                Role::User => messages.push(msg.clone()),
                Role::Assistant => messages.push(msg.clone()),
                Role::System => messages.push(msg.clone()),
                Role::Tool => messages.push(msg.clone()),
            }
        }
        messages
    }

    /// Get tool definitions from the registry, mapping to provider types.
    async fn get_tool_definitions(
        &self,
        filter: Option<&[String]>,
    ) -> Option<Vec<ToolDefinition>> {
        let registry = self.context.tool_registry.as_ref()?;
        let defs = registry.definitions().await;
        let defs: Vec<ToolDefinition> = defs
            .into_iter()
            .filter(|d| filter.map_or(true, |f| f.contains(&d.name)))
            .map(|d| ToolDefinition {
                name: d.name.clone(),
                description: d.description.clone(),
                input_schema: d.input_schema.clone(),
                execution: d.execution.clone(),
            })
            .collect();
        if defs.is_empty() {
            None
        } else {
            Some(defs)
        }
    }

    /// Execute a tool and return the result.
    async fn execute_tool(
        &self,
        id: &str,
        name: &str,
        input: serde_json::Value,
        session_key: &str,
        agent_id: &str,
    ) -> Result<ToolCallInfo> {
        let executor = self
            .context
            .tool_executor
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Tool executor not configured".to_string()))?;

        let tool_context = ToolContext {
            session_id: session_key.to_string(),
            agent_id: agent_id.to_string(),
            ..ToolContext::default()
        };

        debug!("Executing tool '{}' for session {}", name, session_key);

        let result: ToolResult = executor
            .execute(id, name, input.clone(), Some(&tool_context))
            .await
            .map_err(|e| GatewayError::Internal(format!("Tool execution failed: {}", e)))?;

        let success = !result.is_error;

        Ok(ToolCallInfo {
            id: id.to_string(),
            name: name.to_string(),
            input,
            output: Some(result.output),
            success,
        })
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
        let agent_id = params.agent_id.unwrap_or_else(|| "default".to_string());
        let max_turns = params.max_turns.unwrap_or(10);
        let system_prompt = params.system.as_deref();

        // Get or create session
        {
            let mut sessions = self.context.sessions.write().await;
            sessions.entry(session_key.clone()).or_insert_with(|| SessionData {
                key: session_key.clone(),
                agent_id: Some(agent_id.clone()),
                status: "active".to_string(),
                messages: Vec::new(),
                created_at: chrono::Utc::now(),
                last_activity: Some(chrono::Utc::now()),
            });

            // Add user message
            if let Some(session) = sessions.get_mut(&session_key) {
                session.messages.push(CoreMessage::user(&params.message));
                session.last_activity = Some(chrono::Utc::now());
            }
        }

        let mut all_tool_calls: Vec<ToolCallInfo> = Vec::new();
        let mut total_input_tokens: u64 = 0;
        let mut total_output_tokens: u64 = 0;
        let mut final_response = String::new();
        let mut stop_reason = "end_turn".to_string();

        // Tool-use loop
        if let Some(provider) = &self.context.provider {
            let model = params.model.as_deref().unwrap_or(&self.context.default_model);
            let tool_filter = params.tools.as_deref();

            for turn in 0..max_turns {
                debug!("Agent turn {}/{} for session {}", turn + 1, max_turns, session_key);

                // Build messages and get tool definitions
                let messages = {
                    let sessions = self.context.sessions.read().await;
                    let session = sessions.get(&session_key).unwrap();
                    Self::build_messages(session, system_prompt)
                };

                let tool_defs = self.get_tool_definitions(tool_filter).await;

                // Call provider
                let mut options = ChatOptions::with_max_tokens(4096);
                if let Some(defs) = tool_defs {
                    options = options.tools(defs).tool_choice(ToolChoice::Auto);
                }

                let response = provider.chat(model, &messages, Some(options)).await;

                match response {
                    Ok(response) => {
                        total_input_tokens += response.usage.input;
                        total_output_tokens += response.usage.output;

                        if response.has_tool_calls() {
                            stop_reason = "tool_use".to_string();

                            // Execute each tool call
                            for block in &response.tool_calls {
                                if let ContentBlock::ToolUse { id, name, input } = block {
                                    let tool_info = self
                                        .execute_tool(id, name, input.clone(), &session_key, &agent_id)
                                        .await?;

                                    // Append assistant message with tool use and tool result
                                    {
                                        let mut sessions = self.context.sessions.write().await;
                                        if let Some(session) = sessions.get_mut(&session_key) {
                                            session.messages.push(CoreMessage {
                                                role: Role::Assistant,
                                                content: response.content.clone(),
                                                name: None,
                                                tool_use_id: None,
                                                timestamp: chrono::Utc::now(),
                                            });
                                            session.messages.push(CoreMessage::tool_result(
                                                id,
                                                tool_info.output.as_ref().map(|o| o.to_string()).unwrap_or_default(),
                                                !tool_info.success,
                                            ));
                                            session.last_activity = Some(chrono::Utc::now());
                                        }
                                    }

                                    all_tool_calls.push(tool_info);
                                }
                            }
                        } else {
                            // Final text response
                            final_response = response.to_text();
                            stop_reason = match response.stop_reason {
                                StopReason::EndTurn => "end_turn".to_string(),
                                StopReason::MaxTokens => "max_tokens".to_string(),
                                StopReason::StopSequence => "stop_sequence".to_string(),
                                _ => "end_turn".to_string(),
                            };

                            // Store assistant message
                            {
                                let mut sessions = self.context.sessions.write().await;
                                if let Some(session) = sessions.get_mut(&session_key) {
                                    session.messages.push(CoreMessage {
                                        role: Role::Assistant,
                                        content: response.content.clone(),
                                        name: None,
                                        tool_use_id: None,
                                        timestamp: chrono::Utc::now(),
                                    });
                                    session.last_activity = Some(chrono::Utc::now());
                                }
                            }
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Provider error on turn {}: {}", turn + 1, e);
                        final_response = format!("Error: {}", e);
                        stop_reason = "error".to_string();
                        break;
                    }
                }
            }
        } else {
            // No provider configured, return echo
            final_response = format!("Echo: {} (no provider configured)", params.message);
        }

        let result = AgentTurnResult {
            session_key: session_key.clone(),
            response: final_response,
            tool_calls: all_tool_calls,
            usage: Some(TokenUsageInfo {
                input: total_input_tokens,
                output: total_output_tokens,
                cache_read: None,
                cache_write: None,
            }),
            done: true,
            stop_reason: Some(stop_reason),
        };

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

        // TODO: Implement actual streaming via WebSocket events

        Ok(serde_json::json!({
            "streaming": true,
            "message": "Streaming responses are delivered via WebSocket events",
            "session_key": params.session_key.unwrap_or_else(|| "default".to_string()),
        }))
    }
}

/// Parameters for agent stop/status methods.
#[derive(Debug, Deserialize)]
pub struct AgentSessionParams {
    /// Session key.
    pub session_key: String,
}

/// Agent stop handler.
pub struct AgentStopHandler {
    context: Arc<HandlerContext>,
}

impl AgentStopHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for AgentStopHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: AgentSessionParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Agent stop request for session: {}", params.session_key);

        let mut sessions = self.context.sessions.write().await;
        let session = sessions.get_mut(&params.session_key).ok_or_else(|| {
            GatewayError::NotFound(format!("Session '{}' not found", params.session_key))
        })?;

        session.status = "stopped".to_string();
        session.last_activity = Some(chrono::Utc::now());

        Ok(serde_json::json!({
            "session_key": params.session_key,
            "stopped": true,
            "status": "stopped",
        }))
    }
}

/// Agent status handler.
pub struct AgentStatusHandler {
    context: Arc<HandlerContext>,
}

impl AgentStatusHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for AgentStatusHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: AgentSessionParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Agent status request for session: {}", params.session_key);

        let sessions = self.context.sessions.read().await;
        let session = sessions.get(&params.session_key).ok_or_else(|| {
            GatewayError::NotFound(format!("Session '{}' not found", params.session_key))
        })?;

        Ok(serde_json::json!({
            "session_key": session.key,
            "agent_id": session.agent_id,
            "status": session.status,
            "message_count": session.messages.len(),
            "created_at": session.created_at.to_rfc3339(),
            "last_activity": session.last_activity.map(|t| t.to_rfc3339()),
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

impl TryFrom<serde_json::Value> for AgentSessionParams {
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
            usage: Some(TokenUsageInfo {
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