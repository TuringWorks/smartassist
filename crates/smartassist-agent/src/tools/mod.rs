//! Tool execution framework and built-in tools.
//!
//! This module provides:
//! - [`Tool`] trait for implementing tools
//! - [`ToolRegistry`] for managing available tools
//! - [`ToolExecutor`] for executing tools with sandboxing
//! - Built-in tools for file system, execution, and more

mod archive;
mod ask;
mod automation;
mod browser;
mod canvas;
mod channel_actions;
mod checksum;
mod compare;
mod context;
mod diagnostic;
mod diff;
mod encoding;
mod env;
mod fileops;
mod filesystem;
mod git;
mod http;
mod json;
mod lsp;
mod math;
mod media;
mod memory;
mod messaging;
mod network;
mod notebook;
mod plan;
mod process;
mod skill;
mod string;
mod system;
mod tasks;
mod template;
mod time;
mod util;
mod validate;
mod web;

pub use archive::{TarTool, ZipTool};
pub use ask::{AskUserTool, ConfirmTool};
pub use automation::{CronTool, GatewayTool, NodesTool};
pub use browser::BrowserTool;
pub use canvas::CanvasTool;
pub use channel_actions::{DiscordActionsTool, SlackActionsTool, TelegramActionsTool};
pub use checksum::{FileChecksumTool, FileVerifyTool};
pub use compare::{AssertTool, CompareTool, MatchTool, VersionCompareTool};
pub use context::{ContextAddTool, ContextClearTool, ContextGetTool, ContextStore, SharedContextStore};
pub use diagnostic::{DiagnosticTool, HealthCheckTool, SystemInfoTool};
pub use diff::{DiffTool, PatchTool};
pub use encoding::{Base64Tool, HashTool, HexTool, UrlEncodeTool};
pub use env::{EnvCheckTool, EnvGetTool, EnvListTool};
pub use fileops::{FileCopyTool, FileDeleteTool, FileMoveTool, FileStatTool};
pub use filesystem::{EditTool, GlobTool, GrepTool, ReadTool, WriteTool};
pub use git::{GitBranchTool, GitDiffTool, GitLogTool, GitStatusTool};
pub use http::{HttpRequestTool, UrlBuildTool, UrlParseTool};
pub use json::{JsonQueryTool, JsonTransformTool, YamlTool};
pub use lsp::LspTool;
pub use math::{CalcTool, RandomTool, UuidTool};
pub use media::{ImageTool, TtsTool};
pub use memory::{MemoryGetTool, MemoryIndexTool, MemorySearchTool, MemoryStoreTool};
pub use messaging::{
    MessageTool, SessionStatusTool, SessionsHistoryTool, SessionsListTool, SessionsSendTool,
    SessionsSpawnTool,
};
pub use network::{DnsLookupTool, HttpPingTool, NetInfoTool, PortCheckTool};
pub use notebook::NotebookEditTool;
pub use plan::{EnterPlanModeTool, ExitPlanModeTool, PlanState, SharedPlanState};
pub use process::{ProcessInfoTool, ProcessListTool};
pub use skill::{Skill, SkillListTool, SkillRegistry, SkillTool, SharedSkillRegistry};
pub use string::{CaseTool, ReplaceTool, SplitJoinTool, TrimPadTool};
pub use system::BashTool;
pub use tasks::{TaskCreateTool, TaskGetTool, TaskListTool, TaskStore, TaskUpdateTool};
pub use template::{FormatTool, TemplateTool};
pub use time::{DateCalcTool, DateParseTool, NowTool};
pub use util::{EchoTool, SleepTool, TempDirTool, TempFileTool};
pub use validate::{IsEmptyTool, ValidateTool};
pub use web::{WebFetchTool, WebSearchTool};

// Plugin adapter (bridges plugin SDK tools into the agent runtime)
// Note: PluginToolAdapter is defined inline below, not in a submodule.

