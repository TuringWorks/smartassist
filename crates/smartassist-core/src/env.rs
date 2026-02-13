//! Environment variable handling.

use std::collections::HashMap;
use std::env;

/// Get an environment variable, returning None if not set or empty.
pub fn get_var(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.is_empty())
}

/// Get an environment variable with a default value.
pub fn get_var_or(name: &str, default: &str) -> String {
    get_var(name).unwrap_or_else(|| default.to_string())
}

/// Get an environment variable as a boolean.
pub fn get_bool(name: &str) -> bool {
    get_var(name)
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

/// Get an environment variable as a u16 (e.g., for ports).
pub fn get_u16(name: &str) -> Option<u16> {
    get_var(name).and_then(|v| v.parse().ok())
}

/// Get an environment variable as a usize.
pub fn get_usize(name: &str) -> Option<usize> {
    get_var(name).and_then(|v| v.parse().ok())
}

/// Load environment variables from a .env file.
pub fn load_dotenv() -> Result<(), std::io::Error> {
    let path = std::path::Path::new(".env");
    if path.exists() {
        let content = std::fs::read_to_string(path)?;
        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse KEY=value
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                // Remove quotes if present
                let value = value
                    .strip_prefix('"')
                    .and_then(|v| v.strip_suffix('"'))
                    .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                    .unwrap_or(value);

                // Only set if not already set
                if env::var(key).is_err() {
                    env::set_var(key, value);
                }
            }
        }
    }
    Ok(())
}

/// Filter environment variables, removing blocked ones.
pub fn filter_env(env: &HashMap<String, String>) -> HashMap<String, String> {
    use crate::types::is_env_var_blocked;

    env.iter()
        .filter(|(k, _)| !is_env_var_blocked(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Get a filtered copy of the current environment.
pub fn get_filtered_env() -> HashMap<String, String> {
    let current: HashMap<String, String> = env::vars().collect();
    filter_env(&current)
}

/// Common environment variable names.
pub mod vars {
    /// API key for Anthropic.
    pub const ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";

    /// API key for OpenAI.
    pub const OPENAI_API_KEY: &str = "OPENAI_API_KEY";

    /// API key for Google.
    pub const GOOGLE_API_KEY: &str = "GOOGLE_API_KEY";

    /// SmartAssist home directory override.
    pub const SMARTASSIST_HOME: &str = "SMARTASSIST_HOME";

    /// SmartAssist config file override.
    pub const SMARTASSIST_CONFIG: &str = "SMARTASSIST_CONFIG";

    /// SmartAssist log level.
    pub const SMARTASSIST_LOG: &str = "SMARTASSIST_LOG";

    /// SmartAssist debug mode.
    pub const SMARTASSIST_DEBUG: &str = "SMARTASSIST_DEBUG";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_bool() {
        env::set_var("TEST_BOOL_TRUE", "true");
        env::set_var("TEST_BOOL_1", "1");
        env::set_var("TEST_BOOL_FALSE", "false");
        env::set_var("TEST_BOOL_0", "0");

        assert!(get_bool("TEST_BOOL_TRUE"));
        assert!(get_bool("TEST_BOOL_1"));
        assert!(!get_bool("TEST_BOOL_FALSE"));
        assert!(!get_bool("TEST_BOOL_0"));
        assert!(!get_bool("TEST_BOOL_NONEXISTENT"));
    }

    #[test]
    fn test_filter_env() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("HOME".to_string(), "/home/user".to_string());
        env.insert("LD_PRELOAD".to_string(), "/evil.so".to_string());
        env.insert("NODE_OPTIONS".to_string(), "--inspect".to_string());

        let filtered = filter_env(&env);

        assert!(filtered.contains_key("PATH"));
        assert!(filtered.contains_key("HOME"));
        assert!(!filtered.contains_key("LD_PRELOAD"));
        assert!(!filtered.contains_key("NODE_OPTIONS"));
    }
}
