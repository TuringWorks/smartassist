//! SmartAssist command-line interface.

pub mod commands;
pub mod onboard;
pub mod render;
pub mod repl;

use clap::{Parser, Subcommand};

/// SmartAssist - AI agent gateway
#[derive(Parser)]
#[command(name = "smartassist")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Increase logging verbosity
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Path to config file
    #[arg(short, long, env = "SMARTASSIST_CONFIG")]
    pub config: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

/// Available commands
#[derive(Subcommand)]
pub enum Commands {
    /// Start the gateway server
    Gateway(commands::gateway::GatewayArgs),

    /// Manage agents
    Agent(commands::agent::AgentArgs),

    /// Manage channels
    Channels(commands::channels::ChannelsArgs),

    /// Configuration management
    Config(commands::config::ConfigArgs),

    /// Run diagnostics
    Doctor(commands::doctor::DoctorArgs),

    /// Manage encrypted secrets
    Secrets(commands::secrets::SecretsArgs),

    /// Manage plugins
    Plugins(commands::plugins::PluginsArgs),

    /// Initialize SmartAssist configuration
    Init {
        /// Overwrite existing configuration
        #[arg(long)]
        force: bool,
    },

    /// Show version information
    Version,
}

/// Run the CLI with the given arguments.
pub async fn run(cli: Cli) -> anyhow::Result<()> {
    match cli.command {
        Commands::Gateway(args) => commands::gateway::run(args).await,
        Commands::Agent(args) => commands::agent::run(args).await,
        Commands::Channels(args) => commands::channels::run(args).await,
        Commands::Config(args) => commands::config::run(args).await,
        Commands::Doctor(args) => commands::doctor::run(args).await,
        Commands::Secrets(args) => commands::secrets::run(args).await,
        Commands::Plugins(args) => commands::plugins::run(args).await,
        Commands::Init { force } => {
            onboard::OnboardWizard::new(force).run().await
        }
        Commands::Version => {
            println!("smartassist {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_parse_version() {
        let cli = Cli::try_parse_from(["smartassist", "version"]).unwrap();
        assert!(matches!(cli.command, Commands::Version));
    }

    #[test]
    fn test_parse_config_show() {
        let cli = Cli::try_parse_from(["smartassist", "config", "show"]).unwrap();
        match cli.command {
            Commands::Config(args) => {
                assert!(matches!(args.command, commands::config::ConfigCommand::Show));
            }
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_parse_config_get() {
        let cli = Cli::try_parse_from(["smartassist", "config", "get", "gateway.port"]).unwrap();
        match cli.command {
            Commands::Config(args) => match args.command {
                commands::config::ConfigCommand::Get { key } => {
                    assert_eq!(key, "gateway.port");
                }
                _ => panic!("Expected Config Get command"),
            },
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_parse_config_set() {
        let cli =
            Cli::try_parse_from(["smartassist", "config", "set", "gateway.port", "9090"]).unwrap();
        match cli.command {
            Commands::Config(args) => match args.command {
                commands::config::ConfigCommand::Set { key, value } => {
                    assert_eq!(key, "gateway.port");
                    assert_eq!(value, "9090");
                }
                _ => panic!("Expected Config Set command"),
            },
            _ => panic!("Expected Config command"),
        }
    }

    #[test]
    fn test_parse_agent_create() {
        let cli = Cli::try_parse_from([
            "smartassist",
            "agent",
            "create",
            "mybot",
            "--model",
            "anthropic/claude-3-opus",
        ])
        .unwrap();
        match cli.command {
            Commands::Agent(args) => match args.command {
                commands::agent::AgentCommand::Create { id, model, system } => {
                    assert_eq!(id, "mybot");
                    assert_eq!(model, Some("anthropic/claude-3-opus".to_string()));
                    assert!(system.is_none());
                }
                _ => panic!("Expected Agent Create command"),
            },
            _ => panic!("Expected Agent command"),
        }
    }

    #[test]
    fn test_parse_agent_chat() {
        let cli =
            Cli::try_parse_from(["smartassist", "agent", "chat", "--agent", "mybot"]).unwrap();
        match cli.command {
            Commands::Agent(args) => match args.command {
                commands::agent::AgentCommand::Chat { agent, .. } => {
                    assert_eq!(agent, Some("mybot".to_string()));
                }
                _ => panic!("Expected Agent Chat command"),
            },
            _ => panic!("Expected Agent command"),
        }
    }

    #[test]
    fn test_parse_channels_enable() {
        let cli =
            Cli::try_parse_from(["smartassist", "channels", "enable", "telegram"]).unwrap();
        match cli.command {
            Commands::Channels(args) => match args.command {
                commands::channels::ChannelsCommand::Enable { channel } => {
                    assert_eq!(channel, "telegram");
                }
                _ => panic!("Expected Channels Enable command"),
            },
            _ => panic!("Expected Channels command"),
        }
    }

    #[test]
    fn test_parse_gateway_run() {
        let cli = Cli::try_parse_from(["smartassist", "gateway", "run"]).unwrap();
        match cli.command {
            Commands::Gateway(args) => {
                assert!(matches!(
                    args.command,
                    commands::gateway::GatewayCommand::Run { .. }
                ));
            }
            _ => panic!("Expected Gateway command"),
        }
    }

    #[test]
    fn test_parse_doctor_full() {
        let cli = Cli::try_parse_from(["smartassist", "doctor", "--full"]).unwrap();
        match cli.command {
            Commands::Doctor(args) => {
                assert!(args.full);
            }
            _ => panic!("Expected Doctor command"),
        }
    }

    #[test]
    fn test_parse_init_force() {
        let cli = Cli::try_parse_from(["smartassist", "init", "--force"]).unwrap();
        match cli.command {
            Commands::Init { force } => {
                assert!(force);
            }
            _ => panic!("Expected Init command"),
        }
    }
}
