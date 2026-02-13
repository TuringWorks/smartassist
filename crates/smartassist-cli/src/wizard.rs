//! Enhanced configuration wizard.
//!
//! Multi-step interactive wizard for configuring SmartAssist. Steps:
//! 1. Provider selection
//! 2. API key setup
//! 3. Model selection
//! 4. Agent configuration (name, thinking level)
//! 5. Channel setup (optional, skipped with `--quick`)
//! 6. Gateway & security settings (optional, skipped with `--quick`)
//! 7. Review, validate, and write config; optional connection test

use console::style;
use smartassist_core::config::{self, ConfigBuilder};
use smartassist_core::paths;
use smartassist_core::types::{AgentConfig, AgentId, ThinkingLevel};
use smartassist_secrets::{FileSecretStore, SecretStore};
use std::io::{self, Write};

// ---------------------------------------------------------------------------
// Provider enum
// ---------------------------------------------------------------------------

/// Supported AI providers.
#[derive(Debug, Clone, Copy)]
enum Provider {
    Anthropic,
    OpenAI,
    Google,
    Ollama,
}

impl Provider {
    fn all() -> &'static [Provider] {
        &[
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Google,
            Provider::Ollama,
        ]
    }

    fn name(&self) -> &str {
        match self {
            Self::Anthropic => "Anthropic (Claude)",
            Self::OpenAI => "OpenAI (GPT)",
            Self::Google => "Google (Gemini)",
            Self::Ollama => "Ollama (Local)",
        }
    }

    fn config_key(&self) -> &str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAI => "openai",
            Self::Google => "google",
            Self::Ollama => "ollama",
        }
    }

    fn api_key_prefix(&self) -> Option<&str> {
        match self {
            Self::Anthropic => Some("sk-ant-"),
            Self::OpenAI => Some("sk-"),
            Self::Google => None,
            Self::Ollama => None,
        }
    }

    fn env_var(&self) -> Option<&str> {
        match self {
            Self::Anthropic => Some("ANTHROPIC_API_KEY"),
            Self::OpenAI => Some("OPENAI_API_KEY"),
            Self::Google => Some("GOOGLE_API_KEY"),
            Self::Ollama => None,
        }
    }

    /// Return (model_id, description) tuples for this provider.
    fn models(&self) -> Vec<(&str, &str)> {
        match self {
            Provider::Anthropic => vec![
                ("claude-sonnet-4-5-20250929", "Claude Sonnet 4.5 (balanced)"),
                ("claude-opus-4-6", "Claude Opus 4.6 (most capable)"),
                (
                    "claude-haiku-4-5-20251001",
                    "Claude Haiku 4.5 (fastest)",
                ),
            ],
            Provider::OpenAI => vec![
                ("gpt-4o", "GPT-4o (balanced)"),
                ("gpt-4o-mini", "GPT-4o Mini (fastest)"),
                ("gpt-4-turbo", "GPT-4 Turbo"),
            ],
            Provider::Google => vec![
                ("gemini-2.0-flash", "Gemini 2.0 Flash (fast)"),
                ("gemini-2.0-pro", "Gemini 2.0 Pro"),
            ],
            Provider::Ollama => vec![
                ("llama3.2", "Llama 3.2"),
                ("mistral", "Mistral"),
                ("qwen2.5", "Qwen 2.5"),
            ],
        }
    }

    /// Return model in "provider/model-id" format suitable for Config.
    fn model_in_config_format(&self, model: &str) -> String {
        format!("{}/{}", self.config_key(), model)
    }
}

// ---------------------------------------------------------------------------
// Prompt utilities
// ---------------------------------------------------------------------------

