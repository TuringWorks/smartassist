//! Context monitoring and compaction.
//!
//! Provides token estimation, context window usage monitoring, and
//! multi-strategy context compaction for managing conversation history.

pub mod monitor;
pub mod compactor;

pub use monitor::{ContextMonitor, CompactionStrategy};
pub use compactor::{ContextCompactor, CompactionResult};
