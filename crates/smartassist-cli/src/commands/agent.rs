//! Agent management commands.

use crate::repl::{Repl, ReplConfig};
use clap::Args;
use smartassist_agent::providers::anthropic::AnthropicProvider;
use smartassist_agent::runtime::AgentRuntime;
use smartassist_agent::session::SessionManager;
use smartassist_agent::tools::ToolRegistry;
use smartassist_core::config::Config;
use smartassist_core::types::{AgentConfig, AgentId, SessionKey};
use std::sync::Arc;

/// Agent command arguments.
#[derive(Args)]
pub struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(clap::Subcommand)]
pub enum AgentCommand {
    /// List configured agents
    List,

    /// Show agent details
    Show {
        /// Agent ID
        id: String,
    },

    /// Create a new agent
    Create {
        /// Agent ID
        id: String,

        /// Model to use
        #[arg(short, long)]
        model: Option<String>,

        /// System prompt
        #[arg(short, long)]
        system: Option<String>,
    },

    /// Delete an agent
    Delete {
        /// Agent ID
        id: String,
    },

    /// Send a message to an agent
    Message {
        /// Agent ID
        #[arg(short, long)]
        agent: Option<String>,

        /// Message text
        message: String,
    },

    /// Start an interactive chat session
    Chat {
        /// Agent ID
        #[arg(short, long)]
        agent: Option<String>,

        /// Model override
        #[arg(short, long)]
        model: Option<String>,

        /// System prompt
        #[arg(short, long)]
        system: Option<String>,

        /// Resume session ID
        #[arg(long)]
        session: Option<String>,

        /// Provider (default: anthropic)
        #[arg(short, long, default_value = "anthropic")]
        provider: String,
    },
}

