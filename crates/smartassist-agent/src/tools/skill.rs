//! Skill invocation tools.
//!
//! Provides tools for invoking registered skills (slash commands)
//! that extend agent capabilities.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;

/// A registered skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Skill name (used for invocation).
    pub name: String,
    /// Display name.
    pub display_name: String,
    /// Description of what the skill does.
    pub description: String,
    /// Whether the skill is user-invocable (via slash command).
    pub user_invocable: bool,
    /// Required arguments.
    pub required_args: Vec<String>,
    /// Optional arguments.
    pub optional_args: Vec<String>,
}

/// Skill registry for managing available skills.
#[derive(Debug, Default)]
pub struct SkillRegistry {
    skills: HashMap<String, Skill>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a skill.
    pub fn register(&mut self, skill: Skill) {
        self.skills.insert(skill.name.clone(), skill);
    }

    /// Unregister a skill.
    pub fn unregister(&mut self, name: &str) {
        self.skills.remove(name);
    }

    /// Get a skill by name.
    pub fn get(&self, name: &str) -> Option<&Skill> {
        self.skills.get(name)
    }

    /// List all skills.
    pub fn list(&self) -> Vec<&Skill> {
        self.skills.values().collect()
    }

    /// List user-invocable skills.
    pub fn list_user_invocable(&self) -> Vec<&Skill> {
        self.skills
            .values()
            .filter(|s| s.user_invocable)
            .collect()
    }

    /// Create a registry with default skills.
    pub fn with_defaults() -> Self {
        let mut registry = Self::new();

        // Add some common built-in skills
        registry.register(Skill {
            name: "commit".to_string(),
            display_name: "Commit".to_string(),
            description: "Create a git commit with the staged changes".to_string(),
            user_invocable: true,
            required_args: vec![],
            optional_args: vec!["message".to_string()],
        });

        registry.register(Skill {
            name: "review-pr".to_string(),
            display_name: "Review PR".to_string(),
            description: "Review a pull request".to_string(),
            user_invocable: true,
            required_args: vec![],
            optional_args: vec!["pr_number".to_string()],
        });

        registry.register(Skill {
            name: "init".to_string(),
            display_name: "Initialize".to_string(),
            description: "Initialize a new project".to_string(),
            user_invocable: true,
            required_args: vec![],
            optional_args: vec!["template".to_string()],
        });

        registry.register(Skill {
            name: "test".to_string(),
            display_name: "Run Tests".to_string(),
            description: "Run the test suite".to_string(),
            user_invocable: true,
            required_args: vec![],
            optional_args: vec!["filter".to_string()],
        });

        registry.register(Skill {
            name: "build".to_string(),
            display_name: "Build".to_string(),
            description: "Build the project".to_string(),
            user_invocable: true,
            required_args: vec![],
            optional_args: vec!["release".to_string()],
        });

        registry
    }
}

/// Shared skill registry.
pub type SharedSkillRegistry = Arc<RwLock<SkillRegistry>>;

/// Tool for invoking skills.
pub struct SkillTool {
    registry: SharedSkillRegistry,
}

impl SkillTool {
    pub fn new(registry: SharedSkillRegistry) -> Self {
        Self { registry }
    }

    pub fn with_defaults() -> Self {
        Self {
            registry: Arc::new(RwLock::new(SkillRegistry::with_defaults())),
        }
    }
}

#[async_trait]
impl Tool for SkillTool {
    fn name(&self) -> &str {
        "skill"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "skill".to_string(),
            description: "Invoke a registered skill (slash command). \
                         Use this to execute predefined workflows like /commit, /review-pr, etc."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "skill": {
                        "type": "string",
                        "description": "The skill name to invoke (e.g., 'commit', 'review-pr')"
                    },
                    "args": {
                        "type": "string",
                        "description": "Optional arguments for the skill"
                    }
                },
                "required": ["skill"]
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

