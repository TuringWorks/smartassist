//! macOS-specific sandbox implementation.
//!
//! macOS uses a different sandboxing model than Linux, based on:
//! - sandbox-exec: Profile-based sandboxing using SBPL
//! - App Sandbox: Entitlements-based sandboxing (for App Store apps)
//!
//! Note: sandbox-exec is deprecated but still functional.
//! For new apps, Apple recommends using the App Sandbox framework.

#![cfg(target_os = "macos")]

use crate::error::SandboxError;
use crate::profile::{FilesystemRules, NetworkRules};
use crate::Result;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::debug;

/// macOS sandbox profile generator.
pub struct MacOsSandbox {
    /// Profile name.
    name: String,

    /// Filesystem rules.
    filesystem: FilesystemRules,

    /// Network rules.
    network: NetworkRules,

    /// Workspace directory.
    workspace: Option<PathBuf>,

    /// Allow subprocess creation.
    allow_subprocess: bool,
}

impl MacOsSandbox {
    /// Create a new macOS sandbox with default settings.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            filesystem: FilesystemRules::default(),
            network: NetworkRules::default(),
            workspace: None,
            allow_subprocess: true,
        }
    }

    /// Set filesystem rules.
    pub fn with_filesystem(mut self, rules: FilesystemRules) -> Self {
        self.filesystem = rules;
        self
    }

    /// Set network rules.
    pub fn with_network(mut self, rules: NetworkRules) -> Self {
        self.network = rules;
        self
    }

    /// Set workspace directory.
    pub fn with_workspace(mut self, workspace: impl Into<PathBuf>) -> Self {
        self.workspace = Some(workspace.into());
        self
    }

    /// Set subprocess creation permission.
    pub fn with_subprocess(mut self, allowed: bool) -> Self {
        self.allow_subprocess = allowed;
        self
    }

    /// Generate the SBPL (Sandbox Profile Language) profile.
    pub fn generate_profile(&self) -> String {
        let mut profile = String::new();

        // Version and base settings
        profile.push_str("(version 1)\n");
        profile.push_str(&format!("; Profile: {}\n\n", self.name));

        // Default deny
        profile.push_str("(deny default)\n\n");

        // Allow basic system operations
        profile.push_str("; Basic operations\n");
        profile.push_str("(allow signal (target self))\n");
        profile.push_str("(allow process-fork)\n");

        if self.allow_subprocess {
            profile.push_str("(allow process-exec*)\n");
        }

        profile.push_str("\n");

        // File system rules
        profile.push_str("; Filesystem access\n");
        self.add_filesystem_rules(&mut profile);

        // Network rules
        profile.push_str("\n; Network access\n");
        self.add_network_rules(&mut profile);

        // System access
        profile.push_str("\n; System access\n");
        profile.push_str("(allow sysctl-read)\n");
        profile.push_str("(allow mach-lookup)\n");
        profile.push_str("(allow ipc-posix-shm-read-data)\n");

        profile
    }

    /// Add filesystem rules to the profile.
    fn add_filesystem_rules(&self, profile: &mut String) {
        // Read paths
        for path in &self.filesystem.read_paths {
            let path_str = path.to_string_lossy();
            profile.push_str(&format!(
                "(allow file-read* (subpath \"{}\"))\n",
                path_str
            ));
        }

        // Write paths
        for path in &self.filesystem.write_paths {
            let path_str = path.to_string_lossy();
            profile.push_str(&format!(
                "(allow file-read* file-write* (subpath \"{}\"))\n",
                path_str
            ));
        }

        // Exec paths
        for path in &self.filesystem.exec_paths {
            let path_str = path.to_string_lossy();
            profile.push_str(&format!(
                "(allow file-read* process-exec* (subpath \"{}\"))\n",
                path_str
            ));
        }

        // Workspace
        if let Some(ref workspace) = self.workspace {
            if self.filesystem.allow_workspace {
                let path_str = workspace.to_string_lossy();
                profile.push_str(&format!(
                    "(allow file-read* file-write* (subpath \"{}\"))\n",
                    path_str
                ));
            }
        }

        // /tmp access
        if self.filesystem.allow_tmp {
            profile.push_str("(allow file-read* file-write* (subpath \"/tmp\"))\n");
            profile.push_str("(allow file-read* file-write* (subpath \"/private/tmp\"))\n");
        }

        // Blocked paths
        for path in &self.filesystem.blocked_paths {
            let path_str = path.to_string_lossy();
            profile.push_str(&format!("(deny file-read* (subpath \"{}\"))\n", path_str));
        }

        // Standard system paths for reading
        profile.push_str("(allow file-read* (literal \"/\"))\n");
        profile.push_str("(allow file-read* (subpath \"/usr\"))\n");
        profile.push_str("(allow file-read* (subpath \"/bin\"))\n");
        profile.push_str("(allow file-read* (subpath \"/sbin\"))\n");
        profile.push_str("(allow file-read* (subpath \"/Library\"))\n");
        profile.push_str("(allow file-read* (subpath \"/System\"))\n");
        profile.push_str("(allow file-read* (subpath \"/dev\"))\n");
    }

    /// Add network rules to the profile.
    fn add_network_rules(&self, profile: &mut String) {
        if !self.network.enabled {
            profile.push_str("(deny network*)\n");
            return;
        }

        if self.network.localhost_only {
            profile.push_str("(allow network* (local ip \"localhost:*\"))\n");
            profile.push_str("(allow network* (local ip \"127.0.0.1:*\"))\n");
            profile.push_str("(allow network* (local ip \"::1:*\"))\n");
        } else {
            profile.push_str("(allow network*)\n");
        }

        // Block specific ports
        for port in &self.network.blocked_ports {
            profile.push_str(&format!("(deny network* (remote tcp \"*:{}\"))\n", port));
        }
    }

    /// Write the profile to a temporary file and return the path.
    pub fn write_profile(&self) -> Result<PathBuf> {
        let profile_content = self.generate_profile();
        let temp_dir = std::env::temp_dir();
        let profile_path = temp_dir.join(format!("sandbox-{}.sb", self.name));

        std::fs::write(&profile_path, &profile_content)
            .map_err(|e| SandboxError::Io(e))?;

        debug!("Wrote sandbox profile to {:?}", profile_path);
        Ok(profile_path)
    }

    /// Create a sandboxed command.
    pub fn sandbox_command(&self, command: &str, args: &[&str]) -> Result<Command> {
        let profile_path = self.write_profile()?;

        let mut cmd = Command::new("sandbox-exec");
        cmd.arg("-f")
            .arg(&profile_path)
            .arg(command)
            .args(args);

        Ok(cmd)
    }
}

