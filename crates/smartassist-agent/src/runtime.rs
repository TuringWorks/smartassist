//! Agent runtime for executing conversations.

use crate::approval::ApprovalManager;
use crate::compression::{CompressionConfig, CompressionEngine};
use crate::providers::{ChatResponse, Provider, StreamEvent};
use crate::session::{Session, SessionManager};
use crate::skills::ImprovementEngine;
use crate::tools::{GuardrailAction, GuardrailConfig, GuardrailEngine, ToolContext, ToolExecutor, ToolRegistry};
use crate::Result;
use async_stream::stream;
use futures::Stream;
use smartassist_core::types::{
    AgentConfig, AgentId, ChatOptions, Message, SessionKey, ThinkingLevel, TokenUsage,
};
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, info};

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
    provider: Arc<dyn Provider>,

    /// Tool registry.
    tool_registry: Arc<ToolRegistry>,

    /// Tool executor.
    tool_executor: Arc<ToolExecutor>,

    /// Approval manager.
    approval_manager: Arc<ApprovalManager>,

    /// Session manager.
    session_manager: Arc<SessionManager>,

    /// Context compression engine.
    compression: Option<CompressionEngine>,

    /// Guardrail engine for tool execution safety.
    guardrail: Option<Arc<GuardrailEngine>>,

    /// Improvement engine for self-improvement loop.
    improvement: Option<Arc<ImprovementEngine>>,
}

impl AgentRuntime {
    /// Create a new agent runtime.
    pub fn new(
        config: AgentConfig,
        provider: Arc<dyn Provider>,
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
            compression: None,
            guardrail: None,
            improvement: None,
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

    /// Enable context compression with the given configuration.
    pub fn with_compression(mut self, config: CompressionConfig) -> Self {
        self.compression = Some(CompressionEngine::with_provider(config, self.provider.clone()));
        self
    }

    /// Enable context compression with default configuration.
    pub fn with_default_compression(mut self) -> Self {
        self.compression = Some(CompressionEngine::new(CompressionConfig::default()));
        self
    }

    /// Enable guardrail engine with the given configuration.
    pub fn with_guardrail(mut self, config: GuardrailConfig) -> Self {
        let guardrail = GuardrailEngine::new(self.tool_executor.clone(), config);
        self.guardrail = Some(Arc::new(guardrail));
        self
    }

    /// Enable guardrail engine with default configuration.
    pub fn with_default_guardrail(mut self) -> Self {
        let guardrail = GuardrailEngine::with_defaults(self.tool_executor.clone());
        self.guardrail = Some(Arc::new(guardrail));
        self
    }

    /// Enable improvement engine for self-improvement.
    pub fn with_improvement(mut self, engine: ImprovementEngine) -> Self {
        self.improvement = Some(Arc::new(engine));
        self
    }

    /// Enable improvement engine with default configuration.
    pub fn with_default_improvement(mut self) -> Self {
        self.improvement = Some(Arc::new(ImprovementEngine::new()));
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
            yield Ok(StreamEvent::Start {
                id: uuid::Uuid::new_v4().to_string(),
                model: self.config.model.clone().unwrap_or_else(|| "claude-sonnet-4-20250514".to_string()),
            });

            // Get or create session
            let mut session = match self.session_manager.get_or_create(&session_key, &self.config.id).await {
                Ok(s) => s,
                Err(e) => {
                    yield Ok(StreamEvent::Error { message: e.to_string() });
                    return;
                }
            };

            session.add_user_message(&message);

            // Get response
            match self.get_model_response(&session).await {
                Ok(response) => {
                    // Stream the response as text deltas
                    yield Ok(StreamEvent::ContentDelta { delta: response.clone() });

                    // Add to session
                    session.add_assistant_message(&response);

                    // Save session
                    if let Err(e) = self.session_manager.save(&session).await {
                        yield Ok(StreamEvent::Error { message: e.to_string() });
                        return;
                    }

                    // Signal completion
                    yield Ok(StreamEvent::End {
                        stop_reason: smartassist_core::types::StopReason::EndTurn,
                        usage: smartassist_core::types::TokenUsage::default(),
                    });
                }
                Err(e) => {
                    yield Ok(StreamEvent::Error { message: e.to_string() });
                }
            }
        })
    }

    /// Get a response from the model.
    async fn get_model_response(&self, session: &Session) -> Result<String> {
        let mut messages: Vec<Message> = session.messages.clone();

        // Apply context compression if configured
        if let Some(compression) = &self.compression {
            if compression.should_compact(&messages) {
                info!(
                    "Context compression triggered: {} messages, {:.1}% usage",
                    messages.len(),
                    compression.usage_percent(&messages) * 100.0
                );
                match compression.compact(&messages).await {
                    Ok((compacted, result)) => {
                        if let Some(r) = result {
                            info!(
                                "Context compressed: {} -> {} messages, {} -> {} tokens",
                                r.messages_before,
                                r.messages_after,
                                r.tokens_before,
                                r.tokens_after,
                            );
                        }
                        messages = compacted;
                    }
                    Err(e) => {
                        debug!("Context compression failed, using original messages: {}", e);
                    }
                }
            }
        }

        let tools = if self.runtime_config.enable_tools {
            self.tool_registry.definitions().await
        } else {
            Vec::new()
        };

        let options = ChatOptions::with_max_tokens(self.runtime_config.max_output_tokens)
            .tools(tools);

        let response: ChatResponse = self
            .provider
            .chat(self.config.model.as_deref().unwrap_or("claude-sonnet-4-20250514"), &messages, Some(options))
            .await
            .map_err(|e| crate::error::AgentError::ModelApi(e.to_string()))?;

        Ok(response.content.to_text())
    }

    /// Execute a tool use, with guardrail protection and outcome recording.
    pub async fn execute_tool(
        &self,
        tool_use_id: &str,
        tool_name: &str,
        input: serde_json::Value,
        context: &ToolContext,
    ) -> Result<smartassist_core::types::ToolResult> {
        debug!("Executing tool: {} with id: {}", tool_name, tool_use_id);

        let result = if let Some(ref guardrail) = self.guardrail {
            guardrail
                .execute_guarded(tool_use_id, tool_name, input.clone(), context)
                .await?
        } else {
            self.tool_executor
                .execute(tool_use_id, tool_name, input.clone(), Some(context))
                .await?
        };

        // Record outcome in improvement engine
        if let Some(ref improvement) = self.improvement {
            if result.is_error {
                improvement
                    .record_failure(tool_name, &input.to_string(), &result.output.to_string())
                    .await;
            } else {
                improvement
                    .record_success(tool_name, &input.to_string())
                    .await;
            }
        }

        Ok(result)
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

    /// Check if a tool call would be allowed by guardrails.
    pub async fn check_guardrail(
        &self,
        session_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> Option<GuardrailAction> {
        if let Some(ref guardrail) = self.guardrail {
            Some(guardrail.check(session_id, tool_name, args).await)
        } else {
            None
        }
    }

    /// Get a reference to the improvement engine, if configured.
    pub fn improvement_engine(&self) -> Option<&Arc<ImprovementEngine>> {
        self.improvement.as_ref()
    }

    /// Get a reference to the guardrail engine, if configured.
    pub fn guardrail_engine(&self) -> Option<&Arc<GuardrailEngine>> {
        self.guardrail.as_ref()
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
