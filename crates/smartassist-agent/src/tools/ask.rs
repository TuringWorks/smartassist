//! Ask user tools for interactive prompts.
//!
//! Provides tools for asking questions and getting user input during agent execution.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::debug;

/// Option for a question.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    /// Display label for the option.
    pub label: String,
    /// Description of what this option means.
    pub description: String,
}

/// A question to ask the user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Question {
    /// The question text.
    pub question: String,
    /// Short header/tag for the question.
    pub header: String,
    /// Available options.
    pub options: Vec<QuestionOption>,
    /// Whether multiple options can be selected.
    #[serde(default)]
    pub multi_select: bool,
}

/// Tool for asking user questions during agent execution.
pub struct AskUserTool;

impl AskUserTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AskUserTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AskUserTool {
    fn name(&self) -> &str {
        "ask_user"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "ask_user".to_string(),
            description: "Ask the user a question with multiple choice options. \
                         Use this to gather preferences, clarify requirements, or get decisions."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "questions": {
                        "type": "array",
                        "description": "Questions to ask (1-4 questions)",
                        "items": {
                            "type": "object",
                            "properties": {
                                "question": {
                                    "type": "string",
                                    "description": "The question to ask"
                                },
                                "header": {
                                    "type": "string",
                                    "description": "Short label (max 12 chars)"
                                },
                                "options": {
                                    "type": "array",
                                    "description": "Available choices (2-4 options)",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "label": {
                                                "type": "string",
                                                "description": "Option display text"
                                            },
                                            "description": {
                                                "type": "string",
                                                "description": "Option explanation"
                                            }
                                        },
                                        "required": ["label", "description"]
                                    },
                                    "minItems": 2,
                                    "maxItems": 4
                                },
                                "multiSelect": {
                                    "type": "boolean",
                                    "default": false,
                                    "description": "Allow multiple selections"
                                }
                            },
                            "required": ["question", "header", "options"]
                        },
                        "minItems": 1,
                        "maxItems": 4
                    }
                },
                "required": ["questions"]
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

        let questions = args
            .get("questions")
            .and_then(|v| v.as_array())
            .ok_or_else(|| crate::error::AgentError::tool_execution("questions array is required"))?;

        if questions.is_empty() || questions.len() > 4 {
            return Ok(ToolResult::error(
                tool_use_id,
                "Must provide 1-4 questions",
            ));
        }

        // Parse and validate questions
        let mut parsed_questions: Vec<Question> = Vec::new();
        for (i, q) in questions.iter().enumerate() {
            let question_text = q
                .get("question")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    crate::error::AgentError::tool_execution(format!(
                        "Question {} missing 'question' field",
                        i + 1
                    ))
                })?;

            let header = q
                .get("header")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    crate::error::AgentError::tool_execution(format!(
                        "Question {} missing 'header' field",
                        i + 1
                    ))
                })?;

            if header.len() > 12 {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Question {} header exceeds 12 characters", i + 1),
                ));
            }

            let options = q
                .get("options")
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    crate::error::AgentError::tool_execution(format!(
                        "Question {} missing 'options' array",
                        i + 1
                    ))
                })?;

            if options.len() < 2 || options.len() > 4 {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Question {} must have 2-4 options", i + 1),
                ));
            }

            let mut parsed_options: Vec<QuestionOption> = Vec::new();
            for (j, opt) in options.iter().enumerate() {
                let label = opt
                    .get("label")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        crate::error::AgentError::tool_execution(format!(
                            "Question {} option {} missing 'label'",
                            i + 1,
                            j + 1
                        ))
                    })?;

                let description = opt
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                parsed_options.push(QuestionOption {
                    label: label.to_string(),
                    description: description.to_string(),
                });
            }

            let multi_select = q
                .get("multiSelect")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            parsed_questions.push(Question {
                question: question_text.to_string(),
                header: header.to_string(),
                options: parsed_options,
                multi_select,
            });
        }

        debug!("AskUser: {} questions prepared", parsed_questions.len());

        // In a real implementation, this would:
        // 1. Send the questions to the UI layer
        // 2. Wait for user response
        // 3. Return the selected options

        // For now, return a pending response indicating the question was sent
        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "status": "pending",
                "message": "Questions sent to user, awaiting response",
                "questions": parsed_questions,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for confirming an action with the user.
pub struct ConfirmTool;

impl ConfirmTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConfirmTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ConfirmTool {
    fn name(&self) -> &str {
        "confirm"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "confirm".to_string(),
            description: "Ask the user to confirm an action before proceeding.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "The confirmation message to display"
                    },
                    "action": {
                        "type": "string",
                        "description": "Description of the action to be confirmed"
                    },
                    "destructive": {
                        "type": "boolean",
                        "default": false,
                        "description": "Whether this is a destructive action"
                    }
                },
                "required": ["message"]
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

        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("message is required"))?;

        let action = args.get("action").and_then(|v| v.as_str());
        let destructive = args
            .get("destructive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        debug!(
            "Confirm: message='{}', action={:?}, destructive={}",
            message, action, destructive
        );

        // In a real implementation, this would:
        // 1. Display the confirmation dialog to the user
        // 2. Wait for user response (confirm/cancel)
        // 3. Return the user's choice

        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "status": "pending",
                "message": "Confirmation request sent to user",
                "prompt": message,
                "action": action,
                "destructive": destructive,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ask_user_tool_creation() {
        let tool = AskUserTool::new();
        assert_eq!(tool.name(), "ask_user");
    }

    #[test]
    fn test_ask_user_tool_definition() {
        let tool = AskUserTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "ask_user");
        assert!(def.description.contains("question"));
    }

    #[test]
    fn test_confirm_tool_creation() {
        let tool = ConfirmTool::new();
        assert_eq!(tool.name(), "confirm");
    }

    #[test]
    fn test_confirm_tool_definition() {
        let tool = ConfirmTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "confirm");
        assert!(def.description.contains("confirm"));
    }

    #[tokio::test]
    async fn test_ask_user_execute() {
        let tool = AskUserTool::new();
        let ctx = ToolContext::default();

        let args = serde_json::json!({
            "questions": [
                {
                    "question": "Which framework should we use?",
                    "header": "Framework",
                    "options": [
                        {"label": "React", "description": "Popular UI library"},
                        {"label": "Vue", "description": "Progressive framework"}
                    ]
                }
            ]
        });

        let result = tool.execute("test_id", args, &ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_confirm_execute() {
        let tool = ConfirmTool::new();
        let ctx = ToolContext::default();

        let args = serde_json::json!({
            "message": "Delete all files?",
            "action": "delete",
            "destructive": true
        });

        let result = tool.execute("test_id", args, &ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_ask_user_validation() {
        let tool = AskUserTool::new();
        let ctx = ToolContext::default();

        // Test too many questions
        let args = serde_json::json!({
            "questions": [
                {"question": "Q1", "header": "H1", "options": [{"label": "A", "description": ""}, {"label": "B", "description": ""}]},
                {"question": "Q2", "header": "H2", "options": [{"label": "A", "description": ""}, {"label": "B", "description": ""}]},
                {"question": "Q3", "header": "H3", "options": [{"label": "A", "description": ""}, {"label": "B", "description": ""}]},
                {"question": "Q4", "header": "H4", "options": [{"label": "A", "description": ""}, {"label": "B", "description": ""}]},
                {"question": "Q5", "header": "H5", "options": [{"label": "A", "description": ""}, {"label": "B", "description": ""}]}
            ]
        });

        let result = tool.execute("test_id", args, &ctx).await.unwrap();
        assert!(result.is_error);
    }
}
