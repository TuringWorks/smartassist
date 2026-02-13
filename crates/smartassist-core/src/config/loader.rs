//! Configuration loading and persistence.

use super::Config;
use crate::error::ConfigError;
use crate::paths;
use std::fs;
use std::path::Path;

impl Config {
    /// Load configuration from the default path.
    pub fn load_default() -> Result<Self, ConfigError> {
        let path = paths::config_file()?;
        Self::load(&path)
    }

    /// Load configuration from a file path.
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.to_path_buf()));
        }

        let content = fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Parse configuration from a string.
    pub fn parse(content: &str) -> Result<Self, ConfigError> {
        json5::from_str(content).map_err(|e| ConfigError::Json5(e.to_string()))
    }

    /// Save configuration to the default path.
    pub fn save_default(&self) -> Result<(), ConfigError> {
        let path = paths::config_file()?;
        self.save(&path)
    }

    /// Save configuration to a file path.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let content = self.to_json5()?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Write atomically
        let temp_path = path.with_extension("tmp");
        fs::write(&temp_path, &content)?;
        fs::rename(&temp_path, path)?;

        Ok(())
    }

    /// Serialize to JSON5 string.
    pub fn to_json5(&self) -> Result<String, ConfigError> {
        // json5 doesn't have a serializer, so we use serde_json with pretty print
        serde_json::to_string_pretty(self).map_err(|e| ConfigError::Parse(e.to_string()))
    }

    /// Validate the configuration, collecting all errors before returning.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut errors = Vec::new();

        // 1. Port validation
        if self.gateway.port == 0 {
            errors.push("Gateway port cannot be 0".to_string());
        }

        // 2. Session reset at_hour must be a valid hour (0-23)
        if self.session.reset.at_hour > 23 {
            errors.push(format!(
                "Session reset at_hour must be 0-23, got {}",
                self.session.reset.at_hour
            ));
        }

        // 3. Default model format (provider/model-id)
        if let Some(model) = &self.agents.defaults.model {
            if !model.contains('/') {
                errors.push(format!(
                    "Invalid model format '{}', expected 'provider/model-id'",
                    model
                ));
            }
        }

        // 4. Default agent must exist when agents are defined
        if let Some(default) = &self.agents.default {
            if !self.agents.agents.is_empty() && !self.agents.agents.contains_key(default) {
                errors.push(format!(
                    "Default agent '{}' not found in agents map",
                    default
                ));
            }
        }

        // 5. Per-agent model format validation
        for (id, agent) in &self.agents.agents {
            if let Some(model) = &agent.model {
                if !model.contains('/') {
                    errors.push(format!(
                        "Agent '{}': invalid model format '{}', expected 'provider/model-id'",
                        id, model
                    ));
                }
            }
            // Validate fallback models
            for fb_model in &agent.fallback_models {
                if !fb_model.contains('/') {
                    errors.push(format!(
                        "Agent '{}': invalid fallback model format '{}', expected 'provider/model-id'",
                        id, fb_model
                    ));
                }
            }
        }

        // 6. Channel credential validation -- enabled channels must have accounts/config
        if let Some(telegram) = &self.channels.telegram {
            if telegram.enabled && telegram.accounts.is_empty() {
                errors.push("Telegram is enabled but has no accounts configured".to_string());
            }
        }
        if let Some(discord) = &self.channels.discord {
            if discord.enabled && discord.accounts.is_empty() {
                errors.push("Discord is enabled but has no accounts configured".to_string());
            }
        }
        if let Some(slack) = &self.channels.slack {
            if slack.enabled && slack.accounts.is_empty() {
                errors.push("Slack is enabled but has no accounts configured".to_string());
            }
        }
        if let Some(signal) = &self.channels.signal {
            if signal.enabled && signal.api_url.is_none() {
                errors.push("Signal is enabled but api_url is not set".to_string());
            }
        }
        if let Some(whatsapp) = &self.channels.whatsapp {
            if whatsapp.enabled && whatsapp.accounts.is_empty() {
                errors.push("WhatsApp is enabled but has no accounts configured".to_string());
            }
        }

        // 7. Auth mode consistency -- password/token modes need their respective credential
        use super::ControlUiAuthMode;
        match self.gateway.control_ui.auth.mode {
            ControlUiAuthMode::Password => {
                if self.gateway.control_ui.auth.password.is_none() {
                    errors.push(
                        "Control UI auth mode is 'password' but no password is set".to_string(),
                    );
                }
            }
            ControlUiAuthMode::Token => {
                if self.gateway.control_ui.auth.token.is_none() {
                    errors.push(
                        "Control UI auth mode is 'token' but no token is set".to_string(),
                    );
                }
            }
            _ => {}
        }

        // 8. Route binding validation -- agent_id must not be empty
        for (i, binding) in self.routing.bindings.iter().enumerate() {
            if binding.agent_id.is_empty() {
                errors.push(format!("Route binding [{}]: agent_id must not be empty", i));
            }
        }

        // 9. Memory search limits
        if self.memory.search.limit == 0 {
            errors.push("Memory search limit must be greater than 0".to_string());
        }
        if self.memory.search.limit > 100 {
            errors.push(format!(
                "Memory search limit {} exceeds maximum of 100",
                self.memory.search.limit
            ));
        }
        if self.memory.search.top_k == 0 {
            errors.push("Memory search top_k must be greater than 0".to_string());
        }
        if self.memory.search.top_k > self.memory.search.limit {
            errors.push(format!(
                "Memory search top_k ({}) exceeds limit ({})",
                self.memory.search.top_k, self.memory.search.limit
            ));
        }

        // Return collected errors
        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Validation(errors.join("; ")))
        }
    }

    /// Get agent config by ID, falling back to defaults.
    pub fn get_agent(&self, id: &str) -> Option<&crate::types::AgentConfig> {
        self.agents.agents.get(id)
    }

    /// Load configuration from the default path, falling back to defaults if no file exists.
    ///
    /// When no config file is found, environment variables are inspected to auto-detect
    /// the provider and configure a default agent. This enables zero-config startup when
    /// an API key is set in the environment.
    pub fn load_or_default() -> Self {
        match Self::load_default() {
            Ok(config) => config,
            Err(ConfigError::NotFound(_)) => Self::from_env_defaults(),
            Err(_) => Self::default(),
        }
    }

    /// Create a Config from defaults, enhanced by environment variable detection.
    ///
    /// Probes `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GOOGLE_API_KEY` to auto-select
    /// a provider and model. Creates a "default" agent when a key is detected.
    pub fn from_env_defaults() -> Self {
        use crate::env;

        let mut config = Self::default();

        // Auto-detect provider from env vars, in priority order
        let detected = if env::get_var(env::vars::ANTHROPIC_API_KEY).is_some() {
            Some("anthropic/claude-sonnet-4-5-20250929")
        } else if env::get_var(env::vars::OPENAI_API_KEY).is_some() {
            Some("openai/gpt-4o")
        } else if env::get_var(env::vars::GOOGLE_API_KEY).is_some() {
            Some("google/gemini-2.0-flash")
        } else {
            None
        };

        if let Some(model) = detected {
            config.agents.defaults.model = Some(model.to_string());

            // Create a default agent so the gateway has something to route to
            let mut agent = crate::types::AgentConfig::default();
            agent.id = crate::types::AgentId::new("default");
            agent.name = Some("Default Agent".to_string());
            agent.model = Some(model.to_string());
            config.agents.agents.insert("default".to_string(), agent);
            config.agents.default = Some("default".to_string());
        }

        // Honor env override for port
        if let Some(port) = env::get_u16("SMARTASSIST_PORT") {
            config.gateway.port = port;
        }

        config
    }

    /// Get the default agent ID.
    pub fn default_agent_id(&self) -> Option<&str> {
        self.agents.default.as_deref().or_else(|| {
            // If only one agent, use it as default
            if self.agents.agents.len() == 1 {
                self.agents.agents.keys().next().map(|s| s.as_str())
            } else {
                None
            }
        })
    }
}

