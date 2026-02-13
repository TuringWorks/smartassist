//! Messaging tools.
//!
//! - [`MessageTool`] - Send messages through channels
//! - Session management tools

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Message send callback type.
pub type MessageSender = Box<dyn Fn(MessageRequest) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::result::Result<MessageResponse, String>> + Send>> + Send + Sync>;

/// Message request.
#[derive(Debug, Clone)]
pub struct MessageRequest {
    /// Channel to send through.
    pub channel: String,
    /// Recipient ID.
    pub recipient: String,
    /// Message text.
    pub text: String,
    /// Reply to message ID.
    pub reply_to: Option<String>,
}

/// Message response.
#[derive(Debug, Clone)]
pub struct MessageResponse {
    /// Message ID assigned by the channel.
    pub message_id: Option<String>,
}

/// Message tool - Send messages through configured channels.
pub struct MessageTool {
    /// Message sender callback.
    sender: Option<std::sync::Arc<MessageSender>>,
    /// Default channel to use if not specified.
    default_channel: Option<String>,
}

impl Default for MessageTool {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageTool {
    /// Create a new message tool.
    pub fn new() -> Self {
        Self {
            sender: None,
            default_channel: None,
        }
    }

    /// Set the message sender.
    pub fn with_sender(mut self, sender: MessageSender) -> Self {
        self.sender = Some(std::sync::Arc::new(sender));
        self
    }

    /// Set the default channel.
    pub fn with_default_channel(mut self, channel: impl Into<String>) -> Self {
        self.default_channel = Some(channel.into());
        self
    }
}

#[async_trait]
impl Tool for MessageTool {
    fn name(&self) -> &str {
        "message"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "message".to_string(),
            description: "Send a message through a messaging channel (Telegram, Discord, Slack, etc.)".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The message text to send"
                    },
                    "channel": {
                        "type": "string",
                        "description": "Channel to send through (telegram, discord, slack, signal, imessage, whatsapp, line, web)"
                    },
                    "recipient": {
                        "type": "string",
                        "description": "Recipient ID (chat ID, user ID, phone number, etc.)"
                    },
                    "reply_to": {
                        "type": "string",
                        "description": "Message ID to reply to (optional)"
                    },
                    "media": {
                        "type": "object",
                        "properties": {
                            "type": {
                                "type": "string",
                                "enum": ["image", "audio", "video", "document"],
                                "description": "Type of media"
                            },
                            "path": {
                                "type": "string",
                                "description": "Path to the media file"
                            },
                            "url": {
                                "type": "string",
                                "description": "URL of the media file"
                            },
                            "caption": {
                                "type": "string",
                                "description": "Caption for the media"
                            }
                        }
                    }
                },
                "required": ["text"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let text = args
            .get("text")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'text' argument"))?;

        // Get channel from args, context, or default
        let channel_name = args
            .get("channel")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                context
                    .data
                    .get("channel")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .or_else(|| self.default_channel.clone())
            .ok_or_else(|| AgentError::tool_execution("No channel specified"))?;

        // Get recipient from args or context
        let recipient = args
            .get("recipient")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| {
                context
                    .data
                    .get("chat_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .ok_or_else(|| AgentError::tool_execution("No recipient specified"))?;

        let reply_to = args
            .get("reply_to")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        debug!(
            "Sending message via {}: {} chars to {}",
            channel_name,
            text.len(),
            recipient
        );

        // Check if sender is configured
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| AgentError::tool_execution("Message sender not configured"))?;

        // Build and send the request
        let request = MessageRequest {
            channel: channel_name.clone(),
            recipient: recipient.clone(),
            text: text.to_string(),
            reply_to,
        };

        let response = sender(request).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to send message: {}", e))
        })?;

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "channel": channel_name,
                "recipient": recipient,
                "message_id": response.message_id,
                "sent": true,
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom // Messaging tools use Custom group
    }
}

/// Session spawn tool - Create sub-agent sessions.
pub struct SessionsSpawnTool;

