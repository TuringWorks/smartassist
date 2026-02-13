//! Agent configuration types.

use super::AgentId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Agent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent ID (normalized).
    pub id: AgentId,

    /// Display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Workspace directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_dir: Option<PathBuf>,

    /// Primary model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Fallback models.
    #[serde(default)]
    pub fallback_models: Vec<String>,

    /// System prompt override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,

    /// Thinking level.
    #[serde(default)]
    pub thinking_level: ThinkingLevel,

    /// Tool policy.
    #[serde(default)]
    pub tools: ToolPolicyConfig,

    /// Sandbox configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxConfig>,

    /// Subagent settings.
    #[serde(default)]
    pub subagents: SubagentConfig,

    /// Identity (name, emoji, avatar).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<AgentIdentity>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            id: AgentId::default(),
            name: None,
            workspace_dir: None,
            model: None,
            fallback_models: Vec::new(),
            system_prompt: None,
            thinking_level: ThinkingLevel::default(),
            tools: ToolPolicyConfig::default(),
            sandbox: None,
            subagents: SubagentConfig::default(),
            identity: None,
        }
    }
}

/// Extended thinking level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    Off,
    Minimal,
    #[default]
    Low,
    Medium,
    High,
    XHigh,
}

impl ThinkingLevel {
    /// Get the token budget for this thinking level.
    pub fn budget_tokens(&self) -> Option<usize> {
        match self {
            Self::Off => None,
            Self::Minimal => Some(1024),
            Self::Low => Some(4096),
            Self::Medium => Some(8192),
            Self::High => Some(16384),
            Self::XHigh => Some(32768),
        }
    }

    /// Check if thinking is enabled.
    pub fn is_enabled(&self) -> bool {
        !matches!(self, Self::Off)
    }
}

/// Tool policy configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolPolicyConfig {
    /// Tool profile.
    #[serde(default)]
    pub profile: ToolProfile,

    /// Explicit allow patterns.
    #[serde(default)]
    pub allow: Vec<String>,

    /// Explicit deny patterns.
    #[serde(default)]
    pub deny: Vec<String>,

    /// Additional allow (union with profile).
    #[serde(default)]
    pub also_allow: Vec<String>,
}

/// Tool profile presets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolProfile {
    /// Only session_status.
    Minimal,

    /// Filesystem, runtime, sessions, memory.
    Coding,

    /// Messaging with limited sessions.
    Messaging,

    /// All tools allowed.
    #[default]
    Full,
}

impl ToolProfile {
    /// Get the tools included in this profile.
    pub fn included_tools(&self) -> &'static [&'static str] {
        match self {
            Self::Minimal => &["session_status"],
            Self::Coding => &[
                "read", "write", "edit", "glob", "grep", "apply_patch",
                "exec", "process",
                "sessions_list", "sessions_history", "sessions_send", "sessions_spawn",
                "memory_search", "memory_get",
                "image",
            ],
            Self::Messaging => &[
                "message",
                "sessions_list", "sessions_send",
            ],
            Self::Full => &[], // All tools allowed
        }
    }

    /// Check if this profile allows all tools.
    pub fn allows_all(&self) -> bool {
        matches!(self, Self::Full)
    }
}

/// Sandbox configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Whether sandbox is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Sandbox profile.
    #[serde(default)]
    pub profile: SandboxProfile,

    /// Docker container name (if using Docker).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_name: Option<String>,

    /// Workspace directory in container.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_workdir: Option<PathBuf>,

    /// Resource limits.
    #[serde(default)]
    pub limits: ResourceLimits,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            profile: SandboxProfile::default(),
            container_name: None,
            container_workdir: None,
            limits: ResourceLimits::default(),
        }
    }
}

/// Sandbox security profile.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxProfile {
    /// Maximum isolation.
    Strict,

    /// Standard isolation.
    #[default]
    Standard,

    /// Relaxed for trusted tools.
    Trusted,

    /// No sandbox (requires approval).
    None,
}

/// Resource limits for sandboxed execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU seconds.
    pub max_cpu_seconds: u64,

    /// Maximum memory in bytes.
    pub max_memory_bytes: u64,

    /// Maximum number of processes.
    pub max_processes: u32,

    /// Maximum open files.
    pub max_open_files: u64,

    /// Maximum output bytes.
    pub max_output_bytes: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_cpu_seconds: 30,
            max_memory_bytes: 512 * 1024 * 1024, // 512MB
            max_processes: 10,
            max_open_files: 100,
            max_output_bytes: 1024 * 1024, // 1MB
        }
    }
}

/// Subagent configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubagentConfig {
    /// Allowed agent IDs that can be spawned.
    #[serde(default)]
    pub allow_agents: Vec<String>,

    /// Default model for spawned subagents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Default thinking level for subagents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<ThinkingLevel>,

    /// Tool policy overrides for subagents.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_policy: Option<ToolPolicyConfig>,
}

/// Agent identity for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Display name.
    pub name: String,

    /// Emoji identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emoji: Option<String>,

    /// Avatar URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,

    /// Theme color.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme_color: Option<String>,
}