use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::safety::SafetyLayer;
use smartassist_core::types::{ToolDefinition, ToolGroup, ToolResult};
use smartassist_sandbox::{CommandExecutor, ExecutionContext, SandboxProfile};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::debug;

/// Adapter that wraps a plugin tool to implement the agent `Tool` trait.
///
/// This bridges the plugin SDK's `PluginTool` interface with the agent runtime's
/// `Tool` interface, mapping between `ToolContext` and `ToolExecutionContext`.
pub struct PluginToolAdapter {
    inner: Arc<dyn smartassist_plugin_sdk::PluginTool>,
}

impl PluginToolAdapter {
    /// Create a new adapter wrapping a plugin tool.
    pub fn new(tool: Arc<dyn smartassist_plugin_sdk::PluginTool>) -> Self {
        Self { inner: tool }
    }
}

#[async_trait]
impl Tool for PluginToolAdapter {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let exec_ctx = smartassist_plugin_sdk::ToolExecutionContext {
            cwd: context.cwd.clone(),
            env: context.env.clone(),
            session_id: context.session_id.clone(),
            agent_id: context.agent_id.clone(),
            data: context.data.clone(),
        };
        self.inner
            .execute(tool_use_id, args, &exec_ctx)
            .await
            .map_err(|e| AgentError::tool_execution(e.to_string()))
    }

    fn requires_approval(&self, args: &serde_json::Value) -> bool {
        self.inner.requires_approval(args)
    }

    fn group(&self) -> ToolGroup {
        self.inner.group()
    }
}

/// A tool that can be executed by an agent.
#[async_trait]
pub trait Tool: Send + Sync {
    /// Get the tool name.
    fn name(&self) -> &str;

    /// Get the tool definition for the model.
    fn definition(&self) -> ToolDefinition;

    /// Execute the tool with given arguments.
    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult>;

    /// Check if the tool requires approval.
    fn requires_approval(&self, _args: &serde_json::Value) -> bool {
        false
    }

    /// Get the tool group.
    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Context for tool execution.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Working directory.
    pub cwd: std::path::PathBuf,

    /// Environment variables.
    pub env: HashMap<String, String>,

    /// Session ID.
    pub session_id: String,

    /// Agent ID.
    pub agent_id: String,

    /// Sandbox profile.
    pub sandbox_profile: SandboxProfile,

    /// Additional context data.
    pub data: HashMap<String, serde_json::Value>,
}

impl Default for ToolContext {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/")),
            env: std::env::vars().collect(),
            session_id: String::new(),
            agent_id: String::new(),
            sandbox_profile: SandboxProfile::standard(),
            data: HashMap::new(),
        }
    }
}

/// Registry for available tools.
pub struct ToolRegistry {
    /// Registered tools by name.
    tools: RwLock<HashMap<String, Arc<dyn Tool>>>,