/// Prompt for a line of text input (printed to stderr so stdout stays clean).
fn prompt_input(prompt: &str) -> anyhow::Result<String> {
    eprint!("  {}", prompt);
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

/// Prompt for a yes/no question with a default.
fn prompt_yes_no(prompt: &str, default_yes: bool) -> anyhow::Result<bool> {
    let suffix = if default_yes { "[Y/n]" } else { "[y/N]" };
    let answer = prompt_input(&format!("{} {}: ", prompt, suffix))?;
    if answer.is_empty() {
        Ok(default_yes)
    } else {
        Ok(answer.to_lowercase().starts_with('y'))
    }
}

/// Prompt user to select from a numbered list. Returns the zero-based index.
fn prompt_select(items: &[(&str, &str)], default_idx: usize) -> anyhow::Result<usize> {
    for (i, (id, desc)) in items.iter().enumerate() {
        let marker = if i == default_idx {
            " (default)"
        } else {
            ""
        };
        eprintln!(
            "  {} {} - {}{}",
            style(format!("[{}]", i + 1)).cyan(),
            id,
            desc,
            style(marker).dim(),
        );
    }
    eprintln!();

    let choice = prompt_input(&format!("Select [{}]: ", default_idx + 1))?;
    let idx = if choice.is_empty() {
        default_idx
    } else {
        choice
            .parse::<usize>()
            .unwrap_or(default_idx + 1)
            .saturating_sub(1)
    };

    Ok(idx.min(items.len().saturating_sub(1)))
}

/// Prompt for a password/secret (masked input).
fn prompt_secret(prompt: &str) -> anyhow::Result<String> {
    rpassword::prompt_password(format!("  {}", prompt))
        .map_err(|e| anyhow::anyhow!("Failed to read secret: {}", e))
}

// ---------------------------------------------------------------------------
// SetupWizard
// ---------------------------------------------------------------------------

/// Enhanced onboarding wizard.
pub struct SetupWizard {
    force: bool,
    quick: bool,
}

impl SetupWizard {
    /// Create a new wizard.
    ///
    /// * `force` – overwrite existing configuration if present.
    /// * `quick` – skip optional steps (channels, gateway settings).
    pub fn new(force: bool, quick: bool) -> Self {
        Self { force, quick }
    }

    /// Run the full wizard.
    pub async fn run(&self) -> anyhow::Result<()> {
        // Guard: check for existing config
        if !self.force {
            if let Ok(config_path) = paths::config_file() {
                if config_path.exists() {
                    eprintln!(
                        "{} Configuration already exists at {}",
                        style("!").yellow().bold(),
                        style(config_path.display()).dim(),
                    );
                    eprintln!("  Use {} to overwrite.", style("--force").bold());
                    return Ok(());
                }
            }
        }

        self.print_welcome();

        // Step 1: Provider selection
        let provider = self.step_provider()?;

        // Step 2: API key setup
        self.step_api_key(provider).await?;

        // Step 3: Model selection
        let model = self.step_model(provider)?;
        let config_model = provider.model_in_config_format(&model);

        // Step 4: Agent configuration
        let (agent_name, thinking_level) = self.step_agent_config()?;

        // Step 5: Channel setup (optional)
        let channels_configured = if !self.quick {
            self.step_channels().await?
        } else {
            vec![]
        };

        // Step 6: Gateway settings (optional)
        let (port, bind_mode) = if !self.quick {
            self.step_gateway()?
        } else {
            (18789u16, config::BindMode::Loopback)
        };

        // Build config
        let mut agent = AgentConfig::default();
        agent.id = AgentId::new(&agent_name);
        agent.name = Some(agent_name.clone());
        agent.model = Some(config_model.clone());
        agent.thinking_level = thinking_level;

        let config = ConfigBuilder::new()
            .default_agent(&agent_name)
            .default_model(&config_model)
            .port(port)
            .bind(bind_mode)
            .add_agent(&agent_name, agent)
            .build();

        // Validate before writing
        if let Err(e) = config.validate() {
            eprintln!(
                "  {} Generated configuration has validation issues: {}",
                style("!").yellow(),
                e,
            );
            eprintln!("  Writing anyway -- you can fix these with 'smartassist config set'.");
        }

        // Step 7: Review & write
        self.step_write_config(&config)?;

        // Optional connection test
        if !self.quick {
            self.step_test_connection(provider).await?;
        }

        self.print_done(&channels_configured);

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Individual steps
    // -----------------------------------------------------------------------

    fn print_welcome(&self) {
        eprintln!();
        eprintln!(
            "  {}",
            style("Welcome to SmartAssist!").bold().cyan()
        );
        eprintln!(
            "  {}",
            style("Let's set up your configuration.").dim()
        );
        eprintln!();
    }

    /// Step 1: Select a provider.
    fn step_provider(&self) -> anyhow::Result<Provider> {
        eprintln!(
            "{}",
            style("Step 1: Choose your AI provider").bold()
        );
        eprintln!();

        let providers = Provider::all();
        let items: Vec<(&str, &str)> = providers
            .iter()
            .map(|p| (p.config_key(), p.name()))
            .collect();

        let idx = prompt_select(&items, 0)?;
        let provider = providers[idx];

        eprintln!(
            "  {} Selected: {}",
            style("*").green(),
            style(provider.name()).bold(),
        );
        eprintln!();
        Ok(provider)
    }

    /// Step 2: Set up the API key.
    async fn step_api_key(&self, provider: Provider) -> anyhow::Result<()> {
        eprintln!("{}", style("Step 2: API key setup").bold());
        eprintln!();

        if provider.env_var().is_none() {
            // Ollama: prompt for base URL instead
            let url = prompt_input("Ollama base URL [http://localhost:11434]: ")?;
            let url = if url.is_empty() {
                "http://localhost:11434".to_string()
            } else {
                url
            };
            eprintln!(
                "  {} Ollama URL: {}",
                style("*").green(),
                style(&url).dim(),
            );
            eprintln!();
            return Ok(());
        }

        // Check if already set in environment
        if let Some(env_var) = provider.env_var() {
            if std::env::var(env_var).is_ok() {
                eprintln!(
                    "  {} {} is already set in your environment.",
                    style("*").green(),
                    style(env_var).bold(),
                );
                let use_env = prompt_yes_no("Use environment variable?", true)?;
                if use_env {
                    eprintln!();
                    return Ok(());
                }
            }
        }

        // Prompt for API key
        let prompt_msg = format!("Enter your {} API key: ", provider.config_key());
        let api_key = prompt_secret(&prompt_msg)?;

        if api_key.is_empty() {
            anyhow::bail!("API key must not be empty");
        }

        // Validate prefix
        if let Some(prefix) = provider.api_key_prefix() {
            if !api_key.starts_with(prefix) {
                eprintln!(
                    "  {} Key doesn't start with '{}'. It may be invalid.",
                    style("!").yellow(),
                    prefix,
                );
            }
        }

        // Store via smartassist-secrets
        let store = FileSecretStore::from_default_dir()
            .map_err(|e| anyhow::anyhow!("Failed to initialize secret store: {}", e))?;

        let secret_name = format!("{}_api_key", provider.config_key());
        store
            .set(&secret_name, &api_key)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to store API key: {}", e))?;

        eprintln!(
            "  {} API key stored securely as '{}'.",
            style("*").green(),
            style(&secret_name).dim(),
        );
        eprintln!();

        Ok(())
    }

    /// Step 3: Select a model.
    fn step_model(&self, provider: Provider) -> anyhow::Result<String> {
        eprintln!(
            "{}",
            style("Step 3: Choose your default model").bold()
        );
        eprintln!();

        let models = provider.models();
        let idx = prompt_select(&models, 0)?;
        let (model_id, _) = models.get(idx).unwrap_or(&models[0]);

        eprintln!(
            "  {} Selected: {}",
            style("*").green(),
            style(model_id).bold(),
        );
        eprintln!();

        Ok(model_id.to_string())
    }

    /// Step 4: Configure the default agent.
    fn step_agent_config(&self) -> anyhow::Result<(String, ThinkingLevel)> {
        eprintln!(
            "{}",
            style("Step 4: Configure your default agent").bold()
        );
        eprintln!();

        let name = prompt_input("Agent name [default]: ")?;
        let name = if name.is_empty() {
            "default".to_string()
        } else {
            name
        };

        eprintln!();
        eprintln!("  Thinking level controls how much reasoning the model does:");
        let thinking_items = [
            ("off", "No extended thinking"),
            ("low", "Light reasoning (default, 4K tokens)"),
            ("medium", "Moderate reasoning (8K tokens)"),
            ("high", "Deep reasoning (16K tokens)"),
        ];

        let idx = prompt_select(&thinking_items, 1)?;
        let thinking_level = match idx {
            0 => ThinkingLevel::Off,
            1 => ThinkingLevel::Low,
            2 => ThinkingLevel::Medium,
            3 => ThinkingLevel::High,
            _ => ThinkingLevel::Low,
        };

        eprintln!(
            "  {} Agent '{}' with thinking: {:?}",
            style("*").green(),
            style(&name).bold(),
            thinking_level,
        );
        eprintln!();

        Ok((name, thinking_level))
    }

    /// Step 5: Optionally configure messaging channels.
    async fn step_channels(&self) -> anyhow::Result<Vec<String>> {
        eprintln!(
            "{}",
            style("Step 5: Channel setup (optional)").bold()
        );
        eprintln!();

        let want_channels =
            prompt_yes_no("Would you like to configure a messaging channel?", false)?;
        if !want_channels {
            eprintln!("  {} Skipping channel setup.", style("*").green());
            eprintln!();
            return Ok(vec![]);
        }

        let all_channels: Vec<(&str, &str)> = vec![
            ("telegram", "Telegram Bot"),
            ("discord", "Discord Bot"),
            ("slack", "Slack Bot"),
            ("signal", "Signal Messenger"),
            ("whatsapp", "WhatsApp Business"),
        ];

        let mut configured = vec![];

        loop {
            let available: Vec<(&str, &str)> = all_channels
                .iter()
                .filter(|(name, _)| !configured.contains(&name.to_string()))
                .copied()
                .collect();

            if available.is_empty() {
                break;
            }

            eprintln!();
            eprintln!("  Available channels:");
            let idx = prompt_select(&available, 0)?;

            if let Some((channel_name, _)) = available.get(idx) {
                match *channel_name {
                    "telegram" => {
                        eprintln!("  Telegram requires a bot token from @BotFather.");
                        let token = prompt_secret("Bot token: ")?;
                        if !token.is_empty() {
                            let store = FileSecretStore::from_default_dir()
                                .map_err(|e| anyhow::anyhow!("Secret store error: {}", e))?;
                            store
                                .set("telegram_bot_token", &token)
                                .await
                                .map_err(|e| anyhow::anyhow!("Failed to store token: {}", e))?;
                            configured.push("telegram".to_string());
                            eprintln!(
                                "  {} Telegram token stored.",
                                style("*").green()
                            );
                        }
                    }
                    "discord" => {
                        eprintln!(
                            "  Discord requires a bot token from the Developer Portal."
                        );
                        let token = prompt_secret("Bot token: ")?;
                        if !token.is_empty() {
                            let store = FileSecretStore::from_default_dir()
                                .map_err(|e| anyhow::anyhow!("Secret store error: {}", e))?;
                            store
                                .set("discord_bot_token", &token)
                                .await
                                .map_err(|e| anyhow::anyhow!("Failed to store token: {}", e))?;
                            configured.push("discord".to_string());
                            eprintln!(
                                "  {} Discord token stored.",
                                style("*").green()
                            );
                        }
                    }
                    "slack" => {
                        eprintln!(
                            "  Slack requires a bot token from your Slack App settings."
                        );
                        let token = prompt_secret("Bot token (xoxb-...): ")?;
                        if !token.is_empty() {
                            let store = FileSecretStore::from_default_dir()
                                .map_err(|e| anyhow::anyhow!("Secret store error: {}", e))?;
                            store
                                .set("slack_bot_token", &token)
                                .await
                                .map_err(|e| anyhow::anyhow!("Failed to store token: {}", e))?;
                            configured.push("slack".to_string());
                            eprintln!(
                                "  {} Slack token stored.",
                                style("*").green()
                            );
                        }
                    }
                    "signal" => {
                        eprintln!("  Signal requires a Signal CLI REST API URL.");
                        let url =
                            prompt_input("API URL [http://localhost:8080]: ")?;
                        let _url = if url.is_empty() {
                            "http://localhost:8080".to_string()
                        } else {
                            url
                        };
                        configured.push("signal".to_string());
                        eprintln!(
                            "  {} Signal configured.",
                            style("*").green()
                        );
                    }
                    "whatsapp" => {
                        eprintln!(
                            "  WhatsApp requires WhatsApp Business API access."
                        );
                        let phone = prompt_input("Phone number: ")?;
                        if !phone.is_empty() {
                            configured.push("whatsapp".to_string());
                            eprintln!(
                                "  {} WhatsApp configured.",
                                style("*").green()
                            );
                        }
                    }
                    _ => {}
                }
            }

            if configured.len() >= all_channels.len() {
                break;
            }

            let add_more = prompt_yes_no("Add another channel?", false)?;
            if !add_more {
                break;
            }
        }

        eprintln!();
        Ok(configured)
    }

    /// Step 6: Gateway and security settings.
    fn step_gateway(&self) -> anyhow::Result<(u16, config::BindMode)> {
        eprintln!("{}", style("Step 6: Gateway settings").bold());
        eprintln!();

        let port_str = prompt_input("Gateway port [18789]: ")?;
        let port = if port_str.is_empty() {
            18789
        } else {
            port_str.parse::<u16>().unwrap_or(18789)
        };

        eprintln!();
        eprintln!("  Bind mode controls network accessibility:");
        let bind_items = [
            ("loopback", "Localhost only (safest, default)"),
            ("lan", "LAN interfaces (local network access)"),
            ("tailnet", "Tailscale network only"),
            ("auto", "Auto-detect"),
        ];

        let idx = prompt_select(&bind_items, 0)?;
        let bind_mode = match idx {
            0 => config::BindMode::Loopback,
            1 => config::BindMode::Lan,
            2 => config::BindMode::Tailnet,
            3 => config::BindMode::Auto,
            _ => config::BindMode::Loopback,
        };

        eprintln!(
            "  {} Port: {}, Bind: {:?}",
            style("*").green(),
            port,
            bind_mode,
        );
        eprintln!();

        Ok((port, bind_mode))
    }

    /// Step 7: Review and write configuration.
    fn step_write_config(&self, config: &config::Config) -> anyhow::Result<()> {
        eprintln!(
            "{}",
            style("Writing configuration").bold()
        );
        eprintln!();

        // Summary
        eprintln!("  Configuration summary:");
        if let Some(ref default_agent) = config.agents.default {
            eprintln!(
                "    Default agent: {}",
                style(default_agent).cyan()
            );
        }
        if let Some(ref model) = config.agents.defaults.model {
            eprintln!("    Default model:  {}", style(model).cyan());
        }
        eprintln!(
            "    Gateway port:   {}",
            style(config.gateway.port).cyan()
        );
        eprintln!("    Bind mode:      {:?}", config.gateway.bind);
        eprintln!(
            "    Sandbox:        {:?}",
            config.security.sandbox.default_profile
        );
        eprintln!();

        paths::ensure_dirs()
            .map_err(|e| anyhow::anyhow!("Failed to create directories: {}", e))?;

        config
            .save_default()
            .map_err(|e| anyhow::anyhow!("Failed to write config: {}", e))?;

        let config_path = paths::config_file()
            .map_err(|e| anyhow::anyhow!("Failed to get config path: {}", e))?;

        eprintln!(
            "  {} Configuration written to {}",
            style("*").green(),
            style(config_path.display()).dim(),
        );
        eprintln!();

        Ok(())
    }

    /// Optional: Test the provider connection.
    async fn step_test_connection(&self, provider: Provider) -> anyhow::Result<()> {
        let want_test = prompt_yes_no("Test connection to your provider?", true)?;
        if !want_test {
            return Ok(());
        }

        eprint!("  Testing connection... ");
        io::stderr().flush()?;

        match provider {
            Provider::Anthropic => {
                match smartassist_providers::anthropic::AnthropicProvider::from_env() {
                    Ok(_p) => {
                        eprintln!(
                            "{}",
                            style("Anthropic API key detected.").green()
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "{} {}",
                            style("Not configured:").yellow(),
                            e
                        );
                    }
                }
            }
            Provider::OpenAI => {
                match smartassist_providers::openai::OpenAIProvider::from_env() {
                    Ok(_p) => {
                        eprintln!(
                            "{}",
                            style("OpenAI API key detected.").green()
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "{} {}",
                            style("Not configured:").yellow(),
                            e
                        );
                    }
                }
            }
            Provider::Google => {
                match smartassist_providers::google::GoogleProvider::from_env() {
                    Ok(_p) => {
                        eprintln!(
                            "{}",
                            style("Google API key detected.").green()
                        );
                    }
                    Err(e) => {
                        eprintln!(
                            "{} {}",
                            style("Not configured:").yellow(),
                            e
                        );
                    }
                }
            }
            Provider::Ollama => {
                match reqwest::get("http://localhost:11434/api/tags").await {
                    Ok(resp) if resp.status().is_success() => {
                        eprintln!(
                            "{}",
                            style("Ollama is running locally!").green()
                        );
                    }
                    _ => {
                        eprintln!(
                            "{}",
                            style("Could not reach Ollama at localhost:11434.").yellow()
                        );
                    }
                }
            }
        }

        eprintln!();
        Ok(())
    }

    fn print_done(&self, channels_configured: &[String]) {
        eprintln!(
            "  {} {}",
            style("Setup complete!").green().bold(),
            style("You're ready to go.").dim(),
        );
        eprintln!();

        if !channels_configured.is_empty() {
            eprintln!(
                "  Channels configured: {}",
                channels_configured.join(", ")
            );
        }

        eprintln!("  Next steps:");
        eprintln!(
            "    {} Start chatting",
            style("smartassist agent chat").cyan(),
        );
        eprintln!(
            "    {} Start the gateway",
            style("smartassist gateway run").cyan(),
        );
        eprintln!(
            "    {} Run diagnostics",
            style("smartassist doctor").cyan(),
        );
        eprintln!();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use smartassist_core::config::Config;

    #[test]
    fn test_provider_model_config_format() {
        let p = Provider::Anthropic;
        assert_eq!(
            p.model_in_config_format("claude-sonnet-4-5-20250929"),
            "anthropic/claude-sonnet-4-5-20250929"
        );
    }

    #[test]
    fn test_provider_model_config_format_openai() {
        let p = Provider::OpenAI;
        assert_eq!(p.model_in_config_format("gpt-4o"), "openai/gpt-4o");
    }

    #[test]
    fn test_provider_models_non_empty() {
        for provider in Provider::all() {
            assert!(
                !provider.models().is_empty(),
                "{} should have at least one model",
                provider.name()
            );
        }
    }

    #[test]
    fn test_provider_env_vars() {
        assert_eq!(Provider::Anthropic.env_var(), Some("ANTHROPIC_API_KEY"));
        assert_eq!(Provider::OpenAI.env_var(), Some("OPENAI_API_KEY"));
        assert_eq!(Provider::Google.env_var(), Some("GOOGLE_API_KEY"));
        assert_eq!(Provider::Ollama.env_var(), None);
    }

    #[test]
    fn test_wizard_builds_valid_config() {
        let mut agent = AgentConfig::default();
        agent.id = AgentId::new("default");
        agent.name = Some("Default Agent".to_string());
        agent.model = Some("anthropic/claude-sonnet-4-5-20250929".to_string());
        agent.thinking_level = ThinkingLevel::Low;

        let config = ConfigBuilder::new()
            .default_agent("default")
            .default_model("anthropic/claude-sonnet-4-5-20250929")
            .port(18789)
            .bind(config::BindMode::Loopback)
            .add_agent("default", agent)
            .build();

        assert!(
            config.validate().is_ok(),
            "Wizard-generated config should be valid"
        );
        assert_eq!(config.agents.default, Some("default".to_string()));
        assert!(config.agents.agents.contains_key("default"));
    }

    #[test]
    fn test_wizard_config_serialization_roundtrip() {
        let mut agent = AgentConfig::default();
        agent.id = AgentId::new("test");
        agent.model = Some("openai/gpt-4o".to_string());

        let config = ConfigBuilder::new()
            .default_agent("test")
            .default_model("openai/gpt-4o")
            .add_agent("test", agent)
            .build();

        // Roundtrip through JSON
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.agents.default, Some("test".to_string()));
        assert!(parsed.agents.agents.contains_key("test"));
    }

    #[test]
    fn test_wizard_config_with_custom_settings() {
        let mut agent = AgentConfig::default();
        agent.id = AgentId::new("mybot");
        agent.model = Some("google/gemini-2.0-flash".to_string());
        agent.thinking_level = ThinkingLevel::High;

        let config = ConfigBuilder::new()
            .default_agent("mybot")
            .default_model("google/gemini-2.0-flash")
            .port(9090)
            .bind(config::BindMode::Lan)
            .add_agent("mybot", agent)
            .build();

        assert!(config.validate().is_ok());
        assert_eq!(config.gateway.port, 9090);
        assert_eq!(config.gateway.bind, config::BindMode::Lan);
        assert_eq!(
            config.agents.agents["mybot"].thinking_level,
            ThinkingLevel::High
        );
    }
}