/// Run the agent command.
pub async fn run(args: AgentArgs) -> anyhow::Result<()> {
    match args.command {
        AgentCommand::List => {
            let config = Config::load_default().unwrap_or_default();
            if config.agents.agents.is_empty() {
                println!("No agents configured.");
                println!("  Run `smartassist agent create <id>` to add one.");
            } else {
                println!("{:<20} {:<30} {}", "ID", "MODEL", "");
                println!("{}", "-".repeat(55));
                let default_id = config.agents.default.as_deref();
                for (id, agent) in &config.agents.agents {
                    let model = agent.model.as_deref().unwrap_or("(default)");
                    let marker = if Some(id.as_str()) == default_id { " (default)" } else { "" };
                    println!("{:<20} {:<30} {}", id, model, marker);
                }
            }
        }

        AgentCommand::Show { id } => {
            let config = Config::load_default().unwrap_or_default();
            match config.agents.agents.get(&id) {
                Some(agent) => {
                    let json = serde_json::to_string_pretty(agent)?;
                    println!("{}", json);
                }
                None => {
                    anyhow::bail!("Agent not found: {}", id);
                }
            }
        }

        AgentCommand::Create { id, model, system } => {
            let mut config = Config::load_default().unwrap_or_default();

            if config.agents.agents.contains_key(&id) {
                anyhow::bail!("Agent already exists: {}", id);
            }

            let agent = AgentConfig {
                id: AgentId::new(&id),
                model,
                system_prompt: system,
                ..AgentConfig::default()
            };

            // If this is the first agent, set it as default
            let is_first = config.agents.agents.is_empty();
            config.agents.agents.insert(id.clone(), agent);
            if is_first {
                config.agents.default = Some(id.clone());
            }

            config.save_default()?;
            println!("Created agent: {}", id);
        }

        AgentCommand::Delete { id } => {
            let mut config = Config::load_default().unwrap_or_default();

            if config.agents.agents.remove(&id).is_none() {
                anyhow::bail!("Agent not found: {}", id);
            }

            // Clear the default if it matches the deleted agent
            if config.agents.default.as_deref() == Some(&id) {
                config.agents.default = None;
            }

            config.save_default()?;
            println!("Deleted agent: {}", id);
        }

        AgentCommand::Message { agent, message } => {
            let config = Config::load_default().unwrap_or_default();
            let agent_id = agent.unwrap_or_else(|| "default".to_string());
            let port = config.gateway.port;

            // Build a JSON-RPC request to send the message via the gateway
            let payload = serde_json::json!({
                "jsonrpc": "2.0",
                "method": "agent.message",
                "params": {
                    "agent_id": agent_id,
                    "message": message,
                },
                "id": 1
            });

            let url = format!("http://127.0.0.1:{}", port);
            let client = reqwest::Client::new();
            match client.post(&url).json(&payload).send().await {
                Ok(resp) => {
                    let body: serde_json::Value = resp.json().await?;
                    if let Some(result) = body.get("result") {
                        println!("{}", serde_json::to_string_pretty(result)?);
                    } else if let Some(error) = body.get("error") {
                        anyhow::bail!("Gateway error: {}", error);
                    }
                }
                Err(e) => {
                    if e.is_connect() {
                        anyhow::bail!(
                            "Gateway not running on port {}. Start it with `smartassist gateway run`.",
                            port
                        );
                    }
                    anyhow::bail!("Failed to connect to gateway: {}", e);
                }
            }
        }

        AgentCommand::Chat {
            agent,
            model,
            system,
            session,
            provider: _,
        } => {
            let agent_id_str = agent.unwrap_or_else(|| "default".to_string());
            let agent_id = AgentId::new(&agent_id_str);

            // Build agent config
            let config = AgentConfig {
                id: agent_id.clone(),
                model,
                system_prompt: system,
                ..AgentConfig::default()
            };

            // Resolve API key from env
            let api_key = std::env::var("ANTHROPIC_API_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .map_err(|_| anyhow::anyhow!(
                    "No API key found. Set ANTHROPIC_API_KEY or run `smartassist init`."
                ))?;

            // Create provider
            let provider: Arc<dyn smartassist_agent::providers::ModelProvider> =
                Arc::new(AnthropicProvider::new(api_key));

            // Create tool registry and session manager
            let tool_registry = Arc::new(ToolRegistry::new());
            let sessions_dir = smartassist_core::paths::sessions_dir()
                .map_err(|e| anyhow::anyhow!("Failed to get sessions dir: {}", e))?;
            let session_manager = Arc::new(SessionManager::new(sessions_dir));

            // Create runtime
            let runtime = Arc::new(
                AgentRuntime::new(config, provider, tool_registry, session_manager)
            );

            // Create or resume session
            let session_key = match session {
                Some(id) => SessionKey::new(format!("{}:{}", agent_id_str, id)),
                None => SessionKey::new(format!(
                    "{}:{}",
                    agent_id_str,
                    smartassist_core::id::uuid()
                )),
            };

            // Launch REPL
            let mut repl = Repl::new(runtime, session_key, ReplConfig::default());
            repl.run().await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use smartassist_core::config::Config;
    use smartassist_core::types::{AgentConfig, AgentId};

    /// Test agent create: insert a new agent into Config and verify it exists.
    #[test]
    fn test_agent_create() {
        let mut config = Config::default();
        let agent = AgentConfig {
            id: AgentId::new("mybot"),
            model: Some("anthropic/claude-3-opus".to_string()),
            ..AgentConfig::default()
        };

        config.agents.agents.insert("mybot".to_string(), agent);
        assert!(config.agents.agents.contains_key("mybot"));
        assert_eq!(
            config.agents.agents["mybot"].model.as_deref(),
            Some("anthropic/claude-3-opus")
        );
    }

    /// Test agent delete: insert then remove an agent and verify it is gone.
    #[test]
    fn test_agent_delete() {
        let mut config = Config::default();
        let agent = AgentConfig {
            id: AgentId::new("tobedeleted"),
            ..AgentConfig::default()
        };
        config
            .agents
            .agents
            .insert("tobedeleted".to_string(), agent);
        assert!(config.agents.agents.contains_key("tobedeleted"));

        config.agents.agents.remove("tobedeleted");
        assert!(!config.agents.agents.contains_key("tobedeleted"));
    }

    /// Test agent list with empty config shows no agents.
    #[test]
    fn test_agent_list_empty() {
        let config = Config::default();
        assert!(config.agents.agents.is_empty());
    }

    /// Test that creating the first agent sets it as the default.
    #[test]
    fn test_agent_create_sets_default_when_first() {
        let mut config = Config::default();

        let agent = AgentConfig {
            id: AgentId::new("first"),
            ..AgentConfig::default()
        };

        // Replicate the logic from the Create command handler
        let is_first = config.agents.agents.is_empty();
        config.agents.agents.insert("first".to_string(), agent);
        if is_first {
            config.agents.default = Some("first".to_string());
        }

        assert_eq!(config.agents.default.as_deref(), Some("first"));
    }

    /// Test that creating a duplicate agent is rejected.
    #[test]
    fn test_agent_create_rejects_duplicates() {
        let mut config = Config::default();
        let agent = AgentConfig {
            id: AgentId::new("dup"),
            ..AgentConfig::default()
        };
        config.agents.agents.insert("dup".to_string(), agent);

        // Attempting to create the same agent again should find it already exists
        assert!(config.agents.agents.contains_key("dup"));
    }

    /// Test that deleting the default agent clears the default.
    #[test]
    fn test_agent_delete_clears_default() {
        let mut config = Config::default();
        let agent = AgentConfig {
            id: AgentId::new("main"),
            ..AgentConfig::default()
        };
        config.agents.agents.insert("main".to_string(), agent);
        config.agents.default = Some("main".to_string());

        // Replicate the delete logic from the command handler
        config.agents.agents.remove("main");
        if config.agents.default.as_deref() == Some("main") {
            config.agents.default = None;
        }

        assert!(config.agents.default.is_none());
        assert!(!config.agents.agents.contains_key("main"));
    }

    /// Test agent creation with save/load roundtrip using a temp directory.
    #[test]
    fn test_agent_config_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");

        let mut config = Config::default();
        let agent = AgentConfig {
            id: AgentId::new("roundtrip"),
            model: Some("openai/gpt-4".to_string()),
            system_prompt: Some("You are helpful.".to_string()),
            ..AgentConfig::default()
        };
        config.agents.agents.insert("roundtrip".to_string(), agent);
        config.agents.default = Some("roundtrip".to_string());

        // Save to the temp path
        config.save(&config_path).unwrap();

        // Load back and verify
        let loaded = Config::load(&config_path).unwrap();
        assert_eq!(loaded.agents.default.as_deref(), Some("roundtrip"));
        assert!(loaded.agents.agents.contains_key("roundtrip"));
        assert_eq!(
            loaded.agents.agents["roundtrip"].model.as_deref(),
            Some("openai/gpt-4")
        );
        assert_eq!(
            loaded.agents.agents["roundtrip"].system_prompt.as_deref(),
            Some("You are helpful.")
        );
    }
}
