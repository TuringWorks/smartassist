//! Secure sandbox execution for SmartAssist agents.
//!
//! This crate provides sandboxed command execution with multiple layers of security:
//! - Linux: seccomp syscall filtering, landlock filesystem sandboxing, namespaces
//! - macOS: sandbox-exec profiles (limited)
//! - All platforms: resource limits, environment filtering, PTY isolation

pub mod error;
pub mod executor;
pub mod limits;
pub mod pty;
pub mod profile;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

pub use error::SandboxError;
pub use executor::{CommandExecutor, ExecutionContext, ExecutionOutput};
pub use limits::ResourceLimits;
pub use profile::{SandboxProfile, ProfileBuilder};
pub use pty::{PtySession, PtyConfig};

/// Result type for sandbox operations.
pub type Result<T> = std::result::Result<T, SandboxError>;
