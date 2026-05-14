//! Configuration symlink audit.
//!
//! Detects suspicious symlinks in configuration paths that could
//! lead to path traversal or unauthorized file access.

use crate::audit::{AuditFinding, AuditReport, AuditRunner, Severity};
use std::path::Path;
use walkdir::WalkDir;

/// Config symlink auditor.
pub struct ConfigSymlinkAuditor {
    /// Configuration directory to audit.
    config_dir: std::path::PathBuf,
}

impl ConfigSymlinkAuditor {
    /// Create a new config symlink auditor.
    pub fn new(config_dir: impl AsRef<Path>) -> Self {
        Self {
            config_dir: config_dir.as_ref().to_path_buf(),
        }
    }

    /// Scan for symlinks pointing outside the config directory.
    fn scan_external_symlinks(&self, report: &mut AuditReport) {
        let canonical_base = match std::fs::canonicalize(&self.config_dir) {
            Ok(p) => p,
            Err(_) => return,
        };

        for entry in WalkDir::new(&self.config_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if entry.file_type().is_symlink() {
                if let Ok(target) = std::fs::read_link(path) {
                    let combined = if target.is_absolute() {
                        target.clone()
                    } else {
                        path.parent().unwrap_or(Path::new("")).join(&target)
                    };

                    if let Ok(canonical_target) = std::fs::canonicalize(&combined) {
                        if !canonical_target.starts_with(&canonical_base) {
                            report.add(
                                AuditFinding::new(
                                    "CFG-001",
                                    "Symlink escapes config directory",
                                    Severity::High,
                                )
                                .with_description(format!(
                                    "{} points to {} which is outside the config directory",
                                    path.display(),
                                    canonical_target.display()
                                ))
                                .with_path(path)
                                .with_remediation("Remove or restrict the symlink"),
                            );
                        }
                    } else {
                        report.add(
                            AuditFinding::new(
                                "CFG-002",
                                "Broken or dangling symlink",
                                Severity::Medium,
                            )
                            .with_description(format!(
                                "{} points to {} which cannot be resolved",
                                path.display(),
                                target.display()
                            ))
                            .with_path(path)
                            .with_remediation("Remove or fix the symlink"),
                        );
                    }
                }
            }
        }
    }

    /// Scan for symlinks to sensitive system files.
    fn scan_sensitive_targets(&self, report: &mut AuditReport) {
        let sensitive = [
            "/etc/passwd",
            "/etc/shadow",
            "/etc/hosts",
            "/.ssh",
            "/.aws",
        ];

        for entry in WalkDir::new(&self.config_dir)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if entry.file_type().is_symlink() {
                if let Ok(target) = std::fs::read_link(path) {
                    let target_str = target.to_string_lossy();
                    for sens in &sensitive {
                        if target_str.contains(sens) {
                            report.add(
                                AuditFinding::new(
                                    "CFG-003",
                                    "Symlink targets sensitive file",
                                    Severity::Critical,
                                )
                                .with_description(format!(
                                    "{} points to sensitive path {}",
                                    path.display(),
                                    target.display()
                                ))
                                .with_path(path)
                                .with_remediation("Remove the symlink immediately"),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl AuditRunner for ConfigSymlinkAuditor {
    async fn run(&self) -> anyhow::Result<AuditReport> {
        let mut report = AuditReport::new("config_symlinks");
        self.scan_external_symlinks(&mut report);
        self.scan_sensitive_targets(&mut report);
        report.compute_summary();
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_config_symlink_audit_clean() {
        let dir = TempDir::new().unwrap();
        let auditor = ConfigSymlinkAuditor::new(dir.path());
        let report = auditor.run().await.unwrap();
        assert_eq!(report.total(), 0);
    }

    #[tokio::test]
    async fn test_config_symlink_audit_broken() {
        let dir = TempDir::new().unwrap();
        let link = dir.path().join("broken_link");
        #[cfg(unix)]
        std::os::unix::fs::symlink("/nonexistent/path", &link).unwrap();

        let auditor = ConfigSymlinkAuditor::new(dir.path());
        let report = auditor.run().await.unwrap();

        #[cfg(unix)]
        assert!(report.findings.iter().any(|f| f.rule_id == "CFG-002"));
    }
}