/// Pre-built macOS sandbox profiles.
pub mod profiles {
    use super::*;

    /// Create a minimal sandbox (very restrictive).
    pub fn minimal() -> MacOsSandbox {
        MacOsSandbox::new("minimal")
            .with_filesystem(FilesystemRules::read_only())
            .with_network(NetworkRules::disabled())
            .with_subprocess(false)
    }

    /// Create a standard sandbox for typical operations.
    pub fn standard(workspace: impl Into<PathBuf>) -> MacOsSandbox {
        MacOsSandbox::new("standard")
            .with_filesystem(FilesystemRules::workspace_write())
            .with_network(NetworkRules::localhost_only())
            .with_workspace(workspace)
            .with_subprocess(true)
    }

    /// Create a relaxed sandbox for trusted operations.
    pub fn relaxed(workspace: impl Into<PathBuf>) -> MacOsSandbox {
        MacOsSandbox::new("relaxed")
            .with_filesystem(FilesystemRules::workspace_write())
            .with_network(NetworkRules::enabled())
            .with_workspace(workspace)
            .with_subprocess(true)
    }
}

/// Check if sandbox-exec is available.
pub fn sandbox_exec_available() -> bool {
    Path::new("/usr/bin/sandbox-exec").exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_generation() {
        let sandbox = MacOsSandbox::new("test")
            .with_network(NetworkRules::disabled());

        let profile = sandbox.generate_profile();

        assert!(profile.contains("(version 1)"));
        assert!(profile.contains("(deny default)"));
        assert!(profile.contains("(deny network*)"));
    }

    #[test]
    fn test_localhost_network() {
        let sandbox = MacOsSandbox::new("test")
            .with_network(NetworkRules::localhost_only());

        let profile = sandbox.generate_profile();

        assert!(profile.contains("localhost"));
        assert!(profile.contains("127.0.0.1"));
    }

    #[test]
    fn test_workspace_access() {
        let sandbox = MacOsSandbox::new("test")
            .with_workspace("/tmp/workspace")
            .with_filesystem(FilesystemRules {
                allow_workspace: true,
                ..Default::default()
            });

        let profile = sandbox.generate_profile();

        assert!(profile.contains("/tmp/workspace"));
    }
}
