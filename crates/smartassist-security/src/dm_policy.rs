//! DM (Direct Message) policy auditor.
//!
//! Checks that DM channels are properly restricted and
//! sensitive operations require approval.

use crate::audit::{AuditFinding, AuditReport, AuditRunner, Severity};
use std::path::Path;

/// DM policy auditor.
pub struct DmPolicyAuditor {
    /// Path to configuration directory.
    config_path: std::path::PathBuf,
}

impl DmPolicyAuditor {
    /// Create a new DM policy auditor.
    pub fn new(config_path: impl AsRef<Path>) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
        }
    }

    /// Check if DM approval is configured.
    fn check_dm_approval(&self, report: &mut AuditReport) {
        // In a real implementation, this would read config files.
        // For now, simulate a check.
        let config_file = self.config_path.join("channels.yaml");
        if !config_file.exists() {
            report.add(
                AuditFinding::new(
                    "DM-001",
                    "DM approval policy not configured",
                    Severity::Medium,
                )
                .with_description(
                    "No channels.yaml configuration found. DM channels may allow unrestricted access.",
                )
                .with_remediation("Create channels.yaml with dm.require_approval: true"),
            );
            return;
        }

        // Simulated: config exists but dm approval is disabled
        report.add(
            AuditFinding::new("DM-002", "DM approval disabled", Severity::High)
                .with_description("Direct message channels do not require approval for sensitive operations.")
                .with_remediation("Set dm.require_approval to true in channels.yaml"),
        );
    }

    /// Check admin scope restrictions.
    fn check_admin_scope(&self, report: &mut AuditReport) {
        report.add(
            AuditFinding::new("DM-003", "Admin scope not restricted to DMs", Severity::Medium)
                .with_description("Admin methods may be accessible outside of DM contexts.")
                .with_remediation("Restrict admin scope to direct_message context in gateway config"),
        );
    }
}

#[async_trait::async_trait]
impl AuditRunner for DmPolicyAuditor {
    async fn run(&self) -> anyhow::Result<AuditReport> {
        let mut report = AuditReport::new("dm_policy");
        self.check_dm_approval(&mut report);
        self.check_admin_scope(&mut report);
        report.compute_summary();
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_dm_policy_audit_no_config() {
        let dir = TempDir::new().unwrap();
        let auditor = DmPolicyAuditor::new(dir.path());
        let report = auditor.run().await.unwrap();

        assert!(report.total() > 0);
        assert!(report.findings.iter().any(|f| f.rule_id == "DM-001"));
    }

    #[tokio::test]
    async fn test_dm_policy_audit_with_config() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("channels.yaml"), "{}").unwrap();

        let auditor = DmPolicyAuditor::new(dir.path());
        let report = auditor.run().await.unwrap();

        assert!(report.findings.iter().any(|f| f.rule_id == "DM-002"));
    }
}
