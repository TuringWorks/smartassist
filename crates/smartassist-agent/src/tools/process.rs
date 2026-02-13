//! Process management tools.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Instant;

/// Tool for listing running processes.
pub struct ProcessListTool;

impl ProcessListTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct ProcessListArgs {
    /// Filter by process name (substring match)
    #[serde(default)]
    name: Option<String>,
    /// Maximum number of processes to return
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
struct ProcessInfo {
    pid: u32,
    name: String,
    command: Option<String>,
}

#[async_trait]
impl Tool for ProcessListTool {
    fn name(&self) -> &str {
        "process_list"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "process_list".to_string(),
            description: "List running processes".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Filter by process name (substring match)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of processes to return"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::System
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: ProcessListArgs = serde_json::from_value(args)?;

        // Use ps command to list processes (cross-platform approach)
        let output = tokio::process::Command::new("ps")
            .args(["-eo", "pid,comm"])
            .output()
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to run ps: {}", e)))?;

        if !output.status.success() {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("ps command failed: {}", String::from_utf8_lossy(&output.stderr)),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut processes: Vec<ProcessInfo> = stdout
            .lines()
            .skip(1) // Skip header
            .filter_map(|line| {
                let parts: Vec<&str> = line.trim().splitn(2, char::is_whitespace).collect();
                if parts.len() >= 2 {
                    let pid = parts[0].trim().parse::<u32>().ok()?;
                    let name = parts[1].trim().to_string();
                    Some(ProcessInfo {
                        pid,
                        name,
                        command: None,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Filter by name if provided
        if let Some(ref filter) = args.name {
            let filter_lower = filter.to_lowercase();
            processes.retain(|p| p.name.to_lowercase().contains(&filter_lower));
        }

        // Apply limit
        if let Some(limit) = args.limit {
            processes.truncate(limit);
        }

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "count": processes.len(),
                "processes": processes
            }),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for getting current process information.
pub struct ProcessInfoTool;

impl ProcessInfoTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ProcessInfoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ProcessInfoTool {
    fn name(&self) -> &str {
        "process_info"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "process_info".to_string(),
            description: "Get information about the current process".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {}
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::System
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        _args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let pid = std::process::id();
        let cwd = context.cwd.to_string_lossy().to_string();

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "pid": pid,
                "cwd": cwd,
                "platform": std::env::consts::OS,
                "arch": std::env::consts::ARCH
            }),
        ).with_duration(start.elapsed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_list() {
        let tool = ProcessListTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "limit": 10
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_process_info() {
        let tool = ProcessInfoTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({}),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["pid"].as_u64().is_some());
    }
}
