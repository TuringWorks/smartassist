//! Channel management commands.

use clap::Args;
use smartassist_core::config::{
    Config, DiscordConfig, SignalConfig, SlackConfig, TelegramConfig, WhatsAppConfig,
};
use std::net::TcpStream;

/// Channels command arguments.
#[derive(Args)]
pub struct ChannelsArgs {
    #[command(subcommand)]
    pub command: ChannelsCommand,
}

#[derive(clap::Subcommand)]
pub enum ChannelsCommand {
    /// List configured channels
    List,

    /// Show channel status
    Status {
        /// Probe channels for connectivity
        #[arg(long)]
        probe: bool,
    },

    /// Enable a channel
    Enable {
        /// Channel name
        channel: String,
    },

    /// Disable a channel
    Disable {
        /// Channel name
        channel: String,
    },

    /// Configure a channel
    Configure {
        /// Channel name
        channel: String,

        /// Configuration key=value pairs
        #[arg(short, long)]
        set: Vec<String>,
    },
}

/// Known channel names for matching.
const KNOWN_CHANNELS: &[&str] = &["telegram", "discord", "slack", "signal", "whatsapp"];

/// Print a single channel's status line.
fn print_channel_status(name: &str, configured: bool, enabled: bool, account_count: usize) {
    let status = if !configured {
        "not configured"
    } else if enabled {
        "enabled"
    } else {
        "disabled"
    };
    let accounts = if configured && account_count > 0 {
        format!("  ({} account(s))", account_count)
    } else {
        String::new()
    };
    println!("  {:<12} {}{}", name, status, accounts);
}

