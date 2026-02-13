//! Linux-specific sandbox implementation.
//!
//! This module provides Linux-specific sandboxing using:
//! - seccomp: System call filtering
//! - landlock: Filesystem sandboxing
//! - namespaces: Process/network/mount isolation
//! - capabilities: Privilege dropping

#[cfg(target_os = "linux")]
pub mod seccomp;

#[cfg(target_os = "linux")]
pub mod landlock;

#[cfg(target_os = "linux")]
pub mod namespace;

#[cfg(target_os = "linux")]
pub mod capabilities;

#[cfg(target_os = "linux")]
pub use self::seccomp::SeccompFilter;

#[cfg(target_os = "linux")]
pub use self::landlock::LandlockRuleset;

#[cfg(target_os = "linux")]
pub use self::namespace::NamespaceConfig;

#[cfg(target_os = "linux")]
pub use self::capabilities::CapabilitySet;
