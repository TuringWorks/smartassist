//! Configuration schema definitions.

use crate::secret::SecretString;
use crate::types::{
    AgentConfig, AuditConfig, DmPolicy, DmScope, ExecSecurityConfig,
    ResourceLimits, SandboxProfile, ThinkingLevel, ToolPolicyConfig,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main SmartAssist configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Agent configurations.
    #[serde(default)]
    pub agents: AgentsConfig,

    /// Channel configurations.
    #[serde(default)]
    pub channels: ChannelsConfig,

    /// Gateway settings.
    #[serde(default)]
    pub gateway: GatewayConfig,

    /// Session management.
    #[serde(default)]
    pub session: SessionConfig,

    /// Security settings.
    #[serde(default)]
    pub security: SecurityConfig,

    /// Memory/search settings.
    #[serde(default)]
    pub memory: MemoryConfig,

    /// Logging settings.
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Routing bindings.
    #[serde(default)]
    pub routing: RoutingConfig,
}

/// Agents configuration section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentsConfig {
    /// Default agent ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,

    /// Per-agent configurations.
    #[serde(default)]
    pub agents: HashMap<String, AgentConfig>,

    /// Default settings for all agents.
    #[serde(default)]
    pub defaults: AgentDefaults,
}

/// Default agent settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentDefaults {
    /// Default model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Model aliases.
    #[serde(default)]
    pub models: HashMap<String, String>,

    /// Default thinking level.
    #[serde(default)]
    pub thinking_level: ThinkingLevel,

    /// CLI backend credentials.
    #[serde(default)]
    pub cli_backends: HashMap<String, String>,

    /// Default tool policy.
    #[serde(default)]
    pub tools: ToolPolicyConfig,

    /// Cache settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache: Option<CacheConfig>,
}

/// Cache configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Cache type.
    #[serde(rename = "type")]
    pub cache_type: CacheType,

    /// TTL in minutes.
    #[serde(default = "default_cache_ttl")]
    pub ttl_minutes: u32,
}

fn default_cache_ttl() -> u32 {
    5
}

/// Cache type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheType {
    #[default]
    Prompt,
    Static,
}

/// Channels configuration section.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelsConfig {
    /// Telegram configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telegram: Option<TelegramConfig>,

    /// Discord configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord: Option<DiscordConfig>,

    /// Slack configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slack: Option<SlackConfig>,

    /// Signal configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<SignalConfig>,

    /// WhatsApp configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whatsapp: Option<WhatsAppConfig>,

    /// Extensions to load.
    #[serde(default)]
    pub extensions: Vec<String>,
}

/// Telegram channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    /// Enable/disable.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Bot accounts.
    #[serde(default)]
    pub accounts: HashMap<String, TelegramAccountConfig>,
}

/// Telegram account configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramAccountConfig {
    /// Bot token.
    pub bot_token: SecretString,

    /// Bot username.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Webhook URL (if using webhooks).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub webhook_url: Option<String>,

    /// Enable/disable this account.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Discord channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordConfig {
    /// Enable/disable.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Bot accounts.
    #[serde(default)]
    pub accounts: HashMap<String, DiscordAccountConfig>,
}

/// Discord account configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordAccountConfig {
    /// Bot token.
    pub bot_token: SecretString,

    /// Application ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_id: Option<String>,

    /// Enable/disable this account.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Slack channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    /// Enable/disable.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Bot accounts.
    #[serde(default)]
    pub accounts: HashMap<String, SlackAccountConfig>,
}

/// Slack account configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackAccountConfig {
    /// Bot token.
    pub bot_token: SecretString,

    /// App token (for socket mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_token: Option<SecretString>,

    /// Enable/disable this account.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Signal channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalConfig {
    /// Enable/disable.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Signal CLI REST API URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_url: Option<String>,

    /// Phone number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone_number: Option<String>,
}

/// WhatsApp channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    /// Enable/disable.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Bot accounts.
    #[serde(default)]
    pub accounts: HashMap<String, WhatsAppAccountConfig>,
}

/// WhatsApp account configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppAccountConfig {
    /// Phone number.
    pub phone_number: String,

    /// Enable/disable this account.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// Gateway configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayConfig {
    /// Bind mode.
    #[serde(default)]
    pub bind: BindMode,

    /// Port number.
    #[serde(default = "default_port")]
    pub port: u16,

    /// Control UI settings.
    #[serde(default)]
    pub control_ui: ControlUiConfig,

    /// HTTP endpoint settings.
    #[serde(default)]
    pub http: HttpConfig,

    /// Tailscale settings.
    #[serde(default)]
    pub tailscale: TailscaleConfig,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        Self {
            bind: BindMode::default(),
            port: default_port(),
            control_ui: ControlUiConfig::default(),
            http: HttpConfig::default(),
            tailscale: TailscaleConfig::default(),
        }
    }
}

