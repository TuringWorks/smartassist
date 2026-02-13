//! Planning tools for structured task planning.
//!
//! Provides tools for entering and exiting plan mode, allowing agents
//! to explore codebases and design implementation approaches.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;

/// Planning mode state.
#[derive(Debug, Clone, Default)]
pub struct PlanState {
    /// Whether currently in plan mode.
    pub in_plan_mode: bool,
    /// Current plan content.
    pub plan_content: Option<String>,
    /// Plan file path.
    pub plan_file: Option<String>,
}

/// Shared planning state.
pub type SharedPlanState = Arc<RwLock<PlanState>>;

/// Tool to enter planning mode.
pub struct EnterPlanModeTool {
    state: SharedPlanState,
}

impl EnterPlanModeTool {
    pub fn new(state: SharedPlanState) -> Self {
        Self { state }
    }

    pub fn with_new_state() -> Self {
        Self {
            state: Arc::new(RwLock::new(PlanState::default())),
        }
    }
}

#[async_trait]
impl Tool for EnterPlanModeTool {
    fn name(&self) -> &str {
        "enter_plan_mode"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "enter_plan_mode".to_string(),
            description: "Enter planning mode to design an implementation approach. \
                         Use this before starting non-trivial implementation tasks."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        _args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        {
            let mut state = self.state.write().await;
            if state.in_plan_mode {
                return Ok(ToolResult::error(
                    tool_use_id,
                    "Already in plan mode",
                ));
            }
            state.in_plan_mode = true;
            state.plan_content = None;
        }

        debug!("Entered plan mode");

        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "status": "entered",
                "message": "Now in plan mode. Explore the codebase and write your plan.",
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool to exit planning mode with the completed plan.
pub struct ExitPlanModeTool {
    state: SharedPlanState,
}

impl ExitPlanModeTool {
    pub fn new(state: SharedPlanState) -> Self {
        Self { state }
    }

    pub fn with_new_state() -> Self {
        Self {
            state: Arc::new(RwLock::new(PlanState::default())),
        }
    }
}

#[async_trait]
impl Tool for ExitPlanModeTool {
    fn name(&self) -> &str {
        "exit_plan_mode"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "exit_plan_mode".to_string(),
            description: "Exit planning mode after writing the plan. \
                         The plan will be presented to the user for approval."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "allowedPrompts": {
                        "type": "array",
                        "description": "Permissions needed to implement the plan",
                        "items": {
                            "type": "object",
                            "properties": {
                                "tool": {
                                    "type": "string",
                                    "description": "Tool this permission applies to"
                                },
                                "prompt": {
                                    "type": "string",
                                    "description": "Description of the action"
                                }
                            },
                            "required": ["tool", "prompt"]
                        }
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
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let plan_content;
        {
            let mut state = self.state.write().await;
            if !state.in_plan_mode {
                return Ok(ToolResult::error(
                    tool_use_id,
                    "Not in plan mode",
                ));
            }
            plan_content = state.plan_content.clone();
            state.in_plan_mode = false;
        }

        let allowed_prompts = args
            .get("allowedPrompts")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        debug!(
            "Exited plan mode with {} allowed prompts",
            allowed_prompts.len()
        );

        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "status": "exited",
                "message": "Plan submitted for user approval",
                "plan": plan_content,
                "allowedPrompts": allowed_prompts,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Allowed prompt for plan implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowedPrompt {
    /// Tool this permission applies to.
    pub tool: String,
    /// Description of the action.
    pub prompt: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enter_plan_mode_tool_creation() {
        let tool = EnterPlanModeTool::with_new_state();
        assert_eq!(tool.name(), "enter_plan_mode");
    }

    #[test]
    fn test_exit_plan_mode_tool_creation() {
        let tool = ExitPlanModeTool::with_new_state();
        assert_eq!(tool.name(), "exit_plan_mode");
    }

    #[tokio::test]
    async fn test_plan_mode_workflow() {
        let state = Arc::new(RwLock::new(PlanState::default()));
        let enter_tool = EnterPlanModeTool::new(state.clone());
        let exit_tool = ExitPlanModeTool::new(state.clone());
        let ctx = ToolContext::default();

        // Enter plan mode
        let result = enter_tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);

        // Verify we're in plan mode
        {
            let state = state.read().await;
            assert!(state.in_plan_mode);
        }

        // Exit plan mode
        let result = exit_tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);

        // Verify we're out of plan mode
        {
            let state = state.read().await;
            assert!(!state.in_plan_mode);
        }
    }

    #[tokio::test]
    async fn test_cannot_enter_twice() {
        let state = Arc::new(RwLock::new(PlanState::default()));
        let tool = EnterPlanModeTool::new(state.clone());
        let ctx = ToolContext::default();

        // First enter succeeds
        let result = tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);

        // Second enter fails
        let result = tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_cannot_exit_without_entering() {
        let tool = ExitPlanModeTool::with_new_state();
        let ctx = ToolContext::default();

        let result = tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(result.is_error);
    }
}