/// Configuration builder for creating configs programmatically.
#[derive(Debug, Default)]
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    /// Create a new config builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the default agent.
    pub fn default_agent(mut self, id: impl Into<String>) -> Self {
        self.config.agents.default = Some(id.into());
        self
    }

    /// Set the default model.
    pub fn default_model(mut self, model: impl Into<String>) -> Self {
        self.config.agents.defaults.model = Some(model.into());
        self
    }

    /// Set the gateway port.
    pub fn port(mut self, port: u16) -> Self {
        self.config.gateway.port = port;
        self
    }

    /// Set the bind mode.
    pub fn bind(mut self, mode: super::BindMode) -> Self {
        self.config.gateway.bind = mode;
        self
    }

    /// Add an agent configuration. The first agent added becomes the default.
    pub fn add_agent(mut self, id: impl Into<String>, agent: crate::types::AgentConfig) -> Self {
        let id = id.into();
        if self.config.agents.default.is_none() {
            self.config.agents.default = Some(id.clone());
        }
        self.config.agents.agents.insert(id, agent);
        self
    }

    /// Set the default thinking level for all agents.
    pub fn thinking_level(mut self, level: crate::types::ThinkingLevel) -> Self {
        self.config.agents.defaults.thinking_level = level;
        self
    }

    /// Set the log level.
    pub fn log_level(mut self, level: super::LogLevel) -> Self {
        self.config.logging.level = level;
        self
    }

    /// Set the session scope.
    pub fn session_scope(mut self, scope: super::SessionScope) -> Self {
        self.config.session.scope = scope;
        self
    }

    /// Set the session reset mode.
    pub fn session_reset_mode(mut self, mode: super::SessionResetMode) -> Self {
        self.config.session.reset.mode = mode;
        self
    }

    /// Set the default sandbox profile.
    pub fn sandbox_profile(mut self, profile: crate::types::SandboxProfile) -> Self {
        self.config.security.sandbox.default_profile = profile;
        self
    }

    /// Set the memory provider.
    pub fn memory_provider(mut self, provider: super::MemoryProvider) -> Self {
        self.config.memory.provider = provider;
        self
    }

    /// Set the control UI auth mode.
    pub fn control_ui_auth_mode(mut self, mode: super::ControlUiAuthMode) -> Self {
        self.config.gateway.control_ui.auth.mode = mode;
        self
    }

    /// Add a Telegram account, enabling the channel.
    pub fn telegram_account(
        mut self,
        account_id: impl Into<String>,
        account: super::TelegramAccountConfig,
    ) -> Self {
        let telegram = self
            .config
            .channels
            .telegram
            .get_or_insert(super::TelegramConfig {
                enabled: true,
                accounts: Default::default(),
            });
        telegram.accounts.insert(account_id.into(), account);
        self
    }

    /// Add a Discord account, enabling the channel.
    pub fn discord_account(
        mut self,
        account_id: impl Into<String>,
        account: super::DiscordAccountConfig,
    ) -> Self {
        let discord = self
            .config
            .channels
            .discord
            .get_or_insert(super::DiscordConfig {
                enabled: true,
                accounts: Default::default(),
            });
        discord.accounts.insert(account_id.into(), account);
        self
    }

    /// Add a Slack account, enabling the channel.
    pub fn slack_account(
        mut self,
        account_id: impl Into<String>,
        account: super::SlackAccountConfig,
    ) -> Self {
        let slack = self
            .config
            .channels
            .slack
            .get_or_insert(super::SlackConfig {
                enabled: true,
                accounts: Default::default(),
            });
        slack.accounts.insert(account_id.into(), account);
        self
    }

    /// Add a route binding.
    pub fn route(mut self, binding: super::RouteBinding) -> Self {
        self.config.routing.bindings.push(binding);
        self
    }

    /// Build the config.
    pub fn build(self) -> Config {
        self.config
    }

    /// Validate and build the config, returning an error if validation fails.
    pub fn build_validated(self) -> Result<Config, ConfigError> {
        self.config.validate()?;
        Ok(self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let content = r#"{
            "agents": {
                "default": "test"
            }
        }"#;

        let config = Config::parse(content).unwrap();
        assert_eq!(config.agents.default, Some("test".to_string()));
    }

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .default_agent("bot")
            .default_model("anthropic/claude-3-opus")
            .port(8080)
            .build();

        assert_eq!(config.agents.default, Some("bot".to_string()));
        assert_eq!(
            config.agents.defaults.model,
            Some("anthropic/claude-3-opus".to_string())
        );
        assert_eq!(config.gateway.port, 8080);
    }

    #[test]
    fn test_validate_invalid_model() {
        let mut config = Config::default();
        config.agents.defaults.model = Some("invalid".to_string());

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_port_zero() {
        let mut config = Config::default();
        config.gateway.port = 0;
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("port"), "Error should mention port: {}", err_msg);
    }

    #[test]
    fn test_validate_at_hour_24() {
        let mut config = Config::default();
        config.session.reset.at_hour = 24;
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("at_hour"), "Error should mention at_hour: {}", err_msg);
    }

    #[test]
    fn test_validate_at_hour_23() {
        let mut config = Config::default();
        config.session.reset.at_hour = 23;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_valid_model() {
        let mut config = Config::default();
        config.agents.defaults.model = Some("anthropic/claude-3-opus".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_invalid_model_no_slash() {
        let mut config = Config::default();
        config.agents.defaults.model = Some("claude-3-opus".to_string());
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("provider/model-id"),
            "Error should mention expected format: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_agent_model_invalid() {
        let mut config = Config::default();
        let mut agent = crate::types::AgentConfig::default();
        agent.model = Some("no-slash".to_string());
        config.agents.agents.insert("test-agent".to_string(), agent);
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("test-agent"), "Error should mention agent: {}", err_msg);
    }

    #[test]
    fn test_validate_fallback_model_invalid() {
        let mut config = Config::default();
        let mut agent = crate::types::AgentConfig::default();
        agent.fallback_models = vec!["bad-model".to_string()];
        config.agents.agents.insert("fb-agent".to_string(), agent);
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("fallback"),
            "Error should mention fallback: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_default_agent_not_in_map() {
        let mut config = Config::default();
        config.agents.default = Some("missing-agent".to_string());
        let mut agent = crate::types::AgentConfig::default();
        agent.id = crate::types::AgentId::new("other");
        config.agents.agents.insert("other".to_string(), agent);
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("missing-agent"),
            "Error should mention the missing agent name: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_default_agent_implicit() {
        // When no agents are defined, setting a default should pass validation
        // (the agents map is empty, so the check is skipped).
        let mut config = Config::default();
        config.agents.default = Some("main".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_telegram_enabled_no_accounts() {
        let mut config = Config::default();
        config.channels.telegram = Some(super::super::TelegramConfig {
            enabled: true,
            accounts: std::collections::HashMap::new(),
        });
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Telegram"),
            "Error should mention Telegram: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_discord_enabled_no_accounts() {
        let mut config = Config::default();
        config.channels.discord = Some(super::super::DiscordConfig {
            enabled: true,
            accounts: std::collections::HashMap::new(),
        });
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Discord"),
            "Error should mention Discord: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_signal_enabled_no_api_url() {
        let mut config = Config::default();
        config.channels.signal = Some(super::super::SignalConfig {
            enabled: true,
            api_url: None,
            phone_number: None,
        });
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Signal"),
            "Error should mention Signal: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_auth_password_mode_no_password() {
        let mut config = Config::default();
        config.gateway.control_ui.auth.mode = super::super::ControlUiAuthMode::Password;
        config.gateway.control_ui.auth.password = None;
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("password"),
            "Error should mention password: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_auth_token_mode_no_token() {
        let mut config = Config::default();
        config.gateway.control_ui.auth.mode = super::super::ControlUiAuthMode::Token;
        config.gateway.control_ui.auth.token = None;
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("token"),
            "Error should mention token: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_route_binding_empty_agent_id() {
        let mut config = Config::default();
        config.routing.bindings.push(super::super::RouteBinding {
            agent_id: String::new(),
            match_channel: None,
            match_account: None,
            match_peer: None,
            match_guild: None,
        });
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("agent_id"),
            "Error should mention agent_id: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_memory_search_limit_zero() {
        let mut config = Config::default();
        config.memory.search.limit = 0;
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("limit"),
            "Error should mention limit: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_memory_top_k_exceeds_limit() {
        let mut config = Config::default();
        config.memory.search.limit = 3;
        config.memory.search.top_k = 5;
        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("top_k"),
            "Error should mention top_k: {}",
            err_msg
        );
    }

    #[test]
    fn test_validate_collects_all_errors() {
        let mut config = Config::default();
        // Inject multiple validation failures.
        config.gateway.port = 0;
        config.session.reset.at_hour = 25;
        config.agents.defaults.model = Some("bad-model".to_string());

        let result = config.validate();
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // All three errors should be collected in the message.
        assert!(err_msg.contains("port"), "Should contain port error: {}", err_msg);
        assert!(err_msg.contains("at_hour"), "Should contain at_hour error: {}", err_msg);
        assert!(err_msg.contains("provider/model-id"), "Should contain model error: {}", err_msg);
    }

    #[test]
    fn test_default_agent_id_single_agent() {
        let mut config = Config::default();
        let agent = crate::types::AgentConfig::default();
        config.agents.agents.insert("only-one".to_string(), agent);
        // No explicit default set, but only one agent exists.
        assert_eq!(config.default_agent_id(), Some("only-one"));
    }

    #[test]
    fn test_default_agent_id_no_agents() {
        let config = Config::default();
        assert!(config.default_agent_id().is_none());
    }

    #[test]
    fn test_load_or_default_returns_valid_config() {
        // With no config file present, should still produce a valid config
        let config = Config::load_or_default();
        assert!(config.validate().is_ok());
        assert_eq!(config.gateway.port, 18789);
    }

    #[test]
    fn test_from_env_defaults_no_keys() {
        // When no API keys are detected, should return bare defaults with no agents
        // Note: this test may detect keys if they're in the environment;
        // the important thing is it doesn't panic and validates.
        let config = Config::from_env_defaults();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_builder_add_agent() {
        let mut agent = crate::types::AgentConfig::default();
        agent.model = Some("anthropic/claude-sonnet-4-5-20250929".to_string());

        let config = ConfigBuilder::new()
            .add_agent("my-bot", agent)
            .build();

        assert_eq!(config.agents.default, Some("my-bot".to_string()));
        assert!(config.agents.agents.contains_key("my-bot"));
    }

    #[test]
    fn test_config_builder_add_agent_first_becomes_default() {
        let agent1 = crate::types::AgentConfig::default();
        let agent2 = crate::types::AgentConfig::default();

        let config = ConfigBuilder::new()
            .add_agent("first", agent1)
            .add_agent("second", agent2)
            .build();

        assert_eq!(config.agents.default, Some("first".to_string()));
    }

    #[test]
    fn test_config_builder_build_validated_catches_errors() {
        let result = ConfigBuilder::new().port(0).build_validated();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_builder_build_validated_success() {
        let result = ConfigBuilder::new()
            .default_model("anthropic/claude-sonnet-4-5-20250929")
            .build_validated();
        assert!(result.is_ok());
    }

    #[test]
    fn test_config_builder_all_setters() {
        let config = ConfigBuilder::new()
            .port(9090)
            .bind(super::super::BindMode::Lan)
            .log_level(super::super::LogLevel::Debug)
            .session_scope(super::super::SessionScope::Global)
            .session_reset_mode(super::super::SessionResetMode::Idle)
            .sandbox_profile(crate::types::SandboxProfile::Strict)
            .memory_provider(super::super::MemoryProvider::VectorOnly)
            .thinking_level(crate::types::ThinkingLevel::Medium)
            .control_ui_auth_mode(super::super::ControlUiAuthMode::Identity)
            .build();

        assert_eq!(config.gateway.port, 9090);
        assert_eq!(config.gateway.bind, super::super::BindMode::Lan);
        assert_eq!(config.logging.level, super::super::LogLevel::Debug);
        assert_eq!(config.session.scope, super::super::SessionScope::Global);
        assert_eq!(
            config.session.reset.mode,
            super::super::SessionResetMode::Idle
        );
        assert_eq!(
            config.security.sandbox.default_profile,
            crate::types::SandboxProfile::Strict
        );
        assert_eq!(
            config.memory.provider,
            super::super::MemoryProvider::VectorOnly
        );
        assert_eq!(
            config.agents.defaults.thinking_level,
            crate::types::ThinkingLevel::Medium
        );
    }

    #[test]
    fn test_config_builder_route() {
        let config = ConfigBuilder::new()
            .route(super::super::RouteBinding {
                agent_id: "main".to_string(),
                match_channel: Some("telegram".to_string()),
                match_account: None,
                match_peer: None,
                match_guild: None,
            })
            .build();

        assert_eq!(config.routing.bindings.len(), 1);
        assert_eq!(config.routing.bindings[0].agent_id, "main");
    }
}