#[async_trait]
impl Tool for SessionsSpawnTool {
    fn name(&self) -> &str {
        "sessions_spawn"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "sessions_spawn".to_string(),
            description: "Spawn a new sub-agent session to handle a specific task".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "prompt": {
                        "type": "string",
                        "description": "The initial prompt/task for the sub-agent"
                    },
                    "model": {
                        "type": "string",
                        "description": "Model to use for the sub-agent (optional)"
                    },
                    "tools": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "List of tools to enable for the sub-agent"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds for the sub-agent"
                    }
                },
                "required": ["prompt"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let prompt = args
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'prompt' argument"))?;

        let model = args.get("model").and_then(|v| v.as_str());
        let _timeout = args.get("timeout").and_then(|v| v.as_u64());

        debug!("Spawning sub-agent session with prompt: {}", prompt);

        // Generate session ID
        let session_id = uuid::Uuid::new_v4().to_string();

        // TODO: Actually spawn the session through session manager
        // This is a placeholder that returns the session ID

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "session_id": session_id,
                "prompt": prompt,
                "model": model,
                "status": "spawned",
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Session
    }
}

/// Session send tool - Send a message to an existing session.
pub struct SessionsSendTool;

#[async_trait]
impl Tool for SessionsSendTool {
    fn name(&self) -> &str {
        "sessions_send"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "sessions_send".to_string(),
            description: "Send a message to an existing sub-agent session".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session ID to send to"
                    },
                    "message": {
                        "type": "string",
                        "description": "The message to send"
                    }
                },
                "required": ["session_id", "message"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'session_id' argument"))?;

        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'message' argument"))?;

        debug!("Sending message to session {}: {}", session_id, message);

        // TODO: Actually send to session through session manager

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "session_id": session_id,
                "sent": true,
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Session
    }
}

/// Session list tool - List active sessions.
pub struct SessionsListTool;

#[async_trait]
impl Tool for SessionsListTool {
    fn name(&self) -> &str {
        "sessions_list"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "sessions_list".to_string(),
            description: "List active sub-agent sessions".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["active", "paused", "completed", "all"],
                        "description": "Filter by session status"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let _status = args
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active");

        debug!("Listing sessions");

        // TODO: Actually list sessions from session manager

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "sessions": [],
                "count": 0,
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Session
    }
}

/// Session history tool - Get conversation history for a session.
pub struct SessionsHistoryTool;

#[async_trait]
impl Tool for SessionsHistoryTool {
    fn name(&self) -> &str {
        "sessions_history"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "sessions_history".to_string(),
            description: "Get the conversation history for a session".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session ID to get history for"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of messages to return (default: 50)"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Number of messages to skip (for pagination)"
                    }
                },
                "required": ["session_id"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'session_id' argument"))?;

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(50);
        let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);

        debug!(
            "Getting history for session {} (limit: {}, offset: {})",
            session_id, limit, offset
        );

        // TODO: Actually get history from session manager

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "session_id": session_id,
                "messages": [],
                "total": 0,
                "limit": limit,
                "offset": offset,
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Session
    }
}

/// Session status tool - Get current session status.
pub struct SessionStatusTool;

#[async_trait]
impl Tool for SessionStatusTool {
    fn name(&self) -> &str {
        "session_status"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "session_status".to_string(),
            description: "Get the current status of a session".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "session_id": {
                        "type": "string",
                        "description": "The session ID to get status for (optional, defaults to current session)"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| context.session_id.clone());

        debug!("Getting status for session {}", session_id);

        // TODO: Actually get status from session manager

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "session_id": session_id,
                "status": "active",
                "agent_id": context.agent_id,
                "message_count": 0,
                "created_at": chrono::Utc::now().to_rfc3339(),
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Session
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_tool_creation() {
        let tool = MessageTool::new();
        assert_eq!(tool.name(), "message");
    }

    #[test]
    fn test_sessions_spawn_tool_creation() {
        let tool = SessionsSpawnTool;
        assert_eq!(tool.name(), "sessions_spawn");
    }

    #[test]
    fn test_sessions_send_tool_creation() {
        let tool = SessionsSendTool;
        assert_eq!(tool.name(), "sessions_send");
    }

    #[test]
    fn test_sessions_list_tool_creation() {
        let tool = SessionsListTool;
        assert_eq!(tool.name(), "sessions_list");
    }

    #[test]
    fn test_sessions_history_tool_creation() {
        let tool = SessionsHistoryTool;
        assert_eq!(tool.name(), "sessions_history");
    }

    #[test]
    fn test_session_status_tool_creation() {
        let tool = SessionStatusTool;
        assert_eq!(tool.name(), "session_status");
    }
}
