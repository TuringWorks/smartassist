//! Sandbox profile definitions.

use crate::limits::ResourceLimits;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

/// A sandbox profile defining security constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxProfile {
    /// Profile name.
    pub name: String,

    /// Resource limits.
    #[serde(default)]
    pub limits: ResourceLimits,

    /// Filesystem access rules.
    #[serde(default)]
    pub filesystem: FilesystemRules,

    /// Network access rules.
    #[serde(default)]
    pub network: NetworkRules,

    /// System call filter rules.
    #[serde(default)]
    pub syscalls: SyscallRules,

    /// Environment variable rules.
    #[serde(default)]
    pub environment: EnvironmentRules,

    /// Use separate namespaces (Linux).
    #[serde(default)]
    pub use_namespaces: bool,

    /// Drop capabilities (Linux).
    #[serde(default = "default_true")]
    pub drop_capabilities: bool,
}

fn default_true() -> bool {
    true
}

impl Default for SandboxProfile {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            limits: ResourceLimits::default(),
            filesystem: FilesystemRules::default(),
            network: NetworkRules::default(),
            syscalls: SyscallRules::default(),
            environment: EnvironmentRules::default(),
            use_namespaces: false,
            drop_capabilities: true,
        }
    }
}

impl SandboxProfile {
    /// Create a new sandbox profile with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }

    /// Create a minimal (highly restrictive) profile.
    pub fn minimal() -> Self {
        Self {
            name: "minimal".to_string(),
            limits: ResourceLimits::minimal(),
            filesystem: FilesystemRules::read_only(),
            network: NetworkRules::disabled(),
            syscalls: SyscallRules::minimal(),
            environment: EnvironmentRules::minimal(),
            use_namespaces: true,
            drop_capabilities: true,
        }
    }

    /// Create a standard profile for typical agent operations.
    pub fn standard() -> Self {
        Self {
            name: "standard".to_string(),
            limits: ResourceLimits::default(),
            filesystem: FilesystemRules::default(),
            network: NetworkRules::localhost_only(),
            syscalls: SyscallRules::standard(),
            environment: EnvironmentRules::standard(),
            use_namespaces: false,
            drop_capabilities: true,
        }
    }

    /// Create a relaxed profile for trusted operations.
    pub fn relaxed() -> Self {
        Self {
            name: "relaxed".to_string(),
            limits: ResourceLimits::relaxed(),
            filesystem: FilesystemRules::workspace_write(),
            network: NetworkRules::enabled(),
            syscalls: SyscallRules::permissive(),
            environment: EnvironmentRules::permissive(),
            use_namespaces: false,
            drop_capabilities: false,
        }
    }
}

/// Filesystem access rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FilesystemRules {
    /// Paths with read access.
    #[serde(default)]
    pub read_paths: Vec<PathBuf>,

    /// Paths with write access.
    #[serde(default)]
    pub write_paths: Vec<PathBuf>,

    /// Paths with execute access.
    #[serde(default)]
    pub exec_paths: Vec<PathBuf>,

    /// Blocked paths (denied even if parent is allowed).
    #[serde(default)]
    pub blocked_paths: Vec<PathBuf>,

    /// Allow access to /tmp.
    #[serde(default = "default_true")]
    pub allow_tmp: bool,

    /// Allow access to workspace directory.
    #[serde(default = "default_true")]
    pub allow_workspace: bool,
}

impl FilesystemRules {
    /// Create read-only filesystem rules.
    pub fn read_only() -> Self {
        Self {
            read_paths: vec![
                PathBuf::from("/usr"),
                PathBuf::from("/lib"),
                PathBuf::from("/lib64"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
            ],
            write_paths: vec![],
            exec_paths: vec![
                PathBuf::from("/usr/bin"),
                PathBuf::from("/bin"),
            ],
            blocked_paths: vec![
                PathBuf::from("/etc/shadow"),
                PathBuf::from("/etc/passwd"),
            ],
            allow_tmp: false,
            allow_workspace: false,
        }
    }

    /// Create rules that allow workspace writes.
    pub fn workspace_write() -> Self {
        Self {
            read_paths: vec![
                PathBuf::from("/usr"),
                PathBuf::from("/lib"),
                PathBuf::from("/lib64"),
                PathBuf::from("/bin"),
                PathBuf::from("/sbin"),
                PathBuf::from("/etc"),
            ],
            write_paths: vec![], // Workspace added at runtime
            exec_paths: vec![
                PathBuf::from("/usr/bin"),
                PathBuf::from("/bin"),
                PathBuf::from("/usr/local/bin"),
            ],
            blocked_paths: vec![
                PathBuf::from("/etc/shadow"),
            ],
            allow_tmp: true,
            allow_workspace: true,
        }
    }
}

/// Network access rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkRules {
    /// Enable network access.
    #[serde(default)]
    pub enabled: bool,

