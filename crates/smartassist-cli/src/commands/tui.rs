//! TUI command.

use crate::tui::run_tui;
use clap::Args;
use smartassist_agent::providers::anthropic::AnthropicProvider;
use smartassist_agent::runtime::AgentRuntime;
use smartassist_agent::session::SessionManager;
use smartassist_agent::tools::ToolRegistry;
use smartassist_core::config::Config;
use smartassist_core::types::{AgentConfig, AgentId, SessionKey};
use std::sync::Arc;

/// TUI command arguments.
#[derive(Args)]
pub struct TuiArgs {
    /// Agent ID
    #[arg(short, long)]
    pub agent: Option<String>,

    /// Model override
    #[arg(short, long)]
    pub model: Option<String>,

    /// System prompt
    #[arg(short, long)]
    pub system: Option<String>,

    /// Resume session ID
    #[arg(long)]
    pub session: Option<String>,
}

/// Run the TUI command.
pub async fn run(args: TuiArgs) -> anyhow::Result<()> {
    let agent_id_str = args.agent.unwrap_or_else(|| "default".to_string());
    let agent_id = AgentId::new(&agent_id_str);

    let _config = Config::load_or_default();

    // Resolve API key from env
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .map_err(|_| anyhow::anyhow!(
            "No API key found. Set ANTHROPIC_API_KEY or run `smartassist init`."
        ))?;

    let provider: Arc<dyn smartassist_agent::providers::ModelProvider> =
        Arc::new(AnthropicProvider::new(api_key));

    let agent_config = AgentConfig {
        id: agent_id.clone(),
        model: args.model,
        system_prompt: args.system,
        ..AgentConfig::default()
    };

    let tool_registry = Arc::new(ToolRegistry::new());
    let sessions_dir = smartassist_core::paths::sessions_dir()
        .map_err(|e| anyhow::anyhow!("Failed to get sessions dir: {}", e))?;
    let session_manager = Arc::new(SessionManager::new(sessions_dir));

    let runtime = Arc::new(
        AgentRuntime::new(agent_config, provider, tool_registry, session_manager)
    );

    let session_key = match args.session {
        Some(id) => SessionKey::new(format!("{}:{}", agent_id_str, id)),
        None => SessionKey::new(format!(
            "{}:{}",
            agent_id_str,
            smartassist_core::id::uuid()
        )),
    };

    run_tui(runtime, session_key).await
}
