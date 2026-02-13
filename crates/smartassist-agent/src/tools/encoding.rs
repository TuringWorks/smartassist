//! Encoding and hashing tools.
//!
//! Provides tools for encoding/decoding data (base64, hex)
//! and computing hashes (MD5, SHA256).

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Tool for base64 encoding/decoding.
pub struct Base64Tool;

impl Base64Tool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Base64Tool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for Base64Tool {
    fn name(&self) -> &str {
        "base64"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "base64".to_string(),
            description: "Encode or decode base64 data.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The input string"
                    },
                    "operation": {
                        "type": "string",
                        "enum": ["encode", "decode"],
                        "default": "encode",
                        "description": "Operation to perform"
                    },
                    "url_safe": {
                        "type": "boolean",
                        "default": false,
                        "description": "Use URL-safe base64 encoding"
                    }
                },
                "required": ["input"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        use base64::{engine::general_purpose, Engine};

        let start = Instant::now();

        let input = args
            .get("input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("input is required"))?;

        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("encode");

        let url_safe = args
            .get("url_safe")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let result = match operation {
            "encode" => {
                if url_safe {
                    general_purpose::URL_SAFE.encode(input.as_bytes())
                } else {
                    general_purpose::STANDARD.encode(input.as_bytes())
                }
            }
            "decode" => {
                let decoded = if url_safe {
                    general_purpose::URL_SAFE.decode(input)
                } else {
                    general_purpose::STANDARD.decode(input)
                };

                match decoded {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(e) => {
                        return Ok(ToolResult::error(
                            tool_use_id,
                            format!("Failed to decode base64: {}", e),
                        ));
                    }
                }
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Invalid operation: {}", operation),
                ));
            }
        };

        let duration = start.elapsed();

        debug!("Base64 {}: {} bytes", operation, result.len());

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "operation": operation,
                "input_length": input.len(),
                "output_length": result.len(),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for hex encoding/decoding.
pub struct HexTool;

