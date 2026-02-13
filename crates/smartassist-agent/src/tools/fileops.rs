//! File operation tools (copy, move, stat, permissions).

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::Path;
use std::time::Instant;

/// Tool for copying files.
pub struct FileCopyTool;

impl FileCopyTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileCopyTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct FileCopyArgs {
    /// Source path
    source: String,
    /// Destination path
    destination: String,
    /// Overwrite if destination exists
    #[serde(default)]
    overwrite: Option<bool>,
}

#[async_trait]
impl Tool for FileCopyTool {
    fn name(&self) -> &str {
        "file_copy"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_copy".to_string(),
            description: "Copy a file to a new location".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Source file path"
                    },
                    "destination": {
                        "type": "string",
                        "description": "Destination file path"
                    },
                    "overwrite": {
                        "type": "boolean",
                        "description": "Overwrite if destination exists (default: false)"
                    }
                },
                "required": ["source", "destination"]
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
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: FileCopyArgs = serde_json::from_value(args)?;
        let overwrite = args.overwrite.unwrap_or(false);

        let source = if Path::new(&args.source).is_absolute() {
            std::path::PathBuf::from(&args.source)
        } else {
            context.cwd.join(&args.source)
        };

        let destination = if Path::new(&args.destination).is_absolute() {
            std::path::PathBuf::from(&args.destination)
        } else {
            context.cwd.join(&args.destination)
        };

        if !source.exists() {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("Source file not found: {}", source.display()),
            ));
        }

        if destination.exists() && !overwrite {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("Destination exists: {}. Set overwrite=true to replace.", destination.display()),
            ));
        }

        // Create parent directories if needed
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::copy(&source, &destination)
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Copy failed: {}", e)))?;

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "source": source.to_string_lossy(),
                "destination": destination.to_string_lossy(),
                "copied": true
            }),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for moving/renaming files.
pub struct FileMoveTool;

impl FileMoveTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileMoveTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct FileMoveArgs {
    /// Source path
    source: String,
    /// Destination path
    destination: String,
    /// Overwrite if destination exists
    #[serde(default)]
    overwrite: Option<bool>,
}

#[async_trait]
impl Tool for FileMoveTool {
    fn name(&self) -> &str {
        "file_move"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_move".to_string(),
            description: "Move or rename a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "source": {
                        "type": "string",
                        "description": "Source file path"
                    },
                    "destination": {
                        "type": "string",
                        "description": "Destination file path"
                    },
                    "overwrite": {
                        "type": "boolean",
                        "description": "Overwrite if destination exists (default: false)"
                    }
                },
                "required": ["source", "destination"]
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
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: FileMoveArgs = serde_json::from_value(args)?;
        let overwrite = args.overwrite.unwrap_or(false);

        let source = if Path::new(&args.source).is_absolute() {
            std::path::PathBuf::from(&args.source)
        } else {
            context.cwd.join(&args.source)
        };

        let destination = if Path::new(&args.destination).is_absolute() {
            std::path::PathBuf::from(&args.destination)
        } else {
            context.cwd.join(&args.destination)
        };

        if !source.exists() {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("Source not found: {}", source.display()),
            ));
        }

        if destination.exists() && !overwrite {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("Destination exists: {}. Set overwrite=true to replace.", destination.display()),
            ));
        }

        // Create parent directories if needed
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::rename(&source, &destination)
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Move failed: {}", e)))?;

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "source": source.to_string_lossy(),
                "destination": destination.to_string_lossy(),
                "moved": true
            }),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for getting file/directory information.
pub struct FileStatTool;

impl FileStatTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileStatTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct FileStatArgs {
    /// Path to the file or directory
    path: String,
}

#[derive(Debug, Serialize)]
struct FileStats {
    path: String,
    exists: bool,
    is_file: bool,
    is_dir: bool,
    is_symlink: bool,
    size: Option<u64>,
    readonly: Option<bool>,
    modified: Option<String>,
    created: Option<String>,
}

