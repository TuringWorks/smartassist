//! Agent runtime and session management for SmartAssist.
//!
//! This crate provides the core agent execution runtime, including:
//! - Session management and persistence
//! - Tool execution and approval workflows
//! - Model provider integrations
//! - Streaming response handling
//! - Context compression for long conversations

pub mod error;
pub mod runtime;
pub mod session;
pub mod tools;
pub mod providers;
pub mod approval;
pub mod tasks;
pub mod compression;
pub mod skills;

pub use error::AgentError;
pub use runtime::{AgentRuntime, RuntimeConfig};
pub use session::{Session, SessionManager, SessionState};
pub use tools::{GuardrailAction, GuardrailConfig, GuardrailEngine, Tool, ToolContext, ToolExecutor, ToolRegistry};
pub use approval::{ApprovalManager, ApprovalRequest, ApprovalResponse};
pub use compression::{CompressionConfig, CompressionEngine, CompressionResult};
pub use skills::{ImprovementEngine, SkillFragment, SkillSource, Outcome};

/// Result type for agent operations.
pub type Result<T> = std::result::Result<T, AgentError>;