impl HexTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HexTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HexTool {
    fn name(&self) -> &str {
        "hex"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "hex".to_string(),
            description: "Encode or decode hexadecimal data.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The input string"
                    },
                    "operation": {
                        "type": "string",
                        "enum": ["encode", "decode"],
                        "default": "encode",
                        "description": "Operation to perform"
                    },
                    "uppercase": {
                        "type": "boolean",
                        "default": false,
                        "description": "Use uppercase hex characters"
                    }
                },
                "required": ["input"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let input = args
            .get("input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("input is required"))?;

        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("encode");

        let uppercase = args
            .get("uppercase")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let result = match operation {
            "encode" => {
                let hex = hex::encode(input.as_bytes());
                if uppercase {
                    hex.to_uppercase()
                } else {
                    hex
                }
            }
            "decode" => {
                match hex::decode(input) {
                    Ok(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    Err(e) => {
                        return Ok(ToolResult::error(
                            tool_use_id,
                            format!("Failed to decode hex: {}", e),
                        ));
                    }
                }
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Invalid operation: {}", operation),
                ));
            }
        };

        let duration = start.elapsed();

        debug!("Hex {}: {} bytes", operation, result.len());

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "operation": operation,
                "input_length": input.len(),
                "output_length": result.len(),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for computing hashes.
pub struct HashTool;

impl HashTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HashTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for HashTool {
    fn name(&self) -> &str {
        "hash"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "hash".to_string(),
            description: "Compute cryptographic hash of input data.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The input string to hash"
                    },
                    "algorithm": {
                        "type": "string",
                        "enum": ["md5", "sha1", "sha256", "sha512"],
                        "default": "sha256",
                        "description": "Hash algorithm to use"
                    },
                    "file": {
                        "type": "string",
                        "description": "File path to hash (alternative to input)"
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
        use sha2::{Sha256, Sha512, Digest};
        use sha1::Sha1;

        let start = Instant::now();

        let algorithm = args
            .get("algorithm")
            .and_then(|v| v.as_str())
            .unwrap_or("sha256");

        // Get content either from input or file
        let content = if let Some(input) = args.get("input").and_then(|v| v.as_str()) {
            input.as_bytes().to_vec()
        } else if let Some(file) = args.get("file").and_then(|v| v.as_str()) {
            let path = if std::path::Path::new(file).is_absolute() {
                std::path::PathBuf::from(file)
            } else {
                ctx.cwd.join(file)
            };

            tokio::fs::read(&path)
                .await
                .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to read file: {}", e)))?
        } else {
            return Ok(ToolResult::error(
                tool_use_id,
                "Either input or file must be provided",
            ));
        };

        let hash = match algorithm {
            "md5" => {
                let digest = md5::compute(&content);
                hex::encode(digest.0)
            }
            "sha1" => {
                let mut hasher = Sha1::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
            "sha256" => {
                let mut hasher = Sha256::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
            "sha512" => {
                let mut hasher = Sha512::new();
                hasher.update(&content);
                hex::encode(hasher.finalize())
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown algorithm: {}", algorithm),
                ));
            }
        };

        let duration = start.elapsed();

        debug!("Hash {}: {} chars", algorithm, hash.len());

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "hash": hash,
                "algorithm": algorithm,
                "input_bytes": content.len(),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for URL encoding/decoding.
pub struct UrlEncodeTool;

impl UrlEncodeTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UrlEncodeTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for UrlEncodeTool {
    fn name(&self) -> &str {
        "url_encode"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "url_encode".to_string(),
            description: "URL encode or decode a string.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The input string"
                    },
                    "operation": {
                        "type": "string",
                        "enum": ["encode", "decode"],
                        "default": "encode",
                        "description": "Operation to perform"
                    }
                },
                "required": ["input"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        use urlencoding::{encode, decode};

        let start = Instant::now();

        let input = args
            .get("input")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("input is required"))?;

        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .unwrap_or("encode");

        let result = match operation {
            "encode" => encode(input).to_string(),
            "decode" => {
                match decode(input) {
                    Ok(s) => s.to_string(),
                    Err(e) => {
                        return Ok(ToolResult::error(
                            tool_use_id,
                            format!("Failed to decode URL: {}", e),
                        ));
                    }
                }
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Invalid operation: {}", operation),
                ));
            }
        };

        let duration = start.elapsed();

        debug!("URL {}: {} chars", operation, result.len());

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "operation": operation,
                "input_length": input.len(),
                "output_length": result.len(),
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
    fn test_base64_tool_creation() {
        let tool = Base64Tool::new();
        assert_eq!(tool.name(), "base64");
    }

    #[test]
    fn test_hex_tool_creation() {
        let tool = HexTool::new();
        assert_eq!(tool.name(), "hex");
    }

    #[test]
    fn test_hash_tool_creation() {
        let tool = HashTool::new();
        assert_eq!(tool.name(), "hash");
    }

    #[test]
    fn test_url_encode_tool_creation() {
        let tool = UrlEncodeTool::new();
        assert_eq!(tool.name(), "url_encode");
    }

    #[tokio::test]
    async fn test_base64_encode() {
        let tool = Base64Tool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "Hello, World!",
                    "operation": "encode"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("SGVsbG8sIFdvcmxkIQ==")
        );
    }

    #[tokio::test]
    async fn test_base64_decode() {
        let tool = Base64Tool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "SGVsbG8sIFdvcmxkIQ==",
                    "operation": "decode"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("Hello, World!")
        );
    }

    #[tokio::test]
    async fn test_hex_encode() {
        let tool = HexTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "hello",
                    "operation": "encode"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("68656c6c6f")
        );
    }

    #[tokio::test]
    async fn test_hex_decode() {
        let tool = HexTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "68656c6c6f",
                    "operation": "decode"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("hello")
        );
    }

    #[tokio::test]
    async fn test_hash_sha256() {
        let tool = HashTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "hello",
                    "algorithm": "sha256"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("hash").and_then(|v| v.as_str()),
            Some("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824")
        );
    }

    #[tokio::test]
    async fn test_hash_md5() {
        let tool = HashTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "hello",
                    "algorithm": "md5"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("hash").and_then(|v| v.as_str()),
            Some("5d41402abc4b2a76b9719d911017c592")
        );
    }

    #[tokio::test]
    async fn test_url_encode() {
        let tool = UrlEncodeTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "hello world",
                    "operation": "encode"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("hello%20world")
        );
    }

    #[tokio::test]
    async fn test_url_decode() {
        let tool = UrlEncodeTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "hello%20world",
                    "operation": "decode"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("hello world")
        );
    }
}
