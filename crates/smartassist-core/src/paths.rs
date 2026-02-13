//! Path resolution utilities.

use crate::error::ConfigError;
use std::path::PathBuf;

/// Get the SmartAssist base directory (~/.smartassist).
pub fn base_dir() -> Result<PathBuf, ConfigError> {
    let home = dirs::home_dir().ok_or_else(|| {
        ConfigError::Validation("Could not determine home directory".to_string())
    })?;
    Ok(home.join(".smartassist"))
}

/// Get the main config file path (~/.smartassist/smartassist.json5).
pub fn config_file() -> Result<PathBuf, ConfigError> {
    Ok(base_dir()?.join("smartassist.json5"))
}

/// Get the auth profiles file path (~/.smartassist/auth-profiles.json).
pub fn auth_profiles_file() -> Result<PathBuf, ConfigError> {
    Ok(base_dir()?.join("auth-profiles.json"))
}

/// Get the models catalog file path (~/.smartassist/models.json).
pub fn models_file() -> Result<PathBuf, ConfigError> {
    Ok(base_dir()?.join("models.json"))
}

/// Get the sessions directory (~/.smartassist/sessions).
pub fn sessions_dir() -> Result<PathBuf, ConfigError> {
    Ok(base_dir()?.join("sessions"))
}

/// Get the agents directory (~/.smartassist/agents).
pub fn agents_dir() -> Result<PathBuf, ConfigError> {
    Ok(base_dir()?.join("agents"))
}

/// Get the audit log directory (~/.smartassist/audit).
pub fn audit_dir() -> Result<PathBuf, ConfigError> {
    Ok(base_dir()?.join("audit"))
}

/// Get the credentials directory (~/.smartassist/credentials).
pub fn credentials_dir() -> Result<PathBuf, ConfigError> {
    Ok(base_dir()?.join("credentials"))
}

/// Get the plugins directory (~/.smartassist/plugins).
pub fn plugins_dir() -> Result<PathBuf, ConfigError> {
    Ok(base_dir()?.join("plugins"))
}

/// Get an agent's directory (~/.smartassist/agents/{agent_id}).
pub fn agent_dir(agent_id: &str) -> Result<PathBuf, ConfigError> {
    Ok(agents_dir()?.join(agent_id))
}

/// Get an agent's workspace directory (~/.smartassist/workspace-{agent_id}).
pub fn agent_workspace(agent_id: &str) -> Result<PathBuf, ConfigError> {
    Ok(base_dir()?.join(format!("workspace-{}", agent_id)))
}

/// Get an agent's sessions directory.
pub fn agent_sessions_dir(agent_id: &str) -> Result<PathBuf, ConfigError> {
    Ok(agent_dir(agent_id)?.join("sessions"))
}

/// Ensure all required directories exist.
pub fn ensure_dirs() -> Result<(), ConfigError> {
    let dirs = [
        base_dir()?,
        sessions_dir()?,
        agents_dir()?,
        audit_dir()?,
        credentials_dir()?,
        plugins_dir()?,
    ];

    for dir in dirs {
        std::fs::create_dir_all(&dir)?;
    }

    Ok(())
}

/// Expand tilde (~) in a path.
pub fn expand_tilde(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Normalize a path (resolve symlinks, remove redundant components).
pub fn normalize(path: &std::path::Path) -> Result<PathBuf, std::io::Error> {
    path.canonicalize()
}

/// Check if a path is within a workspace directory.
pub fn is_within_workspace(path: &std::path::Path, workspace: &std::path::Path) -> bool {
    match (path.canonicalize(), workspace.canonicalize()) {
        (Ok(p), Ok(w)) => p.starts_with(&w),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_dir() {
        let dir = base_dir().unwrap();
        assert!(dir.ends_with(".smartassist"));
    }

    #[test]
    fn test_expand_tilde() {
        let expanded = expand_tilde("~/test");
        assert!(!expanded.to_string_lossy().contains('~'));
    }

    #[test]
    fn test_agent_paths() {
        let agent_id = "test_agent";
        let dir = agent_dir(agent_id).unwrap();
        assert!(dir.ends_with("test_agent"));

        let workspace = agent_workspace(agent_id).unwrap();
        assert!(workspace.to_string_lossy().contains("workspace-test_agent"));
    }
}