/// Workspace files loaded for an agent.
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// Path to workspace directory.
    pub path: PathBuf,

    /// AGENTS.md content.
    pub agents_md: Option<String>,

    /// SOUL.md content.
    pub soul_md: Option<String>,

    /// TOOLS.md content.
    pub tools_md: Option<String>,

    /// IDENTITY.md content.
    pub identity_md: Option<String>,

    /// Additional markdown files.
    pub additional_files: HashMap<String, String>,
}

impl Workspace {
    /// Build system prompt from workspace files.
    pub fn build_system_prompt(&self) -> String {
        let mut prompt = String::new();

        if let Some(agents) = &self.agents_md {
            prompt.push_str(agents);
            prompt.push_str("\n\n");
        }

        if let Some(soul) = &self.soul_md {
            prompt.push_str(soul);
            prompt.push_str("\n\n");
        }

        if let Some(tools) = &self.tools_md {
            prompt.push_str(tools);
        }

        prompt
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_config_default_fields() {
        let config = AgentConfig::default();
        assert_eq!(config.id.as_str(), "default");
        assert!(config.name.is_none());
        assert!(config.workspace_dir.is_none());
        assert!(config.model.is_none());
        assert!(config.fallback_models.is_empty());
        assert!(config.system_prompt.is_none());
        assert_eq!(config.thinking_level, ThinkingLevel::Low);
        assert!(config.sandbox.is_none());
        assert!(config.identity.is_none());
    }

    #[test]
    fn test_thinking_level_budget_tokens() {
        assert_eq!(ThinkingLevel::Off.budget_tokens(), None);
        assert_eq!(ThinkingLevel::Minimal.budget_tokens(), Some(1024));
        assert_eq!(ThinkingLevel::Low.budget_tokens(), Some(4096));
        assert_eq!(ThinkingLevel::Medium.budget_tokens(), Some(8192));
        assert_eq!(ThinkingLevel::High.budget_tokens(), Some(16384));
        assert_eq!(ThinkingLevel::XHigh.budget_tokens(), Some(32768));
    }

    #[test]
    fn test_thinking_level_is_enabled() {
        assert!(!ThinkingLevel::Off.is_enabled());
        assert!(ThinkingLevel::Minimal.is_enabled());
        assert!(ThinkingLevel::Low.is_enabled());
        assert!(ThinkingLevel::Medium.is_enabled());
        assert!(ThinkingLevel::High.is_enabled());
        assert!(ThinkingLevel::XHigh.is_enabled());
    }

    #[test]
    fn test_thinking_level_default_is_low() {
        assert_eq!(ThinkingLevel::default(), ThinkingLevel::Low);
    }

    #[test]
    fn test_tool_profile_included_tools_minimal() {
        let tools = ToolProfile::Minimal.included_tools();
        assert_eq!(tools, &["session_status"]);
    }

    #[test]
    fn test_tool_profile_included_tools_coding() {
        let tools = ToolProfile::Coding.included_tools();
        assert!(tools.contains(&"read"));
        assert!(tools.contains(&"write"));
        assert!(tools.contains(&"exec"));
        assert!(tools.contains(&"memory_search"));
    }

    #[test]
    fn test_tool_profile_included_tools_messaging() {
        let tools = ToolProfile::Messaging.included_tools();
        assert!(tools.contains(&"message"));
        assert!(tools.contains(&"sessions_list"));
        assert!(tools.contains(&"sessions_send"));
    }

    #[test]
    fn test_tool_profile_full_returns_empty_slice() {
        // Full profile returns empty slice because all tools are allowed.
        let tools = ToolProfile::Full.included_tools();
        assert!(tools.is_empty());
    }

    #[test]
    fn test_tool_profile_allows_all() {
        assert!(ToolProfile::Full.allows_all());
        assert!(!ToolProfile::Minimal.allows_all());
        assert!(!ToolProfile::Coding.allows_all());
        assert!(!ToolProfile::Messaging.allows_all());
    }

    #[test]
    fn test_sandbox_profile_serde_roundtrip() {
        let profiles = [
            SandboxProfile::Strict,
            SandboxProfile::Standard,
            SandboxProfile::Trusted,
            SandboxProfile::None,
        ];
        for profile in &profiles {
            let json = serde_json::to_string(profile).unwrap();
            let parsed: SandboxProfile = serde_json::from_str(&json).unwrap();
            assert_eq!(*profile, parsed);
        }
    }

    #[test]
    fn test_resource_limits_default() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.max_cpu_seconds, 30);
        assert_eq!(limits.max_memory_bytes, 512 * 1024 * 1024);
        assert_eq!(limits.max_processes, 10);
        assert_eq!(limits.max_open_files, 100);
        assert_eq!(limits.max_output_bytes, 1024 * 1024);
    }

    #[test]
    fn test_agent_config_serde_roundtrip() {
        let config = AgentConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id.as_str(), config.id.as_str());
        assert_eq!(parsed.thinking_level, config.thinking_level);
        assert!(parsed.fallback_models.is_empty());
    }

    #[test]
    fn test_workspace_build_system_prompt() {
        let mut ws = Workspace::default();
        ws.agents_md = Some("You are an agent.".to_string());
        ws.soul_md = Some("Be helpful.".to_string());
        ws.tools_md = Some("Use tools wisely.".to_string());

        let prompt = ws.build_system_prompt();
        assert!(prompt.contains("You are an agent."));
        assert!(prompt.contains("Be helpful."));
        assert!(prompt.contains("Use tools wisely."));
    }
}
