//! Tool plugin support.
//!
//! This module provides traits for plugins that add agent tools.

use crate::{PluginContext, Result};
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolGroup, ToolResult};
use std::collections::HashMap;
use std::sync::Arc;

/// Execution context for tools.
#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    /// Session ID.
    pub session_id: String,

    /// Agent ID.
    pub agent_id: String,

    /// Working directory.
    pub cwd: std::path::PathBuf,

    /// Environment variables.
    pub env: HashMap<String, String>,

    /// Additional context data.
    pub data: HashMap<String, serde_json::Value>,
}

impl Default for ToolExecutionContext {
    fn default() -> Self {
        Self {
            session_id: String::new(),
            agent_id: String::new(),
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")),
            env: std::env::vars().collect(),
            data: HashMap::new(),
        }
    }
}

/// Trait for a single tool implementation.
#[async_trait]
pub trait PluginTool: Send + Sync {
    /// Get the tool name.
    fn name(&self) -> &str;

    /// Get the tool definition.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool.
    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        ctx: &ToolExecutionContext,
    ) -> Result<ToolResult>;

    /// Check if the tool requires approval for the given arguments.
    fn requires_approval(&self, _args: &serde_json::Value) -> bool {
        false
    }

    /// Get the tool group.
    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Trait for plugins that provide tools.
#[async_trait]
pub trait ToolPlugin: Send + Sync {
    /// Get all tools provided by this plugin.
    fn tools(&self) -> Vec<Arc<dyn PluginTool>>;

    /// Get a tool by name.
    fn get_tool(&self, name: &str) -> Option<Arc<dyn PluginTool>> {
        self.tools().into_iter().find(|t| t.name() == name)
    }

    /// Get tool definitions for all tools.
    fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools().iter().map(|t| t.definition()).collect()
    }

    /// Get the tool factory.
    fn factory(&self) -> &dyn ToolPluginFactory;
}

/// Factory trait for creating tool instances.
#[async_trait]
pub trait ToolPluginFactory: Send + Sync {
    /// Create tools from plugin configuration.
    async fn create_tools(&self, ctx: &PluginContext) -> Result<Vec<Arc<dyn PluginTool>>>;

    /// Get the tool group for this factory.
    fn tool_group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Builder for tool plugins.
pub struct ToolPluginBuilder {
    name: String,
    tools: Vec<Arc<dyn PluginTool>>,
    group: ToolGroup,
}

impl ToolPluginBuilder {
    /// Create a new tool plugin builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            tools: Vec::new(),
            group: ToolGroup::Custom,
        }
    }

    /// Add a tool.
    pub fn with_tool(mut self, tool: Arc<dyn PluginTool>) -> Self {
        self.tools.push(tool);
        self
    }

    /// Set the tool group.
    pub fn with_group(mut self, group: ToolGroup) -> Self {
        self.group = group;
        self
    }

    /// Get the plugin name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the tools.
    pub fn tools(&self) -> &[Arc<dyn PluginTool>] {
        &self.tools
    }

    /// Get the tool group.
    pub fn group(&self) -> ToolGroup {
        self.group.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_execution_context_default() {
        let ctx = ToolExecutionContext::default();
        assert!(ctx.session_id.is_empty());
        assert!(ctx.agent_id.is_empty());
    }

    #[test]
    fn test_tool_plugin_builder() {
        let builder = ToolPluginBuilder::new("my-tools")
            .with_group(ToolGroup::FileSystem);

        assert_eq!(builder.name(), "my-tools");
        assert_eq!(builder.group(), ToolGroup::FileSystem);
        assert!(builder.tools().is_empty());
    }
}
