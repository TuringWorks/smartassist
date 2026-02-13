//! Resource limits for sandboxed processes.

use serde::{Deserialize, Serialize};

/// Resource limits for a sandboxed process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU time in seconds.
    #[serde(default = "default_cpu_time")]
    pub cpu_time_secs: u64,

    /// Maximum wall clock time in seconds.
    #[serde(default = "default_wall_time")]
    pub wall_time_secs: u64,

    /// Maximum memory in bytes.
    #[serde(default = "default_memory")]
    pub memory_bytes: u64,

    /// Maximum file size in bytes.
    #[serde(default = "default_file_size")]
    pub file_size_bytes: u64,

    /// Maximum number of open files.
    #[serde(default = "default_open_files")]
    pub open_files: u64,

    /// Maximum number of processes.
    #[serde(default = "default_processes")]
    pub processes: u64,

    /// Maximum output size in bytes.
    #[serde(default = "default_output_size")]
    pub output_size_bytes: u64,

    /// Enable network access.
    #[serde(default)]
    pub network_enabled: bool,
}

fn default_cpu_time() -> u64 {
    120 // 2 minutes
}

fn default_wall_time() -> u64 {
    300 // 5 minutes
}

fn default_memory() -> u64 {
    512 * 1024 * 1024 // 512 MB
}

fn default_file_size() -> u64 {
    100 * 1024 * 1024 // 100 MB
}

fn default_open_files() -> u64 {
    256
}

fn default_processes() -> u64 {
    64
}

fn default_output_size() -> u64 {
    10 * 1024 * 1024 // 10 MB
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_time_secs: default_cpu_time(),
            wall_time_secs: default_wall_time(),
            memory_bytes: default_memory(),
            file_size_bytes: default_file_size(),
            open_files: default_open_files(),
            processes: default_processes(),
            output_size_bytes: default_output_size(),
            network_enabled: false,
        }
    }
}

impl ResourceLimits {
    /// Create new resource limits with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create minimal (restrictive) resource limits.
    pub fn minimal() -> Self {
        Self {
            cpu_time_secs: 10,
            wall_time_secs: 30,
            memory_bytes: 64 * 1024 * 1024, // 64 MB
            file_size_bytes: 1024 * 1024,   // 1 MB
            open_files: 32,
            processes: 4,
            output_size_bytes: 1024 * 1024, // 1 MB
            network_enabled: false,
        }
    }

    /// Create relaxed resource limits for trusted operations.
    pub fn relaxed() -> Self {
        Self {
            cpu_time_secs: 600,              // 10 minutes
            wall_time_secs: 1800,            // 30 minutes
            memory_bytes: 2 * 1024 * 1024 * 1024, // 2 GB
            file_size_bytes: 1024 * 1024 * 1024,  // 1 GB
            open_files: 1024,
            processes: 256,
            output_size_bytes: 100 * 1024 * 1024, // 100 MB
            network_enabled: true,
        }
    }

    /// Builder-style method to set CPU time limit.
    pub fn with_cpu_time(mut self, secs: u64) -> Self {
        self.cpu_time_secs = secs;
        self
    }

    /// Builder-style method to set wall time limit.
    pub fn with_wall_time(mut self, secs: u64) -> Self {
        self.wall_time_secs = secs;
        self
    }

    /// Builder-style method to set memory limit.
    pub fn with_memory(mut self, bytes: u64) -> Self {
        self.memory_bytes = bytes;
        self
    }

    /// Builder-style method to enable/disable network.
    pub fn with_network(mut self, enabled: bool) -> Self {
        self.network_enabled = enabled;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_limits() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.cpu_time_secs, 120);
        assert_eq!(limits.wall_time_secs, 300);
        assert!(!limits.network_enabled);
    }

    #[test]
    fn test_minimal_limits() {
        let limits = ResourceLimits::minimal();
        assert!(limits.cpu_time_secs < ResourceLimits::default().cpu_time_secs);
        assert!(limits.memory_bytes < ResourceLimits::default().memory_bytes);
    }

    #[test]
    fn test_builder() {
        let limits = ResourceLimits::new()
            .with_cpu_time(60)
            .with_memory(1024 * 1024 * 1024)
            .with_network(true);

        assert_eq!(limits.cpu_time_secs, 60);
        assert_eq!(limits.memory_bytes, 1024 * 1024 * 1024);
        assert!(limits.network_enabled);
    }
}
