//! Linux capability management.

#![cfg(target_os = "linux")]

use crate::error::SandboxError;
use crate::Result;
use caps::{CapSet, Capability, CapsHashSet};
use std::collections::HashSet;
use tracing::debug;

/// Capability set management.
pub struct CapabilitySet {
    /// Capabilities to keep.
    keep: HashSet<Capability>,

    /// Whether to drop all capabilities first.
    drop_all: bool,
}

impl Default for CapabilitySet {
    fn default() -> Self {
        Self {
            keep: HashSet::new(),
            drop_all: true,
        }
    }
}

impl CapabilitySet {
    /// Create a new capability set that drops all capabilities.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a capability set that keeps specific capabilities.
    pub fn with_capabilities(caps: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            keep: caps.into_iter().collect(),
            drop_all: true,
        }
    }

    /// Create a minimal capability set (no capabilities).
    pub fn minimal() -> Self {
        Self::new()
    }

    /// Create a standard capability set for typical operations.
    pub fn standard() -> Self {
        Self::with_capabilities([
            Capability::CAP_CHOWN,
            Capability::CAP_DAC_OVERRIDE,
            Capability::CAP_FOWNER,
            Capability::CAP_FSETID,
            Capability::CAP_KILL,
            Capability::CAP_SETGID,
            Capability::CAP_SETUID,
            Capability::CAP_NET_BIND_SERVICE,
        ])
    }

    /// Add a capability to keep.
    pub fn keep(mut self, cap: Capability) -> Self {
        self.keep.insert(cap);
        self
    }

    /// Add multiple capabilities to keep.
    pub fn keep_all(mut self, caps: impl IntoIterator<Item = Capability>) -> Self {
        self.keep.extend(caps);
        self
    }

    /// Set whether to drop all capabilities first.
    pub fn drop_all(mut self, drop: bool) -> Self {
        self.drop_all = drop;
        self
    }

    /// Apply the capability set to the current process.
    pub fn apply(&self) -> Result<()> {
        if self.drop_all {
            // Drop all capabilities from all sets
            self.drop_capabilities()?;
        }

        // Set the ambient capabilities (if keeping any)
        if !self.keep.is_empty() {
            self.set_ambient_capabilities()?;
        }

        debug!(
            "Capabilities applied: keeping {:?}",
            self.keep.iter().collect::<Vec<_>>()
        );
        Ok(())
    }

    /// Drop all capabilities from all sets.
    fn drop_capabilities(&self) -> Result<()> {
        // Clear bounding set
        let current_bounding = caps::read(None, CapSet::Bounding)
            .map_err(|e| SandboxError::Capability(e.to_string()))?;

        for cap in current_bounding {
            if !self.keep.contains(&cap) {
                caps::drop(None, CapSet::Bounding, cap)
                    .map_err(|e| SandboxError::Capability(e.to_string()))?;
            }
        }

        // Clear inheritable set
        caps::clear(None, CapSet::Inheritable)
            .map_err(|e| SandboxError::Capability(e.to_string()))?;

        // Clear effective set (except kept caps)
        let mut effective = CapsHashSet::new();
        for cap in &self.keep {
            effective.insert(*cap);
        }
        caps::set(None, CapSet::Effective, &effective)
            .map_err(|e| SandboxError::Capability(e.to_string()))?;

        // Clear permitted set (except kept caps)
        caps::set(None, CapSet::Permitted, &effective)
            .map_err(|e| SandboxError::Capability(e.to_string()))?;

        Ok(())
    }

    /// Set ambient capabilities (for passing to child processes).
    fn set_ambient_capabilities(&self) -> Result<()> {
        // First, ensure capabilities are in inheritable set
        let mut inheritable = CapsHashSet::new();
        for cap in &self.keep {
            inheritable.insert(*cap);
        }
        caps::set(None, CapSet::Inheritable, &inheritable)
            .map_err(|e| SandboxError::Capability(e.to_string()))?;

        // Set ambient capabilities
        for cap in &self.keep {
            caps::raise(None, CapSet::Ambient, *cap)
                .map_err(|e| SandboxError::Capability(e.to_string()))?;
        }

        Ok(())
    }

    /// Get the current effective capabilities.
    pub fn current_effective() -> Result<HashSet<Capability>> {
        caps::read(None, CapSet::Effective)
            .map_err(|e| SandboxError::Capability(e.to_string()))
    }

    /// Get the current permitted capabilities.
    pub fn current_permitted() -> Result<HashSet<Capability>> {
        caps::read(None, CapSet::Permitted)
            .map_err(|e| SandboxError::Capability(e.to_string()))
    }

    /// Check if a capability is in the effective set.
    pub fn has_capability(cap: Capability) -> Result<bool> {
        caps::has_cap(None, CapSet::Effective, cap)
            .map_err(|e| SandboxError::Capability(e.to_string()))
    }
}

/// Common dangerous capabilities that should typically be dropped.
pub const DANGEROUS_CAPS: &[Capability] = &[
    Capability::CAP_SYS_ADMIN,
    Capability::CAP_SYS_PTRACE,
    Capability::CAP_SYS_MODULE,
    Capability::CAP_SYS_BOOT,
    Capability::CAP_SYS_RAWIO,
    Capability::CAP_NET_ADMIN,
    Capability::CAP_NET_RAW,
    Capability::CAP_SYS_CHROOT,
    Capability::CAP_MKNOD,
    Capability::CAP_LINUX_IMMUTABLE,
];

/// Check if the current process has any dangerous capabilities.
pub fn has_dangerous_capabilities() -> Result<bool> {
    let effective = CapabilitySet::current_effective()?;
    for cap in DANGEROUS_CAPS {
        if effective.contains(cap) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Drop all dangerous capabilities.
pub fn drop_dangerous_capabilities() -> Result<()> {
    for cap in DANGEROUS_CAPS {
        let _ = caps::drop(None, CapSet::Bounding, *cap);
        let _ = caps::drop(None, CapSet::Effective, *cap);
        let _ = caps::drop(None, CapSet::Permitted, *cap);
        let _ = caps::drop(None, CapSet::Inheritable, *cap);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_set_creation() {
        let caps = CapabilitySet::new();
        assert!(caps.keep.is_empty());
        assert!(caps.drop_all);
    }

    #[test]
    fn test_capability_set_with_caps() {
        let caps = CapabilitySet::with_capabilities([Capability::CAP_CHOWN, Capability::CAP_FOWNER]);

        assert!(caps.keep.contains(&Capability::CAP_CHOWN));
        assert!(caps.keep.contains(&Capability::CAP_FOWNER));
        assert_eq!(caps.keep.len(), 2);
    }

    #[test]
    fn test_standard_capabilities() {
        let caps = CapabilitySet::standard();
        assert!(caps.keep.contains(&Capability::CAP_CHOWN));
        assert!(caps.keep.contains(&Capability::CAP_SETUID));
        assert!(!caps.keep.contains(&Capability::CAP_SYS_ADMIN));
    }

    #[test]
    fn test_builder_pattern() {
        let caps = CapabilitySet::new()
            .keep(Capability::CAP_NET_BIND_SERVICE)
            .keep_all([Capability::CAP_CHOWN, Capability::CAP_FOWNER]);

        assert_eq!(caps.keep.len(), 3);
    }
}
