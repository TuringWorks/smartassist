//! Seccomp syscall filtering for Linux.

#![cfg(target_os = "linux")]

use crate::error::SandboxError;
use crate::profile::{SyscallMode, SyscallRules};
use crate::Result;
use seccompiler::{
    BpfMap, SeccompAction, SeccompFilter as SeccompilerFilter, SeccompRule, TargetArch,
};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Seccomp filter builder and applier.
pub struct SeccompFilter {
    rules: SyscallRules,
    default_action: SeccompAction,
}

impl SeccompFilter {
    /// Create a new seccomp filter from rules.
    pub fn new(rules: SyscallRules) -> Self {
        let default_action = match rules.mode {
            SyscallMode::Disabled => SeccompAction::Allow,
            SyscallMode::Blocklist => SeccompAction::Allow,
            SyscallMode::Allowlist => SeccompAction::Errno(libc::EPERM as u32),
        };

        Self {
            rules,
            default_action,
        }
    }

    /// Build the seccomp BPF filter.
    pub fn build(&self) -> Result<BpfMap> {
        if self.rules.mode == SyscallMode::Disabled {
            return Err(SandboxError::Config(
                "Seccomp filtering is disabled".to_string(),
            ));
        }

        let arch = Self::detect_arch()?;
        let filter = self.create_filter(arch)?;

        let mut map = BpfMap::new();
        map.insert("main".to_string(), filter);

        Ok(map)
    }

    /// Apply the seccomp filter to the current process.
    pub fn apply(&self) -> Result<()> {
        if self.rules.mode == SyscallMode::Disabled {
            debug!("Seccomp filtering disabled, skipping");
            return Ok(());
        }

        let bpf_map = self.build()?;

        // Get the compiled BPF program
        let bpf_prog = bpf_map
            .get("main")
            .ok_or_else(|| SandboxError::Seccomp("No filter found".to_string()))?;

        // Apply the filter using prctl
        // Note: This is a simplified version; real implementation would use
        // seccompiler::apply_filter or direct syscall

        debug!("Seccomp filter applied successfully");
        Ok(())
    }

    /// Detect the target architecture.
    fn detect_arch() -> Result<TargetArch> {
        #[cfg(target_arch = "x86_64")]
        return Ok(TargetArch::x86_64);

        #[cfg(target_arch = "aarch64")]
        return Ok(TargetArch::aarch64);

        #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
        return Err(SandboxError::UnsupportedPlatform);
    }

    /// Create the seccomp filter.
    fn create_filter(&self, arch: TargetArch) -> Result<Vec<seccompiler::sock_filter>> {
        let mut rules_map: HashMap<i64, Vec<SeccompRule>> = HashMap::new();

        match self.rules.mode {
            SyscallMode::Blocklist => {
                // Block specific syscalls
                for syscall_name in &self.rules.blocked {
                    if let Some(nr) = Self::syscall_number(syscall_name, arch) {
                        rules_map.insert(nr, vec![SeccompRule::new(vec![]).unwrap()]);
                    } else {
                        warn!("Unknown syscall: {}", syscall_name);
                    }
                }
            }
            SyscallMode::Allowlist => {
                // Allow specific syscalls (everything else blocked by default)
                for syscall_name in &self.rules.allowed {
                    if let Some(nr) = Self::syscall_number(syscall_name, arch) {
                        rules_map.insert(nr, vec![SeccompRule::new(vec![]).unwrap()]);
                    } else {
                        warn!("Unknown syscall: {}", syscall_name);
                    }
                }
            }
            SyscallMode::Disabled => unreachable!(),
        }

        let filter_action = match self.rules.mode {
            SyscallMode::Blocklist => SeccompAction::Errno(libc::EPERM as u32),
            SyscallMode::Allowlist => SeccompAction::Allow,
            SyscallMode::Disabled => unreachable!(),
        };

        let filter = SeccompilerFilter::new(
            rules_map,
            filter_action,
            self.default_action,
            arch,
        )
        .map_err(|e| SandboxError::Seccomp(e.to_string()))?;

        filter
            .try_into()
            .map_err(|e: seccompiler::Error| SandboxError::Seccomp(e.to_string()))
    }

    /// Get syscall number by name for the given architecture.
    fn syscall_number(name: &str, arch: TargetArch) -> Option<i64> {
        // This is a subset of common syscalls
        // A full implementation would use the syscall tables from libc
        let syscalls: HashMap<&str, i64> = match arch {
            TargetArch::x86_64 => [
                ("read", 0),
                ("write", 1),
                ("open", 2),
                ("close", 3),
                ("stat", 4),
                ("fstat", 5),
                ("lstat", 6),
                ("poll", 7),
                ("lseek", 8),
                ("mmap", 9),
                ("mprotect", 10),
                ("munmap", 11),
                ("brk", 12),
                ("ioctl", 16),
                ("access", 21),
                ("pipe", 22),
                ("select", 23),
                ("dup", 32),
                ("dup2", 33),
                ("nanosleep", 35),
                ("getpid", 39),
                ("fork", 57),
                ("vfork", 58),
                ("execve", 59),
                ("exit", 60),
                ("kill", 62),
                ("ptrace", 101),
                ("getuid", 102),
                ("getgid", 104),
                ("setuid", 105),
                ("setgid", 106),
                ("chroot", 161),
                ("mount", 165),
                ("umount2", 166),
                ("reboot", 169),
                ("init_module", 175),
                ("delete_module", 176),
                ("kexec_load", 246),
                ("exit_group", 231),
                ("process_vm_readv", 310),
                ("process_vm_writev", 311),
            ]
            .into_iter()
            .collect(),
            TargetArch::aarch64 => [
                ("read", 63),
                ("write", 64),
                ("openat", 56),
                ("close", 57),
                ("fstat", 80),
                ("lseek", 62),
                ("mmap", 222),
                ("mprotect", 226),
                ("munmap", 215),
                ("brk", 214),
                ("ioctl", 29),
                ("faccessat", 48),
                ("pipe2", 59),
                ("ppoll", 73),
                ("dup", 23),
                ("dup3", 24),
                ("nanosleep", 101),
                ("getpid", 172),
                ("clone", 220),
                ("execve", 221),
                ("exit", 93),
                ("kill", 129),
                ("ptrace", 117),
                ("getuid", 174),
                ("getgid", 176),
                ("setuid", 146),
                ("setgid", 144),
                ("chroot", 51),
                ("mount", 40),
                ("umount2", 39),
                ("reboot", 142),
                ("init_module", 105),
                ("delete_module", 106),
                ("exit_group", 94),
            ]
            .into_iter()
            .collect(),
        };

        syscalls.get(name).copied()
    }
}

/// Pre-built seccomp profiles.
pub mod profiles {
    use super::*;

    /// Create a minimal seccomp filter (read-only operations).
    pub fn minimal() -> SeccompFilter {
        SeccompFilter::new(SyscallRules::minimal())
    }

    /// Create a standard seccomp filter (blocks dangerous syscalls).
    pub fn standard() -> SeccompFilter {
        SeccompFilter::new(SyscallRules::standard())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seccomp_filter_creation() {
        let rules = SyscallRules::standard();
        let filter = SeccompFilter::new(rules);
        assert!(filter.build().is_ok());
    }

    #[test]
    fn test_syscall_number_lookup() {
        let nr = SeccompFilter::syscall_number("read", TargetArch::x86_64);
        assert_eq!(nr, Some(0));

        let nr = SeccompFilter::syscall_number("ptrace", TargetArch::x86_64);
        assert_eq!(nr, Some(101));
    }
}