    /// Allow localhost connections only.
    #[serde(default)]
    pub localhost_only: bool,

    /// Allowed hostnames/IPs.
    #[serde(default)]
    pub allowed_hosts: Vec<String>,

    /// Allowed ports.
    #[serde(default)]
    pub allowed_ports: Vec<u16>,

    /// Blocked ports.
    #[serde(default)]
    pub blocked_ports: Vec<u16>,
}

impl NetworkRules {
    /// Create disabled network rules.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            localhost_only: false,
            allowed_hosts: vec![],
            allowed_ports: vec![],
            blocked_ports: vec![],
        }
    }

    /// Create localhost-only network rules.
    /// Blocks the gateway port (18789) to prevent sandbox-to-gateway privilege escalation.
    pub fn localhost_only() -> Self {
        Self {
            enabled: true,
            localhost_only: true,
            allowed_hosts: vec!["localhost".to_string(), "127.0.0.1".to_string()],
            allowed_ports: vec![],
            blocked_ports: vec![18789], // Block gateway port
        }
    }

    /// Create enabled network rules.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            localhost_only: false,
            allowed_hosts: vec![],
            allowed_ports: vec![],
            blocked_ports: vec![22, 23, 25, 18789], // SSH, Telnet, SMTP, gateway
        }
    }
}

/// System call filter rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyscallRules {
    /// Syscall filtering mode.
    #[serde(default)]
    pub mode: SyscallMode,

    /// Explicitly allowed syscalls (for allowlist mode).
    #[serde(default)]
    pub allowed: HashSet<String>,

    /// Explicitly blocked syscalls (for blocklist mode).
    #[serde(default)]
    pub blocked: HashSet<String>,
}

/// Syscall filtering mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyscallMode {
    /// No filtering.
    #[default]
    Disabled,

    /// Block dangerous syscalls (default list).
    Blocklist,

    /// Allow only specified syscalls.
    Allowlist,
}

impl SyscallRules {
    /// Create minimal syscall rules (allowlist mode).
    pub fn minimal() -> Self {
        let mut allowed = HashSet::new();
        // Basic I/O
        for syscall in &[
            "read", "write", "open", "close", "stat", "fstat", "lstat",
            "poll", "lseek", "mmap", "mprotect", "munmap", "brk",
            "ioctl", "access", "pipe", "select", "dup", "dup2",
            "nanosleep", "getpid", "exit", "exit_group",
        ] {
            allowed.insert((*syscall).to_string());
        }

        Self {
            mode: SyscallMode::Allowlist,
            allowed,
            blocked: HashSet::new(),
        }
    }

    /// Create standard syscall rules (blocklist mode).
    pub fn standard() -> Self {
        let mut blocked = HashSet::new();
        // Block dangerous syscalls
        for syscall in &[
            "ptrace", "process_vm_readv", "process_vm_writev",
            "kexec_load", "kexec_file_load",
            "init_module", "finit_module", "delete_module",
            "reboot", "swapon", "swapoff",
            "mount", "umount", "umount2",
            "pivot_root", "chroot",
            "acct", "settimeofday", "adjtimex",
        ] {
            blocked.insert((*syscall).to_string());
        }

        Self {
            mode: SyscallMode::Blocklist,
            allowed: HashSet::new(),
            blocked,
        }
    }

    /// Create permissive syscall rules (no filtering).
    pub fn permissive() -> Self {
        Self {
            mode: SyscallMode::Disabled,
            allowed: HashSet::new(),
            blocked: HashSet::new(),
        }
    }
}

/// Environment variable rules.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvironmentRules {
    /// Allow inheriting environment from parent.
    #[serde(default)]
    pub inherit: bool,

    /// Allowed environment variables (if not inheriting).
    #[serde(default)]
    pub allowed: HashSet<String>,

    /// Blocked environment variables.
    #[serde(default)]
    pub blocked: HashSet<String>,

    /// Environment variables to set.
    #[serde(default)]
    pub set: std::collections::HashMap<String, String>,
}

impl EnvironmentRules {
    /// Safe PATH for sandboxed execution — prevents PATH manipulation attacks (CVE-2026-24763).
    pub const SAFE_PATH: &'static str = "/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";

    /// Create minimal environment rules.
    /// Overrides PATH with a safe hardcoded value to prevent PATH manipulation.
    pub fn minimal() -> Self {
        let mut allowed = HashSet::new();
        for var in &["HOME", "USER", "SHELL", "TERM", "LANG"] {
            allowed.insert((*var).to_string());
        }

        let mut set = std::collections::HashMap::new();
        set.insert("PATH".to_string(), Self::SAFE_PATH.to_string());

        Self {
            inherit: false,
            allowed,
            blocked: Self::default_blocked(),
            set,
        }
    }

