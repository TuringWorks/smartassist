//! Onboarding wizard for first-run configuration.
//!
//! Provides `smartassist init` -- a 4-step interactive wizard that guides
//! users through provider selection, API key setup, model selection,
//! and configuration file creation.

use console::style;
use smartassist_core::paths;
use smartassist_secrets::{FileSecretStore, SecretStore};
use std::io::{self, Write};

/// The onboarding wizard.
pub struct OnboardWizard {
    force: bool,
}

/// Supported providers.
#[derive(Debug, Clone, Copy)]
enum Provider {
    Anthropic,
    OpenAI,
    Google,
    Ollama,
}

impl Provider {
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

    fn default_model(&self) -> &str {
        match self {
            Self::Anthropic => "claude-sonnet-4-5-20250929",
            Self::OpenAI => "gpt-4o",
            Self::Google => "gemini-2.0-flash",
            Self::Ollama => "llama3.2",
        }
    }

    fn api_key_prefix(&self) -> Option<&str> {
        match self {
            Self::Anthropic => Some("sk-ant-"),
            Self::OpenAI => Some("sk-"),
            Self::Google => None, // Google keys don't have a consistent prefix
            Self::Ollama => None, // No API key needed
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
}

impl OnboardWizard {
    /// Create a new wizard.
    pub fn new(force: bool) -> Self {
        Self { force }
    }

    /// Run the 4-step wizard.
    pub async fn run(&self) -> anyhow::Result<()> {
        // Check if config already exists
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

        // Welcome
        self.step_welcome();

        // Step 1: Provider selection
        let provider = self.step_provider()?;

        // Step 2: API key setup
        self.step_api_key(provider).await?;

        // Step 3: Model selection
        let model = self.step_model(provider)?;

        // Step 4: Write config
        self.step_write_config(provider, &model)?;

        Ok(())
    }

    /// Display the welcome banner.
    fn step_welcome(&self) {
        eprintln!();
        eprintln!("  {}", style("Welcome to SmartAssist!").bold().cyan());
        eprintln!("  {}", style("Let's set up your configuration.").dim());
        eprintln!();
    }

    /// Step 1: Select a provider.
    fn step_provider(&self) -> anyhow::Result<Provider> {
        let providers = [
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Google,
            Provider::Ollama,
        ];

        eprintln!("{}", style("Step 1: Choose your AI provider").bold());
        eprintln!();
        for (i, p) in providers.iter().enumerate() {
            let default_marker = if i == 0 { " (default)" } else { "" };
            eprintln!(
                "  {} {}{}",
                style(format!("[{}]", i + 1)).cyan(),
                p.name(),
                style(default_marker).dim(),
            );
        }
        eprintln!();

        let choice = prompt_input("Select provider [1]: ")?;
        let idx = if choice.is_empty() {
            0
        } else {
            choice.parse::<usize>().unwrap_or(1).saturating_sub(1)
        };

        let provider = providers.get(idx).copied().unwrap_or(Provider::Anthropic);
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
                let use_env = prompt_input("Use environment variable? [Y/n]: ")?;
                if use_env.is_empty() || use_env.to_lowercase().starts_with('y') {
                    eprintln!();
                    return Ok(());
                }
            }
        }

        // Prompt for API key
        let prompt_msg = format!("Enter your {} API key: ", provider.config_key());
        let api_key = rpassword::prompt_password(&prompt_msg)
            .map_err(|e| anyhow::anyhow!("Failed to read API key: {}", e))?;

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
        eprintln!("{}", style("Step 3: Choose your default model").bold());
        eprintln!();

        let models: Vec<(&str, &str)> = match provider {
            Provider::Anthropic => vec![
                ("claude-sonnet-4-5-20250929", "Claude Sonnet 4.5 (balanced)"),
                ("claude-opus-4-6", "Claude Opus 4.6 (most capable)"),
                ("claude-haiku-4-5-20251001", "Claude Haiku 4.5 (fastest)"),
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
        };

        for (i, (id, desc)) in models.iter().enumerate() {
            let default_marker = if i == 0 { " (default)" } else { "" };
            eprintln!(
                "  {} {} - {}{}",
                style(format!("[{}]", i + 1)).cyan(),
                id,
                desc,
                style(default_marker).dim(),
            );
        }
        eprintln!();

        let choice = prompt_input("Select model [1]: ")?;
        let idx = if choice.is_empty() {
            0
        } else {
            choice.parse::<usize>().unwrap_or(1).saturating_sub(1)
        };

        let (model_id, _) = models.get(idx).unwrap_or(&models[0]);
        eprintln!(
            "  {} Selected: {}",
            style("*").green(),
            style(model_id).bold(),
        );
        eprintln!();

        Ok(model_id.to_string())
    }

    /// Step 4: Write configuration file.
    fn step_write_config(&self, provider: Provider, model: &str) -> anyhow::Result<()> {
        eprintln!("{}", style("Step 4: Writing configuration").bold());
        eprintln!();

        // Ensure directories exist
        paths::ensure_dirs()
            .map_err(|e| anyhow::anyhow!("Failed to create directories: {}", e))?;

        let config_path = paths::config_file()
            .map_err(|e| anyhow::anyhow!("Failed to get config path: {}", e))?;

        // Build config content (JSON5)
        let config_content = format!(
            r#"{{
  // SmartAssist configuration
  // See: https://docs.smartassist.dev/configuration

  // Default provider
  "default_provider": "{}",

  // Default model
  "default_model": "{}/{}",

  // Gateway settings
  "gateway": {{
    "port": 18789,
    "bind": "loopback"
  }},

  // Agent defaults
  "agents": {{
    "defaults": {{
      "provider": "{}",
      "model": "{}",
      "max_turns": 10,
      "temperature": 0.7
    }}
  }}
}}
"#,
            provider.config_key(),
            provider.config_key(),
            model,
            provider.config_key(),
            model,
        );

        std::fs::write(&config_path, &config_content)
            .map_err(|e| anyhow::anyhow!("Failed to write config: {}", e))?;

        eprintln!(
            "  {} Configuration written to {}",
            style("*").green(),
            style(config_path.display()).dim(),
        );
        eprintln!();

        // Success message
        eprintln!(
            "  {} {}",
            style("Setup complete!").green().bold(),
            style("You're ready to go.").dim(),
        );
        eprintln!();
        eprintln!("  Next steps:");
        eprintln!(
            "    {} Start chatting",
            style("smartassist agent chat").cyan(),
        );
        eprintln!(
            "    {} Start the gateway",
            style("smartassist gateway start").cyan(),
        );
        eprintln!(
            "    {} Run diagnostics",
            style("smartassist doctor").cyan(),
        );
        eprintln!();

        Ok(())
    }
}

/// Prompt for user input.
fn prompt_input(prompt: &str) -> anyhow::Result<String> {
    eprint!("  {}", prompt);
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}
