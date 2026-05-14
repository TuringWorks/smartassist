//! Detached task runtime for SmartAssist.
//!
//! Provides SQLite-backed task persistence, execution policies, and maintenance audits.
//! Tasks can survive agent restarts and be audited for completion status.

pub mod executor;
pub mod registry;

pub use executor::{TaskExecutor, TaskPolicy};
pub use registry::{TaskEntry, TaskRegistry, TaskStatus};
