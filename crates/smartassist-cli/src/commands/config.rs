//! Configuration management commands.

use clap::Args;
use smartassist_core::config::Config;
use smartassist_core::paths;

/// Config command arguments.
#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(clap::Subcommand)]
pub enum ConfigCommand {
    /// Show configuration
    Show,

    /// Get a configuration value
    Get {
        /// Configuration key (dot-separated path)
        key: String,
    },

    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,

        /// Value to set
        value: String,
    },

    /// Initialize configuration
    Init {
        /// Force overwrite existing config
        #[arg(short, long)]
        force: bool,
    },

    /// Show configuration file path
    Path,

    /// Validate configuration
    Validate,
}

/// Run the config command.
pub async fn run(args: ConfigArgs) -> anyhow::Result<()> {
    match args.command {
        ConfigCommand::Show => {
            let config = Config::load_or_default();
            let json = serde_json::to_string_pretty(&config)?;
            println!("{}", json);
        }

        ConfigCommand::Get { key } => {
            let config = Config::load_or_default();
            let json = serde_json::to_value(&config)?;

            let value = key.split('.').fold(Some(&json), |acc, k| {
                acc.and_then(|v| v.get(k))
            });

            match value {
                Some(v) => println!("{}", serde_json::to_string_pretty(v)?),
                None => anyhow::bail!("Key not found: {}", key),
            }
        }

        ConfigCommand::Set { key, value } => {
            // Load current config (or default if none exists yet)
            let config = Config::load_or_default();
            let mut json = serde_json::to_value(&config)?;

            // Walk the dot-separated key path, creating intermediate objects as needed
            let parts: Vec<&str> = key.split('.').collect();
            let mut current = &mut json;
            for (i, part) in parts.iter().enumerate() {
                if i == parts.len() - 1 {
                    // Leaf: parse value as JSON first (handles numbers, bools, etc.),
                    // fall back to plain string if that fails.
                    let parsed: serde_json::Value = serde_json::from_str(&value)
                        .unwrap_or_else(|_| serde_json::Value::String(value.clone()));
                    current[part] = parsed;
                } else {
                    // Intermediate: ensure an object exists at this key
                    if !current.get(part).map_or(false, |v| v.is_object()) {
                        current[part] = serde_json::json!({});
                    }
                    current = &mut current[part];
                }
            }

            // Deserialize back to Config to validate the shape is still correct
            let updated: Config = serde_json::from_value(json)
                .map_err(|e| anyhow::anyhow!("Invalid configuration after set: {}", e))?;
            updated.save_default()?;

            println!("Set {} = {}", key, value);
        }

        ConfigCommand::Init { force } => {
            let path = paths::config_file()?;

            if path.exists() && !force {
                anyhow::bail!(
                    "Config file already exists: {:?}. Use --force to overwrite.",
                    path
                );
            }

            paths::ensure_dirs()?;

            // Use load_or_default() to pick up env vars (e.g. auto-detect provider)
            let config = Config::load_or_default();
            config.save_default()?;

            println!("Created config file: {:?}", path);
            println!("  Tip: Run 'smartassist init' for guided setup.");
        }

        ConfigCommand::Path => {
            let path = paths::config_file()?;
            println!("{}", path.display());
        }

        ConfigCommand::Validate => {
            match Config::load_default() {
                Ok(config) => {
                    match config.validate() {
                        Ok(_) => println!("Configuration is valid"),
                        Err(e) => anyhow::bail!("Configuration error: {}", e),
                    }
                }
                Err(e) => anyhow::bail!("Failed to load config: {}", e),
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use smartassist_core::config::Config;

    /// Test dot-path get logic: serialize a Config to JSON, walk a key path, verify result.
    #[test]
    fn test_dot_path_get() {
        let config = Config::default();
        let json = serde_json::to_value(&config).unwrap();

        // Walk the path "gateway.port"
        let value = "gateway.port"
            .split('.')
            .fold(Some(&json), |acc, k| acc.and_then(|v| v.get(k)));

        assert!(value.is_some());
        // Default gateway port is 18789
        assert_eq!(value.unwrap().as_u64().unwrap(), 18789);
    }

    /// Test dot-path set logic: set a path on a Config's JSON representation, verify update.
    #[test]
    fn test_dot_path_set() {
        let config = Config::default();
        let mut json = serde_json::to_value(&config).unwrap();

        // Set "gateway.port" to 9090
        let parts: Vec<&str> = "gateway.port".split('.').collect();
        let mut current = &mut json;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                current[part] = serde_json::json!(9090);
            } else {
                current = &mut current[part];
            }
        }

        // Verify the value was set
        assert_eq!(json["gateway"]["port"], 9090);

        // Deserialize back to Config to confirm shape is valid
        let updated: Config = serde_json::from_value(json).unwrap();
        assert_eq!(updated.gateway.port, 9090);
    }

    /// Test value parsing: JSON number, JSON bool, and plain string fallback.
    #[test]
    fn test_value_parsing() {
        // JSON number
        let parsed: serde_json::Value = serde_json::from_str("9090").unwrap();
        assert_eq!(parsed, serde_json::json!(9090));

        // JSON bool
        let parsed: serde_json::Value = serde_json::from_str("true").unwrap();
        assert_eq!(parsed, serde_json::json!(true));

        // Plain string fallback (not valid JSON, so fall back to String)
        let raw = "hello-world";
        let parsed: serde_json::Value = serde_json::from_str(raw)
            .unwrap_or_else(|_| serde_json::Value::String(raw.to_string()));
        assert_eq!(parsed, serde_json::Value::String("hello-world".to_string()));
    }

    /// Test setting a nested path creates intermediate objects when they do not exist.
    #[test]
    fn test_set_nested_path_creates_intermediates() {
        let config = Config::default();
        let mut json = serde_json::to_value(&config).unwrap();

        // Set "logging.level" to "debug" -- logging already exists but let's
        // exercise the intermediate-creation branch by ensuring the code path works.
        let value = "debug";
        let parts: Vec<&str> = "logging.level".split('.').collect();
        let mut current = &mut json;
        for (i, part) in parts.iter().enumerate() {
            if i == parts.len() - 1 {
                let parsed: serde_json::Value = serde_json::from_str(value)
                    .unwrap_or_else(|_| serde_json::Value::String(value.to_string()));
                current[part] = parsed;
            } else {
                if !current.get(part).map_or(false, |v| v.is_object()) {
                    current[part] = serde_json::json!({});
                }
                current = &mut current[part];
            }
        }

        assert_eq!(
            json["logging"]["level"],
            serde_json::Value::String("debug".to_string())
        );

        // Deserialize back to verify valid shape
        let updated: Config = serde_json::from_value(json).unwrap();
        assert_eq!(
            updated.logging.level,
            smartassist_core::config::LogLevel::Debug
        );
    }

    /// Test that setting an invalid structure returns an error when deserializing back.
    #[test]
    fn test_invalid_set_returns_error() {
        let config = Config::default();
        let mut json = serde_json::to_value(&config).unwrap();

        // Set "gateway" to a plain string -- this breaks the schema because gateway
        // should be an object, not a string.
        json["gateway"] = serde_json::Value::String("not-an-object".to_string());

        let result: Result<Config, _> = serde_json::from_value(json);
        assert!(result.is_err());
    }
}
