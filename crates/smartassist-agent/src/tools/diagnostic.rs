//! Diagnostic tools for debugging and system information.
//!
//! Provides tools for gathering system information, checking health,
//! and diagnosing issues.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Tool for getting system information.
pub struct SystemInfoTool;

impl SystemInfoTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemInfoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SystemInfoTool {
    fn name(&self) -> &str {
        "system_info"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "system_info".to_string(),
            description: "Get system information including OS, architecture, and environment."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        _args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        let family = std::env::consts::FAMILY;

        let home_dir = dirs::home_dir().map(|p| p.to_string_lossy().to_string());
        let current_dir = ctx.cwd.to_string_lossy().to_string();

        // Get some environment info
        let path = std::env::var("PATH").ok();
        let shell = std::env::var("SHELL").ok();
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .ok();

        let duration = start.elapsed();

        debug!("System info gathered: os={}, arch={}", os, arch);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "os": os,
                "arch": arch,
                "family": family,
                "home_dir": home_dir,
                "current_dir": current_dir,
                "user": user,
                "shell": shell,
                "path": path,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for checking agent health and status.
pub struct HealthCheckTool;

impl HealthCheckTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HealthCheckTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HealthCheckTool {
    fn name(&self) -> &str {
        "health_check"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "health_check".to_string(),
            description: "Check the health and status of the agent and its components."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "include_env": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include environment variable check"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let include_env = args
            .get("include_env")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut checks: Vec<serde_json::Value> = Vec::new();

        // Check working directory
        let cwd_exists = ctx.cwd.exists();
        checks.push(serde_json::json!({
            "name": "working_directory",
            "status": if cwd_exists { "ok" } else { "error" },
            "message": if cwd_exists {
                format!("Working directory exists: {}", ctx.cwd.display())
            } else {
                format!("Working directory does not exist: {}", ctx.cwd.display())
            }
        }));

        // Check if we can write to temp
        let temp_dir = std::env::temp_dir();
        let temp_writable = temp_dir.exists() && temp_dir.is_dir();
        checks.push(serde_json::json!({
            "name": "temp_directory",
            "status": if temp_writable { "ok" } else { "warning" },
            "message": format!("Temp directory: {}", temp_dir.display())
        }));

        // Check environment variables if requested
        if include_env {
            let required_vars = ["PATH", "HOME"];
            for var in required_vars {
                let exists = std::env::var(var).is_ok();
                checks.push(serde_json::json!({
                    "name": format!("env_{}", var.to_lowercase()),
                    "status": if exists { "ok" } else { "warning" },
                    "message": if exists {
                        format!("${} is set", var)
                    } else {
                        format!("${} is not set", var)
                    }
                }));
            }
        }

        // Overall status
        let all_ok = checks
            .iter()
            .all(|c| c.get("status").and_then(|s| s.as_str()) != Some("error"));

        let duration = start.elapsed();

        debug!("Health check complete: {} checks, all_ok={}", checks.len(), all_ok);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "status": if all_ok { "healthy" } else { "unhealthy" },
                "checks": checks,
                "check_count": checks.len(),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for running diagnostics.
pub struct DiagnosticTool;

impl DiagnosticTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DiagnosticTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DiagnosticTool {
    fn name(&self) -> &str {
        "diagnostic"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "diagnostic".to_string(),
            description: "Run diagnostics to identify and troubleshoot issues.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "category": {
                        "type": "string",
                        "enum": ["all", "filesystem", "network", "environment"],
                        "default": "all",
                        "description": "Category of diagnostics to run"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let category = args
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("all");

        let mut diagnostics: Vec<serde_json::Value> = Vec::new();

        // Filesystem diagnostics
        if category == "all" || category == "filesystem" {
            // Check current directory
            diagnostics.push(serde_json::json!({
                "category": "filesystem",
                "name": "cwd",
                "value": ctx.cwd.to_string_lossy(),
                "exists": ctx.cwd.exists(),
            }));

            // Check home directory
            if let Some(home) = dirs::home_dir() {
                diagnostics.push(serde_json::json!({
                    "category": "filesystem",
                    "name": "home",
                    "value": home.to_string_lossy(),
                    "exists": home.exists(),
                }));
            }

            // Check config directory
            if let Some(config) = dirs::config_dir() {
                diagnostics.push(serde_json::json!({
                    "category": "filesystem",
                    "name": "config",
                    "value": config.to_string_lossy(),
                    "exists": config.exists(),
                }));
            }
        }

        // Environment diagnostics
        if category == "all" || category == "environment" {
            let important_vars = [
                "PATH", "HOME", "USER", "SHELL", "TERM", "LANG",
                "ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GOOGLE_API_KEY",
            ];

            for var in important_vars {
                let value = std::env::var(var).ok();
                let is_secret = var.contains("KEY") || var.contains("TOKEN") || var.contains("SECRET");

                diagnostics.push(serde_json::json!({
                    "category": "environment",
                    "name": var,
                    "set": value.is_some(),
                    "value": if is_secret {
                        value.map(|_| "[REDACTED]".to_string())
                    } else {
                        value.map(|v| if v.len() > 100 { format!("{}...", &v[..100]) } else { v })
                    },
                }));
            }
        }

        let duration = start.elapsed();

        debug!("Diagnostics complete: {} items for category '{}'", diagnostics.len(), category);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "category": category,
                "diagnostics": diagnostics,
                "count": diagnostics.len(),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_info_tool_creation() {
        let tool = SystemInfoTool::new();
        assert_eq!(tool.name(), "system_info");
    }

    #[test]
    fn test_health_check_tool_creation() {
        let tool = HealthCheckTool::new();
        assert_eq!(tool.name(), "health_check");
    }

    #[test]
    fn test_diagnostic_tool_creation() {
        let tool = DiagnosticTool::new();
        assert_eq!(tool.name(), "diagnostic");
    }

    #[tokio::test]
    async fn test_system_info_execute() {
        let tool = SystemInfoTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);

        // Check that we got OS info in the output
        assert!(result.output.get("os").is_some());
        assert!(result.output.get("arch").is_some());
    }

    #[tokio::test]
    async fn test_health_check_execute() {
        let tool = HealthCheckTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);

        assert!(result.output.get("status").is_some());
        assert!(result.output.get("checks").is_some());
    }

    #[tokio::test]
    async fn test_diagnostic_execute() {
        let tool = DiagnosticTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute("test_id", serde_json::json!({"category": "environment"}), &ctx)
            .await
            .unwrap();
        assert!(!result.is_error);

        assert_eq!(result.output.get("category").and_then(|v| v.as_str()), Some("environment"));
    }
}