#[async_trait]
impl Tool for FileStatTool {
    fn name(&self) -> &str {
        "file_stat"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_stat".to_string(),
            description: "Get file or directory information".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file or directory"
                    }
                },
                "required": ["path"]
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
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: FileStatArgs = serde_json::from_value(args)?;

        let path = if Path::new(&args.path).is_absolute() {
            std::path::PathBuf::from(&args.path)
        } else {
            context.cwd.join(&args.path)
        };

        if !path.exists() {
            let stats = FileStats {
                path: path.to_string_lossy().to_string(),
                exists: false,
                is_file: false,
                is_dir: false,
                is_symlink: false,
                size: None,
                readonly: None,
                modified: None,
                created: None,
            };
            return Ok(ToolResult::success(tool_use_id, json!(stats))
                .with_duration(start.elapsed()));
        }

        let metadata = tokio::fs::metadata(&path).await?;
        let symlink_metadata = tokio::fs::symlink_metadata(&path).await.ok();

        let modified = metadata.modified().ok().map(|t| {
            chrono::DateTime::<chrono::Utc>::from(t)
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string()
        });

        let created = metadata.created().ok().map(|t| {
            chrono::DateTime::<chrono::Utc>::from(t)
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string()
        });

        let stats = FileStats {
            path: path.to_string_lossy().to_string(),
            exists: true,
            is_file: metadata.is_file(),
            is_dir: metadata.is_dir(),
            is_symlink: symlink_metadata.map(|m| m.file_type().is_symlink()).unwrap_or(false),
            size: Some(metadata.len()),
            readonly: Some(metadata.permissions().readonly()),
            modified,
            created,
        };

        Ok(ToolResult::success(tool_use_id, json!(stats))
            .with_duration(start.elapsed()))
    }
}

/// Tool for deleting files or directories.
pub struct FileDeleteTool;

impl FileDeleteTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileDeleteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct FileDeleteArgs {
    /// Path to delete
    path: String,
    /// Recursively delete directories
    #[serde(default)]
    recursive: Option<bool>,
}

#[async_trait]
impl Tool for FileDeleteTool {
    fn name(&self) -> &str {
        "file_delete"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_delete".to_string(),
            description: "Delete a file or directory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to delete"
                    },
                    "recursive": {
                        "type": "boolean",
                        "description": "Recursively delete directories (default: false)"
                    }
                },
                "required": ["path"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }

    fn requires_approval(&self, _args: &serde_json::Value) -> bool {
        true // Deletion is destructive
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: FileDeleteArgs = serde_json::from_value(args)?;
        let recursive = args.recursive.unwrap_or(false);

        let path = if Path::new(&args.path).is_absolute() {
            std::path::PathBuf::from(&args.path)
        } else {
            context.cwd.join(&args.path)
        };

        if !path.exists() {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("Path not found: {}", path.display()),
            ));
        }

        let is_dir = path.is_dir();

        if is_dir {
            if recursive {
                tokio::fs::remove_dir_all(&path)
                    .await
                    .map_err(|e| crate::error::AgentError::tool_execution(format!("Delete failed: {}", e)))?;
            } else {
                tokio::fs::remove_dir(&path)
                    .await
                    .map_err(|e| crate::error::AgentError::tool_execution(format!("Delete failed: {}. Use recursive=true for non-empty directories.", e)))?;
            }
        } else {
            tokio::fs::remove_file(&path)
                .await
                .map_err(|e| crate::error::AgentError::tool_execution(format!("Delete failed: {}", e)))?;
        }

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "path": path.to_string_lossy(),
                "deleted": true,
                "was_directory": is_dir
            }),
        ).with_duration(start.elapsed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_copy() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.txt");
        std::fs::write(&source, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = FileCopyTool::new();
        let result = tool.execute(
            "test",
            json!({
                "source": "source.txt",
                "destination": "dest.txt"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        assert!(temp.path().join("dest.txt").exists());
        assert!(temp.path().join("source.txt").exists()); // Original still exists
    }

    #[tokio::test]
    async fn test_file_move() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source.txt");
        std::fs::write(&source, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = FileMoveTool::new();
        let result = tool.execute(
            "test",
            json!({
                "source": "source.txt",
                "destination": "moved.txt"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        assert!(temp.path().join("moved.txt").exists());
        assert!(!temp.path().join("source.txt").exists()); // Original is gone
    }

    #[tokio::test]
    async fn test_file_stat() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("test.txt");
        std::fs::write(&file, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = FileStatTool::new();
        let result = tool.execute(
            "test",
            json!({
                "path": "test.txt"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["exists"].as_bool().unwrap());
        assert!(output["is_file"].as_bool().unwrap());
        assert_eq!(output["size"], 13);
    }

    #[tokio::test]
    async fn test_file_stat_nonexistent() {
        let temp = TempDir::new().unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = FileStatTool::new();
        let result = tool.execute(
            "test",
            json!({
                "path": "nonexistent.txt"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(!output["exists"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_file_delete() {
        let temp = TempDir::new().unwrap();
        let file = temp.path().join("test.txt");
        std::fs::write(&file, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = FileDeleteTool::new();
        let result = tool.execute(
            "test",
            json!({
                "path": "test.txt"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        assert!(!file.exists());
    }
}
