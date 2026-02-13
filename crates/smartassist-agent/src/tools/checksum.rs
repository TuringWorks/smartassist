//! File checksum and integrity tools.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::time::Instant;

/// Tool for computing file checksums.
pub struct FileChecksumTool;

impl FileChecksumTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileChecksumTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct FileChecksumArgs {
    /// Path to the file
    path: String,
    /// Hash algorithm
    #[serde(default)]
    algorithm: Option<String>,
}

#[async_trait]
impl Tool for FileChecksumTool {
    fn name(&self) -> &str {
        "file_checksum"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_checksum".to_string(),
            description: "Compute checksum/hash of a file".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file"
                    },
                    "algorithm": {
                        "type": "string",
                        "enum": ["md5", "sha1", "sha256", "sha512"],
                        "description": "Hash algorithm (default: sha256)"
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
        let args: FileChecksumArgs = serde_json::from_value(args)?;
        let algorithm = args.algorithm.unwrap_or_else(|| "sha256".to_string());

        let file_path = if Path::new(&args.path).is_absolute() {
            std::path::PathBuf::from(&args.path)
        } else {
            context.cwd.join(&args.path)
        };

        if !file_path.exists() {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("File not found: {}", file_path.display()),
            ));
        }

        let content = tokio::fs::read(&file_path)
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to read file: {}", e)))?;

        let hash = match algorithm.to_lowercase().as_str() {
            "md5" => {
                let digest = md5::compute(&content);
                hex::encode(digest.0)
            }
            "sha1" => {
                use sha1::Digest as Sha1Digest;
                let mut hasher = sha1::Sha1::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
            "sha256" => {
                let mut hasher = Sha256::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
            "sha512" => {
                use sha2::Sha512;
                let mut hasher = Sha512::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown algorithm: {}. Use md5, sha1, sha256, or sha512", algorithm),
                ));
            }
        };

        let size = content.len();

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "path": file_path.to_string_lossy(),
                "algorithm": algorithm,
                "hash": hash,
                "size": size
            }),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for verifying file checksums.
pub struct FileVerifyTool;

impl FileVerifyTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileVerifyTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct FileVerifyArgs {
    /// Path to the file
    path: String,
    /// Expected hash value
    expected: String,
    /// Hash algorithm
    #[serde(default)]
    algorithm: Option<String>,
}

#[async_trait]
impl Tool for FileVerifyTool {
    fn name(&self) -> &str {
        "file_verify"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "file_verify".to_string(),
            description: "Verify a file's checksum matches expected value".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Path to the file"
                    },
                    "expected": {
                        "type": "string",
                        "description": "Expected hash value"
                    },
                    "algorithm": {
                        "type": "string",
                        "enum": ["md5", "sha1", "sha256", "sha512"],
                        "description": "Hash algorithm (default: sha256)"
                    }
                },
                "required": ["path", "expected"]
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
        let args: FileVerifyArgs = serde_json::from_value(args)?;
        let algorithm = args.algorithm.unwrap_or_else(|| "sha256".to_string());

        let file_path = if Path::new(&args.path).is_absolute() {
            std::path::PathBuf::from(&args.path)
        } else {
            context.cwd.join(&args.path)
        };

        if !file_path.exists() {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("File not found: {}", file_path.display()),
            ));
        }

        let content = tokio::fs::read(&file_path)
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to read file: {}", e)))?;

        let actual_hash = match algorithm.to_lowercase().as_str() {
            "md5" => {
                let digest = md5::compute(&content);
                hex::encode(digest.0)
            }
            "sha1" => {
                use sha1::Digest as Sha1Digest;
                let mut hasher = sha1::Sha1::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
            "sha256" => {
                let mut hasher = Sha256::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
            "sha512" => {
                use sha2::Sha512;
                let mut hasher = Sha512::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown algorithm: {}. Use md5, sha1, sha256, or sha512", algorithm),
                ));
            }
        };

        let expected = args.expected.to_lowercase();
        let matches = actual_hash == expected;

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "path": file_path.to_string_lossy(),
                "algorithm": algorithm,
                "expected": expected,
                "actual": actual_hash,
                "matches": matches
            }),
        ).with_duration(start.elapsed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_file_checksum() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = FileChecksumTool::new();
        let result = tool.execute(
            "test",
            json!({
                "path": "test.txt",
                "algorithm": "sha256"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["hash"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_file_verify_match() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        // First get the actual hash
        let checksum_tool = FileChecksumTool::new();
        let checksum_result = checksum_tool.execute(
            "test",
            json!({
                "path": "test.txt",
                "algorithm": "sha256"
            }),
            &context,
        ).await.unwrap();

        let output: serde_json::Value = serde_json::from_value(checksum_result.output).unwrap();
        let hash = output["hash"].as_str().unwrap();

        // Now verify
        let verify_tool = FileVerifyTool::new();
        let result = verify_tool.execute(
            "test",
            json!({
                "path": "test.txt",
                "expected": hash,
                "algorithm": "sha256"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["matches"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_file_verify_mismatch() {
        let temp = TempDir::new().unwrap();
        let test_file = temp.path().join("test.txt");
        std::fs::write(&test_file, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: temp.path().to_path_buf(),
            ..Default::default()
        };

        let tool = FileVerifyTool::new();
        let result = tool.execute(
            "test",
            json!({
                "path": "test.txt",
                "expected": "0000000000000000000000000000000000000000000000000000000000000000",
                "algorithm": "sha256"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(!output["matches"].as_bool().unwrap());
    }
}