    /// Create standard environment rules.
    /// Inherits most env vars but overrides PATH with a safe value.
    pub fn standard() -> Self {
        let mut set = std::collections::HashMap::new();
        set.insert("PATH".to_string(), Self::SAFE_PATH.to_string());

        Self {
            inherit: true,
            allowed: HashSet::new(),
            blocked: Self::default_blocked(),
            set,
        }
    }

    /// Create permissive environment rules.
    pub fn permissive() -> Self {
        Self {
            inherit: true,
            allowed: HashSet::new(),
            blocked: Self::default_blocked(),
            set: std::collections::HashMap::new(),
        }
    }

    /// Get default blocked environment variables.
    pub fn default_blocked() -> HashSet<String> {
        let mut blocked = HashSet::new();
        for var in &[
            // Dynamic linker injection
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "LD_AUDIT",
            "LD_DEBUG",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_LIBRARY_PATH",
            // Runtime injection
            "NODE_OPTIONS",
            "NODE_PATH",
            "PYTHONSTARTUP",
            "PYTHONPATH",
            "PYTHONHOME",
            "RUBYOPT",
            "RUBYLIB",
            "PERL5OPT",
            "PERL5LIB",
            // Shell injection
            "BASH_ENV",
            "ENV",
            "IFS",
            // Other dangerous
            "GCONV_PATH",
            "SSLKEYLOGFILE",
        ] {
            blocked.insert((*var).to_string());
        }
        blocked
    }
}

/// Builder for creating sandbox profiles.
#[derive(Debug, Default)]
pub struct ProfileBuilder {
    profile: SandboxProfile,
}

impl ProfileBuilder {
    /// Create a new profile builder.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            profile: SandboxProfile::new(name),
        }
    }

    /// Set resource limits.
    pub fn limits(mut self, limits: ResourceLimits) -> Self {
        self.profile.limits = limits;
        self
    }

    /// Set filesystem rules.
    pub fn filesystem(mut self, rules: FilesystemRules) -> Self {
        self.profile.filesystem = rules;
        self
    }

    /// Set network rules.
    pub fn network(mut self, rules: NetworkRules) -> Self {
        self.profile.network = rules;
        self
    }

    /// Set syscall rules.
    pub fn syscalls(mut self, rules: SyscallRules) -> Self {
        self.profile.syscalls = rules;
        self
    }

    /// Set environment rules.
    pub fn environment(mut self, rules: EnvironmentRules) -> Self {
        self.profile.environment = rules;
        self
    }

    /// Enable namespace isolation.
    pub fn with_namespaces(mut self, enabled: bool) -> Self {
        self.profile.use_namespaces = enabled;
        self
    }

    /// Enable capability dropping.
    pub fn drop_capabilities(mut self, enabled: bool) -> Self {
        self.profile.drop_capabilities = enabled;
        self
    }

    /// Build the profile.
    pub fn build(self) -> SandboxProfile {
        self.profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_profile() {
        let profile = SandboxProfile::default();
        assert_eq!(profile.name, "default");
        assert!(profile.drop_capabilities);
    }

    #[test]
    fn test_minimal_profile() {
        let profile = SandboxProfile::minimal();
        assert_eq!(profile.name, "minimal");
        assert!(profile.use_namespaces);
        assert!(!profile.network.enabled);
    }

    #[test]
    fn test_profile_builder() {
        let profile = ProfileBuilder::new("custom")
            .limits(ResourceLimits::minimal())
            .with_namespaces(true)
            .network(NetworkRules::disabled())
            .build();

        assert_eq!(profile.name, "custom");
        assert!(profile.use_namespaces);
        assert!(!profile.network.enabled);
    }

    #[test]
    fn test_environment_blocked() {
        let blocked = EnvironmentRules::default_blocked();
        assert!(blocked.contains("LD_PRELOAD"));
        assert!(blocked.contains("NODE_OPTIONS"));
        assert!(blocked.contains("BASH_ENV"));
        assert!(blocked.contains("IFS"));
    }

    #[test]
    fn test_standard_env_overrides_path() {
        let env = EnvironmentRules::standard();
        assert!(env.set.contains_key("PATH"));
        assert_eq!(env.set["PATH"], EnvironmentRules::SAFE_PATH);
    }

    #[test]
    fn test_minimal_env_sets_safe_path() {
        let env = EnvironmentRules::minimal();
        assert!(env.set.contains_key("PATH"));
        assert!(!env.allowed.contains("PATH")); // PATH comes from set, not inherited
    }

    #[test]
    fn test_localhost_only_blocks_gateway_port() {
        let rules = NetworkRules::localhost_only();
        assert!(rules.blocked_ports.contains(&18789));
    }

    #[test]
    fn test_enabled_network_blocks_gateway_port() {
        let rules = NetworkRules::enabled();
        assert!(rules.blocked_ports.contains(&18789));
    }
}
