//! # smartassist-core
//!
//! Core types, configuration, and utilities for SmartAssist.
//!
//! This crate provides shared functionality used across all SmartAssist crates:
//!
//! - **Configuration**: Loading, validation, and management of config files
//! - **Types**: Common type definitions for messages, sessions, and agents
//! - **Utilities**: Path resolution, ID generation, and environment handling

pub mod config;
pub mod types;
pub mod error;
pub mod paths;
pub mod env;
pub mod id;
pub mod secret;
pub mod safety;
pub mod context;

// Re-exports for convenience
pub use config::Config;
pub use error::{Error, Result};
pub use types::*;
pub use secret::SecretString;
