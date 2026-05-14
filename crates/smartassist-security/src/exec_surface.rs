//! Execution surface audit.
//!
//! Scans for dangerous executables, world-writable scripts, and
//! overly permissive file permissions in the SmartAssist environment.

use crate::audit::{AuditFinding, AuditReport, AuditRunner, Severity};
use walkdir::WalkDir;

/// Exec surface auditor.
pub struct ExecSurfaceAuditor {
    /// Directories to scan.
    scan_paths: Vec<std::path::PathBuf>,
}

impl ExecSurfaceAuditor {
    /// Create a new exec surface auditor.
    pub fn new(scan_paths: Vec<std::path::PathBuf>) -> Self {
        Self { scan_paths }
    }

    /// Scan for world-writable executable files.
    fn scan_world_writable(&self, report: &mut AuditReport) {
        for base in &self.scan_paths {
            for entry in WalkDir::new(base).max_depth(3).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Ok(meta) = std::fs::metadata(path) {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mode = meta.permissions().mode();
                        let world_writable = mode & 0o002 != 0;
                        let is_file = meta.is_file();
                        if is_file && world_writable {
                            report.add(
                                AuditFinding::new(
                                    "EXEC-001",
                                    "World-writable executable",
                                    Severity::High,
                                )
                                .with_description(format!(
                                    "File {} is world-writable (mode {:o})",
                                    path.display(),
                                    mode & 0o777
                                ))
                                .with_path(path)
                                .with_remediation("chmod o-w on the file"),
                            );
                        }
                    }
                }
            }
        }
    }

    /// Scan for SUID/SGID binaries.
    fn scan_setuid(&self, report: &mut AuditReport) {
        for base in &self.scan_paths {
            for entry in WalkDir::new(base).max_depth(3).into_iter().filter_map(|e| e.ok()) {
                let path = entry.path();
                if let Ok(meta) = std::fs::metadata(path) {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        let mode = meta.permissions().mode();
                        let setuid = mode & 0o4000 != 0;
                        let setgid = mode & 0o2000 != 0;
                        if meta.is_file() && (setuid || setgid) {
                            report.add(
                                AuditFinding::new(
                                    "EXEC-002",
                                    "SUID/SGID binary detected",
                                    Severity::Medium,
                                )
                                .with_description(format!(
                                    "{} has setuid/setgid bits set (mode {:o})",
                                    path.display(),
                                    mode & 0o777
                                ))
                                .with_path(path)
                                .with_remediation("Review if SUID/SGID is necessary; remove if not"),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[async_trait::async_trait]
impl AuditRunner for ExecSurfaceAuditor {
    async fn run(&self) -> anyhow::Result<AuditReport> {
        let mut report = AuditReport::new("exec_surface");
        self.scan_world_writable(&mut report);
        self.scan_setuid(&mut report);
        report.compute_summary();
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_exec_surface_audit() {
        let dir = TempDir::new().unwrap();
        let auditor = ExecSurfaceAuditor::new(vec![dir.path().to_path_buf()]);
        let report = auditor.run().await.unwrap();

        // No files, so no findings expected
        assert_eq!(report.total(), 0);
    }
}
