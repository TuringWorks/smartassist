//! Encrypted secret management for SmartAssist.
//!
//! Provides AES-256-GCM encrypted storage with OS keychain integration
//! for master key management.

pub mod crypto;
pub mod error;
pub mod keychain;
pub mod store;
pub mod types;

pub use error::{Result, SecretError};
pub use store::{FileSecretStore, SecretStore};
pub use types::{CreateSecretParams, DecryptedSecret, Secret, SecretRef};