    /// Tool groups.
    groups: RwLock<HashMap<ToolGroup, Vec<String>>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    /// Create a new tool registry.
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
            groups: RwLock::new(HashMap::new()),
        }
    }

    /// Create a registry with default tools.
    pub async fn with_defaults() -> Self {
        let registry = Self::new();

        // File system tools
        registry.register(Arc::new(ReadTool)).await;
        registry.register(Arc::new(WriteTool)).await;
        registry.register(Arc::new(EditTool::new())).await;
        registry.register(Arc::new(GlobTool::new())).await;
        registry.register(Arc::new(GrepTool::new())).await;

        // System tools
        registry.register(Arc::new(BashTool::new())).await;

        // Web tools
        registry.register(Arc::new(WebFetchTool::new())).await;
        registry.register(Arc::new(WebSearchTool::new())).await;

        // Messaging tools
        registry.register(Arc::new(MessageTool::new())).await;
        registry.register(Arc::new(SessionsSpawnTool)).await;
        registry.register(Arc::new(SessionsSendTool)).await;
        registry.register(Arc::new(SessionsListTool)).await;
        registry.register(Arc::new(SessionsHistoryTool)).await;
        registry.register(Arc::new(SessionStatusTool)).await;

        // Memory tools
        registry.register(Arc::new(MemorySearchTool::new())).await;
        registry.register(Arc::new(MemoryGetTool::new())).await;
        registry.register(Arc::new(MemoryStoreTool::new())).await;
        registry.register(Arc::new(MemoryIndexTool::new())).await;

        // Automation tools
        registry.register(Arc::new(CronTool::new())).await;
        registry.register(Arc::new(GatewayTool::new())).await;
        registry.register(Arc::new(NodesTool::new())).await;

        // Media tools
        registry.register(Arc::new(ImageTool::new())).await;
        registry.register(Arc::new(TtsTool::new())).await;

        // Browser tools
        registry.register(Arc::new(BrowserTool::new())).await;

        // Canvas tools
        registry.register(Arc::new(CanvasTool::new())).await;

        // Channel action tools
        registry.register(Arc::new(TelegramActionsTool::new())).await;
        registry.register(Arc::new(DiscordActionsTool::new())).await;
        registry.register(Arc::new(SlackActionsTool::new())).await;

        // Notebook tools
        registry.register(Arc::new(NotebookEditTool::new())).await;

        // LSP tools
        registry.register(Arc::new(LspTool::new())).await;

        // Task tools (shared store)
        let task_store = Arc::new(TaskStore::new());
        registry.register(Arc::new(TaskCreateTool::new(task_store.clone()))).await;
        registry.register(Arc::new(TaskListTool::new(task_store.clone()))).await;
        registry.register(Arc::new(TaskUpdateTool::new(task_store.clone()))).await;
        registry.register(Arc::new(TaskGetTool::new(task_store))).await;

        // Interactive tools
        registry.register(Arc::new(AskUserTool::new())).await;
        registry.register(Arc::new(ConfirmTool::new())).await;

        // Planning tools (shared state)
        let plan_state = Arc::new(tokio::sync::RwLock::new(PlanState::default()));
        registry.register(Arc::new(EnterPlanModeTool::new(plan_state.clone()))).await;
        registry.register(Arc::new(ExitPlanModeTool::new(plan_state))).await;

        // Skill tools (shared registry)
        let skill_registry = Arc::new(tokio::sync::RwLock::new(SkillRegistry::with_defaults()));
        registry.register(Arc::new(SkillTool::new(skill_registry.clone()))).await;
        registry.register(Arc::new(SkillListTool::new(skill_registry))).await;

        // Diagnostic tools
        registry.register(Arc::new(SystemInfoTool::new())).await;
        registry.register(Arc::new(HealthCheckTool::new())).await;
        registry.register(Arc::new(DiagnosticTool::new())).await;

        // Context tools (shared store)
        let context_store = Arc::new(RwLock::new(context::ContextStore::new()));
        registry.register(Arc::new(ContextAddTool::new(context_store.clone()))).await;
        registry.register(Arc::new(ContextGetTool::new(context_store.clone()))).await;
        registry.register(Arc::new(ContextClearTool::new(context_store))).await;

        // Diff tools
        registry.register(Arc::new(DiffTool::default())).await;
        registry.register(Arc::new(PatchTool::default())).await;

        // Git tools
        registry.register(Arc::new(GitStatusTool::new())).await;
        registry.register(Arc::new(GitLogTool::new())).await;
        registry.register(Arc::new(GitDiffTool::new())).await;
        registry.register(Arc::new(GitBranchTool::new())).await;

        // JSON/YAML tools
        registry.register(Arc::new(JsonQueryTool::new())).await;
        registry.register(Arc::new(JsonTransformTool::new())).await;
        registry.register(Arc::new(YamlTool::new())).await;

        // Encoding/hashing tools
        registry.register(Arc::new(Base64Tool::new())).await;
        registry.register(Arc::new(HexTool::new())).await;
        registry.register(Arc::new(HashTool::new())).await;
        registry.register(Arc::new(UrlEncodeTool::new())).await;

        // Time tools
        registry.register(Arc::new(NowTool::new())).await;
        registry.register(Arc::new(DateParseTool::new())).await;
        registry.register(Arc::new(DateCalcTool::new())).await;

        // String tools
        registry.register(Arc::new(CaseTool::new())).await;
        registry.register(Arc::new(SplitJoinTool::new())).await;
        registry.register(Arc::new(ReplaceTool::new())).await;
        registry.register(Arc::new(TrimPadTool::new())).await;

        // Math/random tools
        registry.register(Arc::new(CalcTool::new())).await;
        registry.register(Arc::new(RandomTool::new())).await;
        registry.register(Arc::new(UuidTool::new())).await;

        // Validation tools
        registry.register(Arc::new(ValidateTool::new())).await;
        registry.register(Arc::new(IsEmptyTool::new())).await;

        // Archive tools
        registry.register(Arc::new(ZipTool::new())).await;
        registry.register(Arc::new(TarTool::new())).await;

        // Network tools
        registry.register(Arc::new(DnsLookupTool::new())).await;
        registry.register(Arc::new(PortCheckTool::new())).await;
        registry.register(Arc::new(HttpPingTool::new())).await;
        registry.register(Arc::new(NetInfoTool::new())).await;

        // Environment tools
        registry.register(Arc::new(EnvGetTool::new())).await;
        registry.register(Arc::new(EnvListTool::new())).await;
        registry.register(Arc::new(EnvCheckTool::new())).await;

        // HTTP tools
        registry.register(Arc::new(HttpRequestTool::new())).await;
        registry.register(Arc::new(UrlParseTool::new())).await;
        registry.register(Arc::new(UrlBuildTool::new())).await;

        // Process tools
        registry.register(Arc::new(ProcessListTool::new())).await;
        registry.register(Arc::new(ProcessInfoTool::new())).await;

        // Utility tools
        registry.register(Arc::new(SleepTool::new())).await;
        registry.register(Arc::new(TempFileTool::new())).await;
        registry.register(Arc::new(TempDirTool::new())).await;
        registry.register(Arc::new(EchoTool::new())).await;

        // Checksum tools
        registry.register(Arc::new(FileChecksumTool::new())).await;
        registry.register(Arc::new(FileVerifyTool::new())).await;

        // Template tools
        registry.register(Arc::new(TemplateTool::new())).await;
        registry.register(Arc::new(FormatTool::new())).await;

        // File operation tools
        registry.register(Arc::new(FileCopyTool::new())).await;
        registry.register(Arc::new(FileMoveTool::new())).await;
        registry.register(Arc::new(FileStatTool::new())).await;
        registry.register(Arc::new(FileDeleteTool::new())).await;

        // Comparison tools
        registry.register(Arc::new(CompareTool::new())).await;
        registry.register(Arc::new(AssertTool::new())).await;
        registry.register(Arc::new(MatchTool::new())).await;
        registry.register(Arc::new(VersionCompareTool::new())).await;

        registry
    }

    /// Register a tool.
    pub async fn register(&self, tool: Arc<dyn Tool>) {
        let name = tool.name().to_string();
        let group = tool.group();

        let mut tools = self.tools.write().await;
        tools.insert(name.clone(), tool);

        let mut groups = self.groups.write().await;
        groups.entry(group).or_default().push(name);
    }

    /// Unregister a tool.
    pub async fn unregister(&self, name: &str) {
        let mut tools = self.tools.write().await;
        if let Some(tool) = tools.remove(name) {
            let group = tool.group();
            let mut groups = self.groups.write().await;
            if let Some(group_tools) = groups.get_mut(&group) {
                group_tools.retain(|n| n != name);
            }
        }
    }

    /// Get a tool by name.
    pub async fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read().await;
        tools.get(name).cloned()
    }

    /// List all tool names.
    pub async fn list(&self) -> Vec<String> {
        let tools = self.tools.read().await;
        tools.keys().cloned().collect()
    }

    /// List tools in a group.
    pub async fn list_group(&self, group: ToolGroup) -> Vec<String> {
        let groups = self.groups.read().await;
        groups.get(&group).cloned().unwrap_or_default()
    }

    /// Get all tool definitions.
    pub async fn definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools.values().map(|t| t.definition()).collect()
    }

    /// Get tool definitions for specific groups.
    pub async fn definitions_for_groups(&self, target_groups: &[ToolGroup]) -> Vec<ToolDefinition> {
        let tools = self.tools.read().await;
        tools
            .values()
            .filter(|t| target_groups.contains(&t.group()))
            .map(|t| t.definition())
            .collect()
    }

    /// Register tools from a plugin loader.
    ///
    /// Iterates over all loaded plugins that implement the `ToolPlugin` trait
    /// and registers each of their tools via a `PluginToolAdapter`.
    pub async fn register_plugin_tools(&self, loader: &smartassist_plugin_sdk::PluginLoader) {
        for plugin_meta in loader.list() {
            if let Some(plugin) = loader.get(&plugin_meta.name) {
                if let Some(tool_plugin) = plugin.as_tool_plugin() {
                    for tool in tool_plugin.tools() {
                        tracing::info!("Registered plugin tool: {}", tool.name());
                        self.register(Arc::new(PluginToolAdapter::new(tool))).await;
                    }
                }
            }
        }
    }
}