fn default_port() -> u16 {
    18789
}

/// Bind mode for the gateway.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BindMode {
    /// Bind to loopback only (127.0.0.1).
    #[default]
    Loopback,

    /// Bind to LAN interfaces.
    Lan,

    /// Bind to Tailscale interface.
    Tailnet,

    /// Auto-detect.
    Auto,
}

/// Control UI configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlUiConfig {
    /// Enable control UI.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Authentication settings.
    #[serde(default)]
    pub auth: ControlUiAuth,
}

/// Control UI authentication.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ControlUiAuth {
    /// Auth mode.
    #[serde(default)]
    pub mode: ControlUiAuthMode,

    /// Password (if using password auth).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<SecretString>,

    /// Token (if using token auth).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<SecretString>,
}

/// Control UI auth mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ControlUiAuthMode {
    #[default]
    Identity,
    Password,
    Token,
}

/// HTTP endpoint configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpConfig {
    /// Endpoints configuration.
    #[serde(default)]
    pub endpoints: HttpEndpoints,
}

/// HTTP endpoints.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpEndpoints {
    /// Chat completions endpoint.
    #[serde(default)]
    pub chat_completions: HttpEndpointConfig,
}

/// HTTP endpoint configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HttpEndpointConfig {
    /// Enable this endpoint.
    #[serde(default)]
    pub enabled: bool,
}

/// Tailscale configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TailscaleConfig {
    /// Tailscale mode.
    #[serde(default)]
    pub mode: TailscaleMode,
}

/// Tailscale mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TailscaleMode {
    #[default]
    Off,
    Serve,
    Funnel,
}

/// Session configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionConfig {
    /// Session scope for routing.
    #[serde(default)]
    pub scope: SessionScope,

    /// DM scope.
    #[serde(default)]
    pub dm_scope: DmScope,

    /// Session reset settings.
    #[serde(default)]
    pub reset: SessionResetConfig,
}

/// Session scope.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionScope {
    #[default]
    PerSender,
    Global,
}

/// Session reset configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionResetConfig {
    /// Reset mode.
    #[serde(default)]
    pub mode: SessionResetMode,

    /// Hour of day to reset (for daily mode).
    #[serde(default)]
    pub at_hour: u8,

    /// Idle minutes before reset.
    #[serde(default = "default_idle_minutes")]
    pub idle_minutes: u32,
}

fn default_idle_minutes() -> u32 {
    60
}

/// Session reset mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionResetMode {
    #[default]
    Daily,
    Idle,
    Never,
}

/// Security configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// Audit logging.
    #[serde(default)]
    pub audit: AuditConfig,

    /// Execution security.
    #[serde(default)]
    pub exec: ExecSecurityConfig,

    /// DM policy.
    #[serde(default)]
    pub dm_policy: DmPolicy,

    /// Sandbox settings.
    #[serde(default)]
    pub sandbox: SecuritySandboxConfig,
}

/// Security sandbox configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecuritySandboxConfig {
    /// Default sandbox profile.
    #[serde(default)]
    pub default_profile: SandboxProfile,

    /// Default resource limits.
    #[serde(default)]
    pub default_limits: ResourceLimits,
}

/// Memory configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Memory provider.
    #[serde(default)]
    pub provider: MemoryProvider,

    /// Embeddings provider.
    #[serde(default)]
    pub embeddings: EmbeddingsProvider,

    /// Search settings.
    #[serde(default)]
    pub search: MemorySearchConfig,
}

/// Memory provider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemoryProvider {
    #[default]
    Lancedb,
    VectorOnly,
}

/// Embeddings provider.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EmbeddingsProvider {
    #[default]
    Openai,
    Google,
}

/// Memory search configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySearchConfig {
    /// Result limit.
    pub limit: usize,

    /// Top K for vector search.
    pub top_k: usize,
}

impl Default for MemorySearchConfig {
    fn default() -> Self {
        Self { limit: 5, top_k: 3 }
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level.
    #[serde(default)]
    pub level: LogLevel,

    /// Log file path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,

    /// Diagnostics settings.
    #[serde(default)]
    pub diagnostics: DiagnosticsConfig,
}

/// Log level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

/// Diagnostics configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiagnosticsConfig {
    /// Enable diagnostics.
    #[serde(default)]
    pub enabled: bool,

