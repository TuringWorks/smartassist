//! Utility tools for common operations.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};

/// Tool for sleeping/waiting.
pub struct SleepTool;

impl SleepTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SleepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct SleepArgs {
    /// Duration in milliseconds
    #[serde(default)]
    ms: Option<u64>,
    /// Duration in seconds
    #[serde(default)]
    secs: Option<u64>,
}

#[async_trait]
impl Tool for SleepTool {
    fn name(&self) -> &str {
        "sleep"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "sleep".to_string(),
            description: "Wait for a specified duration".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "ms": {
                        "type": "integer",
                        "description": "Duration in milliseconds"
                    },
                    "secs": {
                        "type": "integer",
                        "description": "Duration in seconds"
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
        let args: SleepArgs = serde_json::from_value(args)?;

        // Calculate total duration
        let mut total_ms = args.ms.unwrap_or(0);
        if let Some(secs) = args.secs {
            total_ms += secs * 1000;
        }

        // Cap at 60 seconds to prevent abuse
        let max_ms = 60_000;
        let actual_ms = total_ms.min(max_ms);

        if actual_ms > 0 {
            tokio::time::sleep(Duration::from_millis(actual_ms)).await;
        }

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "slept_ms": actual_ms,
                "requested_ms": total_ms,
                "capped": total_ms > max_ms
            }),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for creating temporary files.
pub struct TempFileTool;

impl TempFileTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TempFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct TempFileArgs {
    /// Content to write to the temp file
    #[serde(default)]
    content: Option<String>,
    /// File extension (without dot)
    #[serde(default)]
    extension: Option<String>,
    /// Prefix for the filename
    #[serde(default)]
    prefix: Option<String>,
}

#[async_trait]
impl Tool for TempFileTool {
    fn name(&self) -> &str {
        "temp_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "temp_file".to_string(),
            description: "Create a temporary file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "Content to write to the file"
                    },
                    "extension": {
                        "type": "string",
                        "description": "File extension (without dot)"
                    },
                    "prefix": {
                        "type": "string",
                        "description": "Prefix for the filename"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: TempFileArgs = serde_json::from_value(args)?;

        let temp_dir = std::env::temp_dir();
        let prefix = args.prefix.unwrap_or_else(|| "smartassist_".to_string());
        let extension = args.extension.unwrap_or_else(|| "tmp".to_string());

        // Generate unique filename
        let unique_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let filename = format!("{}{}.{}", prefix, unique_id, extension);
        let path = temp_dir.join(&filename);

        // Write content if provided
        if let Some(content) = args.content {
            tokio::fs::write(&path, &content)
                .await
                .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to write temp file: {}", e)))?;
        } else {
            tokio::fs::write(&path, "")
                .await
                .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to create temp file: {}", e)))?;
        }

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "path": path.to_string_lossy(),
                "filename": filename
            }),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for creating temporary directories.
pub struct TempDirTool;

impl TempDirTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TempDirTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct TempDirArgs {
    /// Prefix for the directory name
    #[serde(default)]
    prefix: Option<String>,
}

#[async_trait]
impl Tool for TempDirTool {
    fn name(&self) -> &str {
        "temp_dir"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "temp_dir".to_string(),
            description: "Create a temporary directory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prefix": {
                        "type": "string",
                        "description": "Prefix for the directory name"
                    }
                }
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: TempDirArgs = serde_json::from_value(args)?;

        let temp_dir = std::env::temp_dir();
        let prefix = args.prefix.unwrap_or_else(|| "smartassist_".to_string());

        // Generate unique dirname
        let unique_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let dirname = format!("{}{}", prefix, unique_id);
        let path = temp_dir.join(&dirname);

        tokio::fs::create_dir_all(&path)
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to create temp dir: {}", e)))?;

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "path": path.to_string_lossy(),
                "dirname": dirname
            }),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for echoing/returning values (useful for testing).
pub struct EchoTool;

impl EchoTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EchoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct EchoArgs {
    /// Value to echo
    value: serde_json::Value,
}

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &str {
        "echo"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "echo".to_string(),
            description: "Echo a value back (useful for testing)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "value": {
                        "description": "Value to echo back"
                    }
                },
                "required": ["value"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: EchoArgs = serde_json::from_value(args)?;

        Ok(ToolResult::success(
            tool_use_id,
            args.value,
        ).with_duration(start.elapsed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_sleep_short() {
        let tool = SleepTool::new();
        let context = ToolContext::default();

        let start = Instant::now();
        let result = tool.execute(
            "test",
            json!({
                "ms": 50
            }),
            &context,
        ).await.unwrap();

        let elapsed = start.elapsed();
        assert!(!result.is_error);
        assert!(elapsed >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_temp_file() {
        let tool = TempFileTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "content": "test content",
                "extension": "txt"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        let path = output["path"].as_str().unwrap();
        assert!(std::path::Path::new(path).exists());

        // Cleanup
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn test_temp_dir() {
        let tool = TempDirTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "prefix": "test_"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        let path = output["path"].as_str().unwrap();
        assert!(std::path::Path::new(path).is_dir());

        // Cleanup
        let _ = std::fs::remove_dir(path);
    }

    #[tokio::test]
    async fn test_echo() {
        let tool = EchoTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "value": {"hello": "world", "number": 42}
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert_eq!(output["hello"], "world");
        assert_eq!(output["number"], 42);
    }
}