/// Tool executor with sandbox support and safety layer.
pub struct ToolExecutor {
    /// Tool registry.
    registry: Arc<ToolRegistry>,

    /// Default execution context.
    default_context: ToolContext,

    /// Command executor for shell tools.
    command_executor: Option<CommandExecutor>,

    /// Safety layer for input/output validation.
    safety: Option<SafetyLayer>,
}

impl ToolExecutor {
    /// Create a new tool executor.
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            default_context: ToolContext::default(),
            command_executor: None,
            safety: None,
        }
    }

    /// Set the default context.
    pub fn with_context(mut self, context: ToolContext) -> Self {
        self.default_context = context;
        self
    }

    /// Set up command executor with sandbox.
    pub fn with_sandbox(mut self, profile: SandboxProfile) -> Self {
        let exec_context = ExecutionContext::new(&self.default_context.cwd)
            .with_profile(profile)
            .with_envs(self.default_context.env.clone());
        self.command_executor = Some(CommandExecutor::new(exec_context));
        self
    }

    /// Enable the safety layer for input/output validation.
    pub fn with_safety(mut self, layer: SafetyLayer) -> Self {
        self.safety = Some(layer);
        self
    }

    /// Execute a tool by name.
    pub async fn execute(
        &self,
        tool_use_id: &str,
        name: &str,
        args: serde_json::Value,
        context: Option<&ToolContext>,
    ) -> Result<ToolResult> {
        let tool = self
            .registry
            .get(name)
            .await
            .ok_or_else(|| AgentError::ToolNotFound(name.to_string()))?;

        let ctx = context.unwrap_or(&self.default_context);

        // Pre-execution: validate and scan args
        if let Some(ref safety) = self.safety {
            safety.check_input(name, &args)?;
        }

        debug!("Executing tool '{}' with args: {:?}", name, args);
        let result = tool.execute(tool_use_id, args, ctx).await?;

        // Post-execution: scan output for leaks, wrap in XML
        if let Some(ref safety) = self.safety {
            let cleaned_output = safety.check_output(name, &result.output)?;
            return Ok(ToolResult {
                output: cleaned_output,
                ..result
            });
        }

        Ok(result)
    }

    /// Check if a tool requires approval.
    pub async fn requires_approval(&self, name: &str, args: &serde_json::Value) -> Result<bool> {
        let tool = self
            .registry
            .get(name)
            .await
            .ok_or_else(|| AgentError::ToolNotFound(name.to_string()))?;

        Ok(tool.requires_approval(args))
    }

    /// Execute a shell command (with sandboxing).
    pub async fn execute_command(
        &self,
        command: &str,
    ) -> Result<smartassist_core::types::ExecutionResult> {
        let executor = self
            .command_executor
            .as_ref()
            .ok_or_else(|| AgentError::config("Command executor not configured"))?;

        let start = Instant::now();
        let output = executor.execute(command).await?;
        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(smartassist_core::types::ExecutionResult {
            exit_code: output.exit_code,
            stdout: output.stdout,
            stderr: output.stderr,
            duration_ms,
            resource_usage: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tool_registry() {
        let registry = ToolRegistry::new();

        registry.register(Arc::new(BashTool::new())).await;
        registry.register(Arc::new(ReadTool)).await;

        let tools = registry.list().await;
        assert!(tools.contains(&"bash".to_string()));
        assert!(tools.contains(&"read".to_string()));
    }

    #[tokio::test]
    async fn test_registry_with_defaults() {
        let registry = ToolRegistry::with_defaults().await;
        let tools = registry.list().await;

        // Check file system tools
        assert!(tools.contains(&"read".to_string()));
        assert!(tools.contains(&"write".to_string()));
        assert!(tools.contains(&"edit".to_string()));
        assert!(tools.contains(&"glob".to_string()));
        assert!(tools.contains(&"grep".to_string()));

        // Check system tools
        assert!(tools.contains(&"bash".to_string()));

        // Check web tools
        assert!(tools.contains(&"web_fetch".to_string()));
        assert!(tools.contains(&"web_search".to_string()));

        // Check messaging tools
        assert!(tools.contains(&"message".to_string()));
        assert!(tools.contains(&"sessions_spawn".to_string()));

        // Check memory tools
        assert!(tools.contains(&"memory_search".to_string()));
        assert!(tools.contains(&"memory_get".to_string()));
        assert!(tools.contains(&"memory_store".to_string()));
        assert!(tools.contains(&"memory_index".to_string()));

        // Check automation tools
        assert!(tools.contains(&"cron".to_string()));
        assert!(tools.contains(&"gateway".to_string()));
        assert!(tools.contains(&"nodes".to_string()));

        // Check media tools
        assert!(tools.contains(&"image".to_string()));
        assert!(tools.contains(&"tts".to_string()));

        // Check browser tools
        assert!(tools.contains(&"browser".to_string()));

        // Check canvas tools
        assert!(tools.contains(&"canvas".to_string()));

        // Check channel action tools
        assert!(tools.contains(&"telegram_actions".to_string()));
        assert!(tools.contains(&"discord_actions".to_string()));
        assert!(tools.contains(&"slack_actions".to_string()));

        // Check notebook tools
        assert!(tools.contains(&"notebook_edit".to_string()));

        // Check LSP tools
        assert!(tools.contains(&"lsp".to_string()));

        // Check task tools
        assert!(tools.contains(&"task_create".to_string()));
        assert!(tools.contains(&"task_list".to_string()));
        assert!(tools.contains(&"task_update".to_string()));
        assert!(tools.contains(&"task_get".to_string()));

        // Check interactive tools
        assert!(tools.contains(&"ask_user".to_string()));
        assert!(tools.contains(&"confirm".to_string()));

        // Check planning tools
        assert!(tools.contains(&"enter_plan_mode".to_string()));
        assert!(tools.contains(&"exit_plan_mode".to_string()));

        // Check skill tools
        assert!(tools.contains(&"skill".to_string()));
        assert!(tools.contains(&"skill_list".to_string()));

        // Check diagnostic tools
        assert!(tools.contains(&"system_info".to_string()));
        assert!(tools.contains(&"health_check".to_string()));
        assert!(tools.contains(&"diagnostic".to_string()));

        // Check context tools
        assert!(tools.contains(&"context_add".to_string()));
        assert!(tools.contains(&"context_get".to_string()));
        assert!(tools.contains(&"context_clear".to_string()));

        // Check diff tools
        assert!(tools.contains(&"diff".to_string()));
        assert!(tools.contains(&"patch".to_string()));

        // Check git tools
        assert!(tools.contains(&"git_status".to_string()));
        assert!(tools.contains(&"git_log".to_string()));
        assert!(tools.contains(&"git_diff".to_string()));
        assert!(tools.contains(&"git_branch".to_string()));

        // Check JSON/YAML tools
        assert!(tools.contains(&"json_query".to_string()));
        assert!(tools.contains(&"json_transform".to_string()));
        assert!(tools.contains(&"yaml".to_string()));

        // Check encoding/hashing tools
        assert!(tools.contains(&"base64".to_string()));
        assert!(tools.contains(&"hex".to_string()));
        assert!(tools.contains(&"hash".to_string()));
        assert!(tools.contains(&"url_encode".to_string()));

        // Check time tools
        assert!(tools.contains(&"now".to_string()));
        assert!(tools.contains(&"date_parse".to_string()));
        assert!(tools.contains(&"date_calc".to_string()));

        // Check string tools
        assert!(tools.contains(&"case".to_string()));
        assert!(tools.contains(&"split_join".to_string()));
        assert!(tools.contains(&"replace".to_string()));
        assert!(tools.contains(&"trim_pad".to_string()));

        // Check math/random tools
        assert!(tools.contains(&"calc".to_string()));
        assert!(tools.contains(&"random".to_string()));
        assert!(tools.contains(&"uuid".to_string()));

        // Check validation tools
        assert!(tools.contains(&"validate".to_string()));
        assert!(tools.contains(&"is_empty".to_string()));

        // Check archive tools
        assert!(tools.contains(&"zip".to_string()));
        assert!(tools.contains(&"tar".to_string()));

        // Check network tools
        assert!(tools.contains(&"dns_lookup".to_string()));
        assert!(tools.contains(&"port_check".to_string()));
        assert!(tools.contains(&"http_ping".to_string()));
        assert!(tools.contains(&"net_info".to_string()));

        // Check environment tools
        assert!(tools.contains(&"env_get".to_string()));
        assert!(tools.contains(&"env_list".to_string()));
        assert!(tools.contains(&"env_check".to_string()));

        // Check HTTP tools
        assert!(tools.contains(&"http_request".to_string()));
        assert!(tools.contains(&"url_parse".to_string()));
        assert!(tools.contains(&"url_build".to_string()));

        // Check process tools
        assert!(tools.contains(&"process_list".to_string()));
        assert!(tools.contains(&"process_info".to_string()));

        // Check utility tools
        assert!(tools.contains(&"sleep".to_string()));
        assert!(tools.contains(&"temp_file".to_string()));
        assert!(tools.contains(&"temp_dir".to_string()));
        assert!(tools.contains(&"echo".to_string()));

        // Check checksum tools
        assert!(tools.contains(&"file_checksum".to_string()));
        assert!(tools.contains(&"file_verify".to_string()));

        // Check template tools
        assert!(tools.contains(&"template".to_string()));
        assert!(tools.contains(&"format".to_string()));

        // Check file operation tools
        assert!(tools.contains(&"file_copy".to_string()));
        assert!(tools.contains(&"file_move".to_string()));
        assert!(tools.contains(&"file_stat".to_string()));
        assert!(tools.contains(&"file_delete".to_string()));

        // Check comparison tools
        assert!(tools.contains(&"compare".to_string()));
        assert!(tools.contains(&"assert".to_string()));
        assert!(tools.contains(&"match".to_string()));
        assert!(tools.contains(&"version_compare".to_string()));

        // Total: 101 tools
        assert_eq!(tools.len(), 101);
    }
}