    /// Diagnostic flags.
    #[serde(default)]
    pub flags: Vec<String>,
}

/// Routing configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingConfig {
    /// Route bindings.
    #[serde(default)]
    pub bindings: Vec<RouteBinding>,
}

/// A route binding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteBinding {
    /// Target agent ID.
    pub agent_id: String,

    /// Match channel.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_channel: Option<String>,

    /// Match account.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_account: Option<String>,

    /// Match peer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_peer: Option<String>,

    /// Match guild/team.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_guild: Option<String>,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(config.agents.default.is_none());
        assert!(config.agents.agents.is_empty());
        assert_eq!(config.gateway.port, 18789);
        assert_eq!(config.gateway.bind, BindMode::Loopback);
    }

    #[test]
    fn test_config_serde_roundtrip() {
        let config = Config::default();
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.gateway.port, config.gateway.port);
        assert_eq!(parsed.gateway.bind, config.gateway.bind);
        assert_eq!(parsed.session.scope, config.session.scope);
    }

    #[test]
    fn test_bind_mode_default_is_loopback() {
        assert_eq!(BindMode::default(), BindMode::Loopback);
    }

    #[test]
    fn test_bind_mode_serde_all_variants() {
        let modes = [BindMode::Loopback, BindMode::Lan, BindMode::Tailnet, BindMode::Auto];
        for mode in &modes {
            let json = serde_json::to_string(mode).unwrap();
            let parsed: BindMode = serde_json::from_str(&json).unwrap();
            assert_eq!(*mode, parsed);
        }
    }

    #[test]
    fn test_session_scope_default_is_per_sender() {
        assert_eq!(SessionScope::default(), SessionScope::PerSender);
    }

    #[test]
    fn test_session_reset_mode_default_is_daily() {
        assert_eq!(SessionResetMode::default(), SessionResetMode::Daily);
    }

    #[test]
    fn test_memory_provider_default_is_lancedb() {
        assert_eq!(MemoryProvider::default(), MemoryProvider::Lancedb);
    }

    #[test]
    fn test_embeddings_provider_default_is_openai() {
        assert_eq!(EmbeddingsProvider::default(), EmbeddingsProvider::Openai);
    }

    #[test]
    fn test_memory_search_config_default() {
        let config = MemorySearchConfig::default();
        assert_eq!(config.limit, 5);
        assert_eq!(config.top_k, 3);
    }

    #[test]
    fn test_log_level_default_is_info() {
        assert_eq!(LogLevel::default(), LogLevel::Info);
    }

    #[test]
    fn test_control_ui_auth_mode_default_is_identity() {
        assert_eq!(ControlUiAuthMode::default(), ControlUiAuthMode::Identity);
    }

    #[test]
    fn test_tailscale_mode_default_is_off() {
        assert_eq!(TailscaleMode::default(), TailscaleMode::Off);
    }

    #[test]
    fn test_cache_type_default_is_prompt() {
        assert_eq!(CacheType::default(), CacheType::Prompt);
    }

    #[test]
    fn test_channels_config_default_all_none() {
        let channels = ChannelsConfig::default();
        assert!(channels.telegram.is_none());
        assert!(channels.discord.is_none());
        assert!(channels.slack.is_none());
        assert!(channels.signal.is_none());
        assert!(channels.whatsapp.is_none());
        assert!(channels.extensions.is_empty());
    }

    #[test]
    fn test_route_binding_serde_roundtrip() {
        let binding = RouteBinding {
            agent_id: "my-agent".to_string(),
            match_channel: Some("telegram".to_string()),
            match_account: None,
            match_peer: Some("user123".to_string()),
            match_guild: None,
        };
        let json = serde_json::to_string(&binding).unwrap();
        let parsed: RouteBinding = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.agent_id, "my-agent");
        assert_eq!(parsed.match_channel.as_deref(), Some("telegram"));
        assert!(parsed.match_account.is_none());
        assert_eq!(parsed.match_peer.as_deref(), Some("user123"));
    }

    #[test]
    fn test_gateway_config_default_port() {
        let config = GatewayConfig::default();
        assert_eq!(config.port, 18789);
    }

    #[test]
    fn test_log_level_serde_all_variants() {
        let levels = [LogLevel::Trace, LogLevel::Debug, LogLevel::Info, LogLevel::Warn, LogLevel::Error];
        for level in &levels {
            let json = serde_json::to_string(level).unwrap();
            let parsed: LogLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(*level, parsed);
        }
    }
}
