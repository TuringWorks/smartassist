//! Linux namespace isolation.

#![cfg(target_os = "linux")]

use crate::error::SandboxError;
use crate::Result;
use nix::sched::{unshare, CloneFlags};
use nix::unistd::{Gid, Uid};
use std::path::Path;
use tracing::{debug, warn};

/// Namespace configuration.
#[derive(Debug, Clone, Default)]
pub struct NamespaceConfig {
    /// Create new user namespace.
    pub user: bool,

    /// Create new PID namespace.
    pub pid: bool,

    /// Create new network namespace.
    pub network: bool,

    /// Create new mount namespace.
    pub mount: bool,

    /// Create new UTS namespace (hostname).
    pub uts: bool,

    /// Create new IPC namespace.
    pub ipc: bool,

    /// Create new cgroup namespace.
    pub cgroup: bool,

    /// Map current user to root in user namespace.
    pub map_user_to_root: bool,

    /// Custom hostname for UTS namespace.
    pub hostname: Option<String>,
}

impl NamespaceConfig {
    /// Create a new namespace config with all namespaces disabled.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a minimal namespace config (user + mount).
    pub fn minimal() -> Self {
        Self {
            user: true,
            mount: true,
            map_user_to_root: false,
            ..Default::default()
        }
    }

    /// Create a standard namespace config (user + pid + mount + uts + ipc).
    pub fn standard() -> Self {
        Self {
            user: true,
            pid: true,
            mount: true,
            uts: true,
            ipc: true,
            map_user_to_root: false,
            hostname: Some("sandbox".to_string()),
            ..Default::default()
        }
    }

    /// Create a full isolation config (all namespaces).
    pub fn full_isolation() -> Self {
        Self {
            user: true,
            pid: true,
            network: true,
            mount: true,
            uts: true,
            ipc: true,
            cgroup: true,
            map_user_to_root: true,
            hostname: Some("sandbox".to_string()),
        }
    }

    /// Builder method to enable user namespace.
    pub fn with_user(mut self, enabled: bool) -> Self {
        self.user = enabled;
        self
    }

    /// Builder method to enable PID namespace.
    pub fn with_pid(mut self, enabled: bool) -> Self {
        self.pid = enabled;
        self
    }

    /// Builder method to enable network namespace.
    pub fn with_network(mut self, enabled: bool) -> Self {
        self.network = enabled;
        self
    }

    /// Builder method to enable mount namespace.
    pub fn with_mount(mut self, enabled: bool) -> Self {
        self.mount = enabled;
        self
    }

    /// Builder method to set hostname.
    pub fn with_hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = Some(hostname.into());
        self.uts = true;
        self
    }

    /// Get the clone flags for this configuration.
    pub fn clone_flags(&self) -> CloneFlags {
        let mut flags = CloneFlags::empty();

        if self.user {
            flags |= CloneFlags::CLONE_NEWUSER;
        }
        if self.pid {
            flags |= CloneFlags::CLONE_NEWPID;
        }
        if self.network {
            flags |= CloneFlags::CLONE_NEWNET;
        }
        if self.mount {
            flags |= CloneFlags::CLONE_NEWNS;
        }
        if self.uts {
            flags |= CloneFlags::CLONE_NEWUTS;
        }
        if self.ipc {
            flags |= CloneFlags::CLONE_NEWIPC;
        }
        if self.cgroup {
            flags |= CloneFlags::CLONE_NEWCGROUP;
        }

        flags
    }

    /// Apply namespace isolation to the current process.
    pub fn apply(&self) -> Result<()> {
        let flags = self.clone_flags();

        if flags.is_empty() {
            debug!("No namespaces to create");
            return Ok(());
        }

        // Unshare namespaces
        unshare(flags).map_err(|e| SandboxError::Namespace(e.to_string()))?;

        // Set up user namespace mappings if needed
        if self.user {
            self.setup_user_namespace()?;
        }

        // Set hostname if UTS namespace is used
        if self.uts {
            if let Some(ref hostname) = self.hostname {
                nix::unistd::sethostname(hostname)
                    .map_err(|e| SandboxError::Namespace(e.to_string()))?;
            }
        }

        debug!("Namespace isolation applied: {:?}", flags);
        Ok(())
    }

    /// Set up user namespace UID/GID mappings.
    fn setup_user_namespace(&self) -> Result<()> {
        let uid = Uid::current();
        let gid = Gid::current();

        let (new_uid, new_gid) = if self.map_user_to_root {
            (0, 0)
        } else {
            (uid.as_raw(), gid.as_raw())
        };

        // Write uid_map
        let uid_map = format!("{} {} 1\n", new_uid, uid.as_raw());
        std::fs::write("/proc/self/uid_map", &uid_map)
            .map_err(|e| SandboxError::Namespace(format!("Failed to write uid_map: {}", e)))?;

        // Write "deny" to setgroups before writing gid_map
        std::fs::write("/proc/self/setgroups", "deny\n").ok(); // Ignore errors

        // Write gid_map
        let gid_map = format!("{} {} 1\n", new_gid, gid.as_raw());
        std::fs::write("/proc/self/gid_map", &gid_map)
            .map_err(|e| SandboxError::Namespace(format!("Failed to write gid_map: {}", e)))?;

        debug!(
            "User namespace mappings: {} -> {}, {} -> {}",
            uid.as_raw(),
            new_uid,
            gid.as_raw(),
            new_gid
        );

        Ok(())
    }
}