        let skill_name = args
            .get("skill")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("skill name is required"))?;

        let skill_args = args.get("args").and_then(|v| v.as_str());

        let registry = self.registry.read().await;

        let skill = match registry.get(skill_name) {
            Some(s) => s.clone(),
            None => {
                let available: Vec<_> = registry.list_user_invocable()
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect();
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!(
                        "Skill '{}' not found. Available skills: {}",
                        skill_name,
                        available.join(", ")
                    ),
                ));
            }
        };

        debug!("Invoking skill: {} with args: {:?}", skill_name, skill_args);

        // In a real implementation, this would:
        // 1. Load the skill's prompt/instructions
        // 2. Execute the skill's workflow
        // 3. Return the result

        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "status": "invoked",
                "skill": skill.name,
                "display_name": skill.display_name,
                "description": skill.description,
                "args": skill_args,
                "message": format!("Skill '{}' invoked successfully", skill.name),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for listing available skills.
pub struct SkillListTool {
    registry: SharedSkillRegistry,
}

impl SkillListTool {
    pub fn new(registry: SharedSkillRegistry) -> Self {
        Self { registry }
    }

    pub fn with_defaults() -> Self {
        Self {
            registry: Arc::new(RwLock::new(SkillRegistry::with_defaults())),
        }
    }
}

#[async_trait]
impl Tool for SkillListTool {
    fn name(&self) -> &str {
        "skill_list"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "skill_list".to_string(),
            description: "List all available skills".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "user_invocable_only": {
                        "type": "boolean",
                        "default": true,
                        "description": "Only show user-invocable skills"
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

        let user_invocable_only = args
            .get("user_invocable_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let registry = self.registry.read().await;

        let skills: Vec<_> = if user_invocable_only {
            registry.list_user_invocable()
        } else {
            registry.list()
        };

        let skill_list: Vec<serde_json::Value> = skills
            .iter()
            .map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "display_name": s.display_name,
                    "description": s.description,
                    "user_invocable": s.user_invocable,
                })
            })
            .collect();

        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "skills": skill_list,
                "count": skill_list.len(),
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
    fn test_skill_registry() {
        let mut registry = SkillRegistry::new();

        registry.register(Skill {
            name: "test".to_string(),
            display_name: "Test".to_string(),
            description: "A test skill".to_string(),
            user_invocable: true,
            required_args: vec![],
            optional_args: vec![],
        });

        assert!(registry.get("test").is_some());
        assert!(registry.get("nonexistent").is_none());
        assert_eq!(registry.list().len(), 1);
    }

    #[test]
    fn test_skill_registry_defaults() {
        let registry = SkillRegistry::with_defaults();

        assert!(registry.get("commit").is_some());
        assert!(registry.get("review-pr").is_some());
        assert!(registry.get("test").is_some());
        assert!(registry.get("build").is_some());
    }

    #[test]
    fn test_skill_tool_creation() {
        let tool = SkillTool::with_defaults();
        assert_eq!(tool.name(), "skill");
    }

    #[test]
    fn test_skill_list_tool_creation() {
        let tool = SkillListTool::with_defaults();
        assert_eq!(tool.name(), "skill_list");
    }

    #[tokio::test]
    async fn test_skill_invoke() {
        let tool = SkillTool::with_defaults();
        let ctx = ToolContext::default();

        let args = serde_json::json!({
            "skill": "commit",
            "args": "-m 'test commit'"
        });

        let result = tool.execute("test_id", args, &ctx).await.unwrap();
        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_skill_invoke_not_found() {
        let tool = SkillTool::with_defaults();
        let ctx = ToolContext::default();

        let args = serde_json::json!({
            "skill": "nonexistent"
        });

        let result = tool.execute("test_id", args, &ctx).await.unwrap();
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_skill_list() {
        let tool = SkillListTool::with_defaults();
        let ctx = ToolContext::default();

        let result = tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);
    }
}
