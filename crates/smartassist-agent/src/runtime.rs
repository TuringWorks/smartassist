//! Agent runtime for executing conversations.

use crate::approval::ApprovalManager;
use crate::providers::{ModelProvider, StreamEvent};
use crate::session::{Session, SessionManager};
use crate::tools::{ToolContext, ToolExecutor, ToolRegistry};
use crate::Result;
use async_stream::stream;
use futures::Stream;
use smartassist_core::types::{
    AgentConfig, AgentId, Message, SessionKey, ThinkingLevel, TokenUsage,
};
use std::pin::Pin;
use std::sync::Arc;
use tracing::debug;

/// Configuration for the agent runtime.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Maximum turns per request.
    pub max_turns: usize,

    /// Maximum output tokens.
    pub max_output_tokens: usize,

    /// Temperature for generation.
    pub temperature: f32,

    /// Thinking level.
    pub thinking_level: ThinkingLevel,

    /// System prompt.
    pub system_prompt: Option<String>,

    /// Stop sequences.
    pub stop_sequences: Vec<String>,

    /// Enable tool use.
    pub enable_tools: bool,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_turns: 10,
            max_output_tokens: 4096,
            temperature: 0.7,
            thinking_level: ThinkingLevel::default(),
            system_prompt: None,
            stop_sequences: Vec::new(),
            enable_tools: true,
        }
    }
}

/// The agent runtime manages conversation execution.
pub struct AgentRuntime {
    /// Agent configuration.
    config: AgentConfig,

    /// Runtime configuration.
    runtime_config: RuntimeConfig,

    /// Model provider.
    provider: Arc<dyn ModelProvider>,

    /// Tool registry.
    tool_registry: Arc<ToolRegistry>,

    /// Tool executor.
    tool_executor: Arc<ToolExecutor>,

    /// Approval manager.
    approval_manager: Arc<ApprovalManager>,

    /// Session manager.
    session_manager: Arc<SessionManager>,
}

impl AgentRuntime {
    /// Create a new agent runtime.
    pub fn new(
        config: AgentConfig,
        provider: Arc<dyn ModelProvider>,
        tool_registry: Arc<ToolRegistry>,
        session_manager: Arc<SessionManager>,
    ) -> Self {
        let tool_executor = Arc::new(ToolExecutor::new(tool_registry.clone()));
        let approval_manager = Arc::new(ApprovalManager::new());

        Self {
            config,
            runtime_config: RuntimeConfig::default(),
            provider,
            tool_registry,
            tool_executor,
            approval_manager,
            session_manager,
        }
    }

    /// Set the runtime configuration.
    pub fn with_config(mut self, config: RuntimeConfig) -> Self {
        self.runtime_config = config;
        self
    }

    /// Set the approval manager.
    pub fn with_approval_manager(mut self, manager: Arc<ApprovalManager>) -> Self {
        self.approval_manager = manager;
        self
    }

    /// Get the agent ID.
    pub fn agent_id(&self) -> &AgentId {
        &self.config.id
    }

    /// Get the tool definitions.
    pub async fn tool_definitions(&self) -> Vec<smartassist_core::types::ToolDefinition> {
        self.tool_registry.definitions().await
    }

    /// Process a user message and return a response.
    pub async fn process_message(
        &self,
        session_key: &SessionKey,
        message: &str,
    ) -> Result<String> {
        let mut session = self
            .session_manager
            .get_or_create(session_key, &self.config.id)
            .await?;

        session.add_user_message(message);

        // Get response from model
        let response = self.get_model_response(&session).await?;

        // Add assistant response
        session.add_assistant_message(&response);

        // Save session
        self.session_manager.save(&session).await?;

        Ok(response)
    }

    /// Process a message with streaming response.
    pub fn process_message_stream(
        &self,
        session_key: SessionKey,
        message: String,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent>> + Send + '_>> {
        Box::pin(stream! {
            // Signal start
            yield Ok(StreamEvent::Start);

            // Get or create session
            let mut session = match self.session_manager.get_or_create(&session_key, &self.config.id).await {
                Ok(s) => s,
                Err(e) => {
                    yield Err(e);
                    return;
                }
            };

            session.add_user_message(&message);

            // Get response
            match self.get_model_response(&session).await {
                Ok(response) => {
                    // Stream the response as text deltas
                    yield Ok(StreamEvent::Text(response.clone()));

                    // Add to session
                    session.add_assistant_message(&response);

                    // Save session
                    if let Err(e) = self.session_manager.save(&session).await {
                        yield Err(e);
                        return;
                    }
                }
                Err(e) => {
                    yield Err(e);
                    return;
                }
            }

            // Signal completion
            yield Ok(StreamEvent::Done);
        })
    }

    /// Get a response from the model.
    async fn get_model_response(&self, session: &Session) -> Result<String> {
        let messages: Vec<Message> = session.messages.clone();
        let tools = if self.runtime_config.enable_tools {
            self.tool_registry.definitions().await
        } else {
            Vec::new()
        };

        let response = self.provider.complete(&messages, &tools).await?;

        // Extract text from response
        Ok(response.content.to_text())
    }

    /// Execute a tool use.
    pub async fn execute_tool(
        &self,
        tool_use_id: &str,
        tool_name: &str,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> Result<smartassist_core::types::ToolResult> {
        debug!("Executing tool: {} with id: {}", tool_name, tool_use_id);
        self.tool_executor
            .execute(tool_use_id, tool_name, input, Some(context))
            .await
    }

    /// Check if a tool requires approval.
    pub async fn tool_requires_approval(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
    ) -> Result<bool> {
        // Check tool-level approval requirement
        let tool_requires = self.tool_executor.requires_approval(tool_name, input).await?;

        // Check policy-level approval requirement
        let policy_requires = self.approval_manager.requires_approval(tool_name, input);

        Ok(tool_requires || policy_requires)
    }
}

/// A turn in a conversation.
#[derive(Debug, Clone)]
pub struct ConversationTurn {
    /// Turn number.
    pub turn_number: usize,

    /// User message (if this is a user turn).
    pub user_message: Option<String>,

    /// Assistant response.
    pub assistant_response: Option<String>,

    /// Tool uses in this turn.
    pub tool_uses: Vec<ToolUse>,

    /// Token usage for this turn.
    pub token_usage: TokenUsage,
}

/// A tool use in a turn.
#[derive(Debug, Clone)]
pub struct ToolUse {
    /// Tool use ID.
    pub id: String,

    /// Tool name.
    pub name: String,

    /// Input arguments.
    pub input: serde_json::Value,

    /// Result (if executed).
    pub result: Option<ToolUseResult>,
}

/// Result of a tool use.
#[derive(Debug, Clone)]
pub struct ToolUseResult {
    /// Output value.
    pub output: serde_json::Value,

    /// Whether it was an error.
    pub is_error: bool,

    /// Duration in milliseconds.
    pub duration_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_config_default() {
        let config = RuntimeConfig::default();
        assert_eq!(config.max_turns, 10);
        assert!(config.enable_tools);
    }
}