/// Check if user namespaces are available.
pub fn user_namespaces_available() -> bool {
    // Try to create a user namespace and immediately exit
    Path::new("/proc/sys/kernel/unprivileged_userns_clone")
        .exists()
        .then(|| {
            std::fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone")
                .ok()
                .map(|s| s.trim() == "1")
        })
        .flatten()
        .unwrap_or(true) // Assume available if file doesn't exist
}

/// Set up a private /tmp mount.
pub fn setup_private_tmp() -> Result<()> {
    use nix::mount::{mount, MsFlags};

    mount(
        Some("tmpfs"),
        "/tmp",
        Some("tmpfs"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV,
        Some("size=256M,mode=1777"),
    )
    .map_err(|e| SandboxError::Namespace(format!("Failed to mount private /tmp: {}", e)))?;

    Ok(())
}

/// Set up a minimal /dev mount.
pub fn setup_minimal_dev() -> Result<()> {
    use nix::mount::{mount, MsFlags};
    use nix::sys::stat::{makedev, mknod, Mode, SFlag};

    // Mount tmpfs on /dev
    mount(
        Some("tmpfs"),
        "/dev",
        Some("tmpfs"),
        MsFlags::MS_NOSUID,
        Some("size=64K,mode=755"),
    )
    .map_err(|e| SandboxError::Namespace(format!("Failed to mount /dev: {}", e)))?;

    // Create essential device nodes
    let devices = [
        ("null", 1, 3, 0o666),
        ("zero", 1, 5, 0o666),
        ("random", 1, 8, 0o666),
        ("urandom", 1, 9, 0o666),
        ("tty", 5, 0, 0o666),
    ];

    for (name, major, minor, mode) in devices {
        let path = format!("/dev/{}", name);
        let dev = makedev(major, minor);
        let _ = mknod(
            path.as_str(),
            SFlag::S_IFCHR,
            Mode::from_bits_truncate(mode),
            dev,
        );
    }

    // Create /dev/fd symlink
    let _ = std::os::unix::fs::symlink("/proc/self/fd", "/dev/fd");

    // Create standard streams symlinks
    let _ = std::os::unix::fs::symlink("/proc/self/fd/0", "/dev/stdin");
    let _ = std::os::unix::fs::symlink("/proc/self/fd/1", "/dev/stdout");
    let _ = std::os::unix::fs::symlink("/proc/self/fd/2", "/dev/stderr");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_namespace_config_default() {
        let config = NamespaceConfig::new();
        assert!(!config.user);
        assert!(!config.pid);
        assert!(!config.network);
    }

    #[test]
    fn test_namespace_config_full() {
        let config = NamespaceConfig::full_isolation();
        assert!(config.user);
        assert!(config.pid);
        assert!(config.network);
        assert!(config.mount);
        assert!(config.uts);
        assert!(config.ipc);
        assert!(config.cgroup);
    }

    #[test]
    fn test_clone_flags() {
        let config = NamespaceConfig::standard();
        let flags = config.clone_flags();

        assert!(flags.contains(CloneFlags::CLONE_NEWUSER));
        assert!(flags.contains(CloneFlags::CLONE_NEWPID));
        assert!(flags.contains(CloneFlags::CLONE_NEWNS));
        assert!(flags.contains(CloneFlags::CLONE_NEWUTS));
        assert!(!flags.contains(CloneFlags::CLONE_NEWNET));
    }

    #[test]
    fn test_user_namespaces_check() {
        // Just verify the check doesn't panic
        let _ = user_namespaces_available();
    }
}
