//! Landlock filesystem sandboxing for Linux.

#![cfg(target_os = "linux")]

use crate::error::SandboxError;
use crate::profile::FilesystemRules;
use crate::Result;
use landlock::{
    Access, AccessFs, PathBeneath, PathFd, Ruleset, RulesetAttr, RulesetCreatedAttr, ABI,
};
use std::path::Path;
use tracing::{debug, warn};

/// Landlock ruleset builder and applier.
pub struct LandlockRuleset {
    rules: FilesystemRules,
    workspace: Option<std::path::PathBuf>,
}

impl LandlockRuleset {
    /// Create a new landlock ruleset from filesystem rules.
    pub fn new(rules: FilesystemRules) -> Self {
        Self {
            rules,
            workspace: None,
        }
    }

    /// Set the workspace directory (will be granted write access if allowed).
    pub fn with_workspace(mut self, workspace: impl Into<std::path::PathBuf>) -> Self {
        self.workspace = Some(workspace.into());
        self
    }

    /// Check if landlock is available on this system.
    pub fn is_available() -> bool {
        // Check if landlock ABI is supported
        Ruleset::new()
            .handle_access(AccessFs::from_all(ABI::V1))
            .is_ok()
    }

    /// Build and apply the landlock ruleset.
    pub fn apply(&self) -> Result<()> {
        if !Self::is_available() {
            warn!("Landlock is not available on this system");
            return Ok(());
        }

        let abi = ABI::V3; // Use latest ABI

        // Create ruleset with all possible access rights
        let mut ruleset = Ruleset::new()
            .handle_access(AccessFs::from_all(abi))
            .map_err(|e| SandboxError::Landlock(e.to_string()))?;

        // Add read paths
        for path in &self.rules.read_paths {
            if let Err(e) = self.add_read_rule(&mut ruleset, path) {
                warn!("Failed to add read rule for {:?}: {}", path, e);
            }
        }

        // Add write paths
        for path in &self.rules.write_paths {
            if let Err(e) = self.add_write_rule(&mut ruleset, path) {
                warn!("Failed to add write rule for {:?}: {}", path, e);
            }
        }

        // Add exec paths
        for path in &self.rules.exec_paths {
            if let Err(e) = self.add_exec_rule(&mut ruleset, path) {
                warn!("Failed to add exec rule for {:?}: {}", path, e);
            }
        }

        // Add tmp access if allowed
        if self.rules.allow_tmp {
            if let Err(e) = self.add_write_rule(&mut ruleset, Path::new("/tmp")) {
                warn!("Failed to add /tmp rule: {}", e);
            }
        }

        // Add workspace access if allowed
        if self.rules.allow_workspace {
            if let Some(ref workspace) = self.workspace {
                if let Err(e) = self.add_write_rule(&mut ruleset, workspace) {
                    warn!("Failed to add workspace rule: {}", e);
                }
            }
        }

        // Create and enforce the ruleset
        let ruleset = ruleset
            .create()
            .map_err(|e| SandboxError::Landlock(e.to_string()))?;

        ruleset
            .set_no_new_privs(true)
            .restrict_self()
            .map_err(|e| SandboxError::Landlock(e.to_string()))?;

        debug!("Landlock ruleset applied successfully");
        Ok(())
    }

    /// Add a read-only rule for a path.
    fn add_read_rule(&self, ruleset: &mut Ruleset, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(()); // Skip non-existent paths
        }

        let fd = PathFd::new(path).map_err(|e| SandboxError::Landlock(e.to_string()))?;

        let access = AccessFs::ReadFile | AccessFs::ReadDir;

        ruleset
            .add_rule(PathBeneath::new(fd, access))
            .map_err(|e| SandboxError::Landlock(e.to_string()))?;

        Ok(())
    }

    /// Add a read-write rule for a path.
    fn add_write_rule(&self, ruleset: &mut Ruleset, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(()); // Skip non-existent paths
        }

        let fd = PathFd::new(path).map_err(|e| SandboxError::Landlock(e.to_string()))?;

        let access = AccessFs::ReadFile
            | AccessFs::ReadDir
            | AccessFs::WriteFile
            | AccessFs::RemoveFile
            | AccessFs::RemoveDir
            | AccessFs::MakeChar
            | AccessFs::MakeDir
            | AccessFs::MakeReg
            | AccessFs::MakeSock
            | AccessFs::MakeFifo
            | AccessFs::MakeBlock
            | AccessFs::MakeSym;

        ruleset
            .add_rule(PathBeneath::new(fd, access))
            .map_err(|e| SandboxError::Landlock(e.to_string()))?;

        Ok(())
    }

    /// Add an execute rule for a path.
    fn add_exec_rule(&self, ruleset: &mut Ruleset, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(()); // Skip non-existent paths
        }

        let fd = PathFd::new(path).map_err(|e| SandboxError::Landlock(e.to_string()))?;

        let access = AccessFs::ReadFile | AccessFs::Execute;

        ruleset
            .add_rule(PathBeneath::new(fd, access))
            .map_err(|e| SandboxError::Landlock(e.to_string()))?;

        Ok(())
    }
}

/// Pre-built landlock profiles.
pub mod profiles {
    use super::*;

    /// Create a minimal landlock ruleset (very restrictive).
    pub fn minimal() -> LandlockRuleset {
        LandlockRuleset::new(FilesystemRules::read_only())
    }

    /// Create a standard landlock ruleset.
    pub fn standard(workspace: impl Into<std::path::PathBuf>) -> LandlockRuleset {
        LandlockRuleset::new(FilesystemRules::workspace_write()).with_workspace(workspace)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landlock_availability() {
        // This test just checks the availability check doesn't panic
        let _ = LandlockRuleset::is_available();
    }

    #[test]
    fn test_ruleset_creation() {
        let ruleset = LandlockRuleset::new(FilesystemRules::default());
        assert!(ruleset.workspace.is_none());
    }

    #[test]
    fn test_ruleset_with_workspace() {
        let ruleset = LandlockRuleset::new(FilesystemRules::workspace_write())
            .with_workspace("/tmp/test");

        assert_eq!(
            ruleset.workspace,
            Some(std::path::PathBuf::from("/tmp/test"))
        );
    }
}
