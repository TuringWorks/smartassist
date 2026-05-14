//! Core audit framework types and runner.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Severity of an audit finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Informational only.
    Info,
    /// Low risk — best practice deviation.
    Low,
    /// Medium risk — potential security issue.
    Medium,
    /// High risk — likely exploitable.
    High,
    /// Critical risk — immediate action required.
    Critical,
}

/// A single audit finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    /// Audit rule ID.
    pub rule_id: String,
    /// Human-readable title.
    pub title: String,
    /// Detailed description.
    pub description: String,
    /// Severity level.
    pub severity: Severity,
    /// Affected file or resource path.
    pub path: Option<PathBuf>,
    /// Suggested remediation.
    pub remediation: Option<String>,
    /// Additional metadata.
    pub metadata: Option<serde_json::Value>,
}

impl AuditFinding {
    /// Create a new finding.
    pub fn new(rule_id: impl Into<String>, title: impl Into<String>, severity: Severity) -> Self {
        Self {
            rule_id: rule_id.into(),
            title: title.into(),
            description: String::new(),
            severity,
            path: None,
            remediation: None,
            metadata: None,
        }
    }

    /// Set description.
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }

    /// Set affected path.
    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    /// Set remediation.
    pub fn with_remediation(mut self, rem: impl Into<String>) -> Self {
        self.remediation = Some(rem.into());
        self
    }
}

/// Complete audit report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditReport {
    /// Audit name.
    pub audit_name: String,
    /// When the audit ran.
    pub timestamp: DateTime<Utc>,
    /// All findings.
    pub findings: Vec<AuditFinding>,
    /// Summary counts by severity.
    pub summary: serde_json::Value,
}

impl AuditReport {
    /// Create a new empty report.
    pub fn new(audit_name: impl Into<String>) -> Self {
        Self {
            audit_name: audit_name.into(),
            timestamp: Utc::now(),
            findings: Vec::new(),
            summary: serde_json::json!({}),
        }
    }

    /// Add a finding.
    pub fn add(&mut self, finding: AuditFinding) {
        self.findings.push(finding);
    }

    /// Compute summary counts.
    pub fn compute_summary(&mut self) {
        let mut counts = serde_json::Map::new();
        for sev in [Severity::Info, Severity::Low, Severity::Medium, Severity::High, Severity::Critical] {
            let count = self.findings.iter().filter(|f| f.severity == sev).count();
            counts.insert(format!("{:?}", sev).to_lowercase(), serde_json::json!(count));
        }
        self.summary = serde_json::Value::Object(counts);
    }

    /// Total number of findings.
    pub fn total(&self) -> usize {
        self.findings.len()
    }

    /// Number of findings at or above a severity.
    pub fn count_at_or_above(&self, min_severity: Severity) -> usize {
        self.findings.iter().filter(|f| severity_rank(f.severity) >= severity_rank(min_severity)).count()
    }
}

fn severity_rank(sev: Severity) -> u8 {
    match sev {
        Severity::Info => 0,
        Severity::Low => 1,
        Severity::Medium => 2,
        Severity::High => 3,
        Severity::Critical => 4,
    }
}

/// Trait for audit modules.
#[async_trait::async_trait]
pub trait AuditRunner: Send + Sync {
    /// Run the audit and return findings.
    async fn run(&self) -> anyhow::Result<AuditReport>;
}

/// Composite runner that runs multiple audits.
pub struct CompositeAuditRunner {
    runners: Vec<Box<dyn AuditRunner>>,
}

impl CompositeAuditRunner {
    /// Create a new composite runner.
    pub fn new() -> Self {
        Self { runners: Vec::new() }
    }

    /// Add an audit module.
    pub fn add(&mut self, runner: Box<dyn AuditRunner>) {
        self.runners.push(runner);
    }

    /// Run all audits and merge findings.
    pub async fn run_all(&self) -> anyhow::Result<AuditReport> {
        let mut report = AuditReport::new("composite");
        for runner in &self.runners {
            let sub = runner.run().await?;
            report.findings.extend(sub.findings);
        }
        report.compute_summary();
        Ok(report)
    }
}

impl Default for CompositeAuditRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finding_builder() {
        let finding = AuditFinding::new("RULE-1", "Test Finding", Severity::High)
            .with_description("Something is wrong")
            .with_path("/etc/passwd")
            .with_remediation("Fix it");

        assert_eq!(finding.rule_id, "RULE-1");
        assert_eq!(finding.severity, Severity::High);
        assert!(finding.path.is_some());
    }

    #[test]
    fn test_report_summary() {
        let mut report = AuditReport::new("test");
        report.add(AuditFinding::new("A", "A", Severity::High));
        report.add(AuditFinding::new("B", "B", Severity::Low));
        report.add(AuditFinding::new("C", "C", Severity::High));
        report.compute_summary();

        assert_eq!(report.total(), 3);
        assert_eq!(report.count_at_or_above(Severity::High), 2);
        assert_eq!(report.summary["high"], 2);
        assert_eq!(report.summary["low"], 1);
    }
}