/// Run the channels command.
pub async fn run(args: ChannelsArgs) -> anyhow::Result<()> {
    match args.command {
        ChannelsCommand::List => {
            let config = Config::load_or_default();
            println!("Configured channels:\n");
            println!("  {:<12} {}", "CHANNEL", "STATUS");
            println!("  {}", "-".repeat(40));

            // Telegram
            let (configured, enabled, accounts) = match &config.channels.telegram {
                Some(t) => (true, t.enabled, t.accounts.len()),
                None => (false, false, 0),
            };
            print_channel_status("telegram", configured, enabled, accounts);

            // Discord
            let (configured, enabled, accounts) = match &config.channels.discord {
                Some(d) => (true, d.enabled, d.accounts.len()),
                None => (false, false, 0),
            };
            print_channel_status("discord", configured, enabled, accounts);

            // Slack
            let (configured, enabled, accounts) = match &config.channels.slack {
                Some(s) => (true, s.enabled, s.accounts.len()),
                None => (false, false, 0),
            };
            print_channel_status("slack", configured, enabled, accounts);

            // Signal
            let (configured, enabled) = match &config.channels.signal {
                Some(s) => (true, s.enabled),
                None => (false, false),
            };
            print_channel_status("signal", configured, enabled, 0);

            // WhatsApp
            let (configured, enabled, accounts) = match &config.channels.whatsapp {
                Some(w) => (true, w.enabled, w.accounts.len()),
                None => (false, false, 0),
            };
            print_channel_status("whatsapp", configured, enabled, accounts);
        }

        ChannelsCommand::Status { probe } => {
            let config = Config::load_or_default();
            println!("Channel status:\n");
            println!("  {:<12} {}", "CHANNEL", "STATUS");
            println!("  {}", "-".repeat(40));

            // Same listing as List
            let (configured, enabled, accounts) = match &config.channels.telegram {
                Some(t) => (true, t.enabled, t.accounts.len()),
                None => (false, false, 0),
            };
            print_channel_status("telegram", configured, enabled, accounts);

            let (configured, enabled, accounts) = match &config.channels.discord {
                Some(d) => (true, d.enabled, d.accounts.len()),
                None => (false, false, 0),
            };
            print_channel_status("discord", configured, enabled, accounts);

            let (configured, enabled, accounts) = match &config.channels.slack {
                Some(s) => (true, s.enabled, s.accounts.len()),
                None => (false, false, 0),
            };
            print_channel_status("slack", configured, enabled, accounts);

            let (configured, enabled) = match &config.channels.signal {
                Some(s) => (true, s.enabled),
                None => (false, false),
            };
            print_channel_status("signal", configured, enabled, 0);

            let (configured, enabled, accounts) = match &config.channels.whatsapp {
                Some(w) => (true, w.enabled, w.accounts.len()),
                None => (false, false, 0),
            };
            print_channel_status("whatsapp", configured, enabled, accounts);

            // If probe is enabled, check gateway connectivity
            if probe {
                let port = config.gateway.port;
                println!();
                match TcpStream::connect(format!("127.0.0.1:{}", port)) {
                    Ok(_) => println!("  Gateway is running on port {}", port),
                    Err(_) => println!("  Gateway is not running (port {})", port),
                }
            }
        }

        ChannelsCommand::Enable { channel } => {
            let name = channel.to_lowercase();
            if !KNOWN_CHANNELS.contains(&name.as_str()) {
                anyhow::bail!(
                    "Unknown channel: {}. Known channels: {}",
                    channel,
                    KNOWN_CHANNELS.join(", ")
                );
            }

            let mut config = Config::load_or_default();
            match name.as_str() {
                "telegram" => {
                    match config.channels.telegram.as_mut() {
                        Some(t) => t.enabled = true,
                        None => {
                            config.channels.telegram = Some(TelegramConfig {
                                enabled: true,
                                accounts: Default::default(),
                            });
                        }
                    }
                }
                "discord" => {
                    match config.channels.discord.as_mut() {
                        Some(d) => d.enabled = true,
                        None => {
                            config.channels.discord = Some(DiscordConfig {
                                enabled: true,
                                accounts: Default::default(),
                            });
                        }
                    }
                }
                "slack" => {
                    match config.channels.slack.as_mut() {
                        Some(s) => s.enabled = true,
                        None => {
                            config.channels.slack = Some(SlackConfig {
                                enabled: true,
                                accounts: Default::default(),
                            });
                        }
                    }
                }
                "signal" => {
                    match config.channels.signal.as_mut() {
                        Some(s) => s.enabled = true,
                        None => {
                            config.channels.signal = Some(SignalConfig {
                                enabled: true,
                                api_url: None,
                                phone_number: None,
                            });
                        }
                    }
                }
                "whatsapp" => {
                    match config.channels.whatsapp.as_mut() {
                        Some(w) => w.enabled = true,
                        None => {
                            config.channels.whatsapp = Some(WhatsAppConfig {
                                enabled: true,
                                accounts: Default::default(),
                            });
                        }
                    }
                }
                _ => unreachable!(),
            }

            config.save_default()?;
            println!("Enabled channel: {}", name);
        }

        ChannelsCommand::Disable { channel } => {
            let name = channel.to_lowercase();
            if !KNOWN_CHANNELS.contains(&name.as_str()) {
                anyhow::bail!(
                    "Unknown channel: {}. Known channels: {}",
                    channel,
                    KNOWN_CHANNELS.join(", ")
                );
            }

            let mut config = Config::load_or_default();
            match name.as_str() {
                "telegram" => {
                    if let Some(t) = config.channels.telegram.as_mut() {
                        t.enabled = false;
                    }
                }
                "discord" => {
                    if let Some(d) = config.channels.discord.as_mut() {
                        d.enabled = false;
                    }
                }
                "slack" => {
                    if let Some(s) = config.channels.slack.as_mut() {
                        s.enabled = false;
                    }
                }
                "signal" => {
                    if let Some(s) = config.channels.signal.as_mut() {
                        s.enabled = false;
                    }
                }
                "whatsapp" => {
                    if let Some(w) = config.channels.whatsapp.as_mut() {
                        w.enabled = false;
                    }
                }
                _ => unreachable!(),
            }

            config.save_default()?;
            println!("Disabled channel: {}", name);
        }

        ChannelsCommand::Configure { channel, set } => {
            let name = channel.to_lowercase();
            if !KNOWN_CHANNELS.contains(&name.as_str()) {
                anyhow::bail!(
                    "Unknown channel: {}. Known channels: {}",
                    channel,
                    KNOWN_CHANNELS.join(", ")
                );
            }

            // Load config as a raw JSON value so we can set arbitrary keys
            let config = Config::load_or_default();
            let mut json = serde_json::to_value(&config)?;

            // Ensure the channels.{name} object exists
            if json.get("channels").is_none() {
                json["channels"] = serde_json::json!({});
            }
            if json["channels"].get(&name).is_none() {
                json["channels"][&name] = serde_json::json!({"enabled": true});
            }

            // Apply each key=value pair
            for kv in &set {
                let (key, value) = kv.split_once('=').ok_or_else(|| {
                    anyhow::anyhow!("Invalid key=value pair: '{}'. Expected format: key=value", kv)
                })?;

                // Parse value: try JSON first, fall back to string
                let parsed: serde_json::Value = serde_json::from_str(value)
                    .unwrap_or_else(|_| serde_json::Value::String(value.to_string()));
                json["channels"][&name][key] = parsed;
            }

            // Deserialize back to validate shape
            let updated: Config = serde_json::from_value(json)
                .map_err(|e| anyhow::anyhow!("Invalid configuration after update: {}", e))?;
            updated.save_default()?;

            println!("Configured channel: {}", name);
            for kv in &set {
                println!("  {}", kv);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use smartassist_core::config::{Config, SignalConfig, TelegramConfig};

    /// Test that enabling a channel that is not yet configured creates the config entry.
    #[test]
    fn test_channel_enable_creates_config() {
        let mut config = Config::default();
        assert!(config.channels.telegram.is_none());

        // Replicate enable logic for telegram
        config.channels.telegram = Some(TelegramConfig {
            enabled: true,
            accounts: Default::default(),
        });

        let telegram = config.channels.telegram.as_ref().unwrap();
        assert!(telegram.enabled);
    }

    /// Test that enabling an already-configured channel sets enabled = true.
    #[test]
    fn test_channel_enable_existing() {
        let mut config = Config::default();
        config.channels.telegram = Some(TelegramConfig {
            enabled: false,
            accounts: Default::default(),
        });

        // Replicate enable logic
        if let Some(t) = config.channels.telegram.as_mut() {
            t.enabled = true;
        }

        assert!(config.channels.telegram.as_ref().unwrap().enabled);
    }

    /// Test that disabling a channel sets enabled = false.
    #[test]
    fn test_channel_disable() {
        let mut config = Config::default();
        config.channels.signal = Some(SignalConfig {
            enabled: true,
            api_url: Some("http://localhost:8080".to_string()),
            phone_number: None,
        });

        if let Some(s) = config.channels.signal.as_mut() {
            s.enabled = false;
        }

        assert!(!config.channels.signal.as_ref().unwrap().enabled);
    }

    /// Test that disabling a channel that is not configured is a no-op (no panic).
    #[test]
    fn test_channel_disable_unconfigured_is_noop() {
        let mut config = Config::default();
        assert!(config.channels.discord.is_none());

        // Replicate disable logic -- should not panic
        if let Some(d) = config.channels.discord.as_mut() {
            d.enabled = false;
        }

        // Still None, no change
        assert!(config.channels.discord.is_none());
    }

    /// Test that configure applies key=value pairs via JSON manipulation.
    #[test]
    fn test_channel_configure_applies_key_value() {
        let mut config = Config::default();
        config.channels.signal = Some(SignalConfig {
            enabled: true,
            api_url: None,
            phone_number: None,
        });

        // Replicate configure logic using JSON
        let mut json = serde_json::to_value(&config).unwrap();

        // Ensure channels.signal exists
        if json["channels"].get("signal").is_none() {
            json["channels"]["signal"] = serde_json::json!({"enabled": true});
        }

        // Apply key=value pairs
        let kv = "api_url=http://localhost:8080";
        let (key, value) = kv.split_once('=').unwrap();
        let parsed: serde_json::Value = serde_json::from_str(value)
            .unwrap_or_else(|_| serde_json::Value::String(value.to_string()));
        json["channels"]["signal"][key] = parsed;

        // Deserialize back to validate shape
        let updated: Config = serde_json::from_value(json).unwrap();
        assert_eq!(
            updated.channels.signal.as_ref().unwrap().api_url.as_deref(),
            Some("http://localhost:8080")
        );
    }

    /// Test that unknown channel names are correctly identified.
    #[test]
    fn test_known_channels_check() {
        let known: &[&str] = &["telegram", "discord", "slack", "signal", "whatsapp"];
        assert!(known.contains(&"telegram"));
        assert!(known.contains(&"discord"));
        assert!(!known.contains(&"irc"));
        assert!(!known.contains(&"matrix"));
    }

    /// Test channel config roundtrip through save/load with a temp directory.
    #[test]
    fn test_channel_config_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("config.json");

        let mut config = Config::default();
        config.channels.signal = Some(SignalConfig {
            enabled: true,
            api_url: Some("http://signal-api:8080".to_string()),
            phone_number: Some("+15551234567".to_string()),
        });

        config.save(&config_path).unwrap();

        let loaded = Config::load(&config_path).unwrap();
        let signal = loaded.channels.signal.as_ref().unwrap();
        assert!(signal.enabled);
        assert_eq!(signal.api_url.as_deref(), Some("http://signal-api:8080"));
        assert_eq!(signal.phone_number.as_deref(), Some("+15551234567"));
    }
}
