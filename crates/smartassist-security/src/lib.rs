//! SmartAssist Security Audit Framework.
//!
//! Provides security scanning and audit capabilities:
//! - DM policy validation
//! - Allowlist resolution
//! - Execution surface audit
//! - Configuration symlink audit

pub mod allowlist;
pub mod audit;
pub mod config_audit;
pub mod dm_policy;
pub mod exec_surface;

pub use allowlist::AllowlistResolver;
pub use audit::{AuditFinding, AuditReport, AuditRunner, Severity};
pub use config_audit::ConfigSymlinkAuditor;
pub use dm_policy::DmPolicyAuditor;
pub use exec_surface::ExecSurfaceAuditor;
