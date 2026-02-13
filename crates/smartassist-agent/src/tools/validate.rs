//! Validation tools.
//!
//! Provides tools for validating data formats like
//! email addresses, URLs, JSON, and more.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Tool for validating various data formats.
pub struct ValidateTool;

impl ValidateTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ValidateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ValidateTool {
    fn name(&self) -> &str {
        "validate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "validate".to_string(),
            description: "Validate data against common formats (email, URL, JSON, etc.)."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The input to validate"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["email", "url", "json", "uuid", "ip", "ipv4", "ipv6", "semver", "date", "base64", "hex", "phone"],
                        "description": "Format to validate against"
                    }
                },
                "required": ["input", "format"]
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

        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("format is required"))?;

        let (valid, message, details) = match format {
            "email" => validate_email(input),
            "url" => validate_url(input),
            "json" => validate_json(input),
            "uuid" => validate_uuid(input),
            "ip" => validate_ip(input),
            "ipv4" => validate_ipv4(input),
            "ipv6" => validate_ipv6(input),
            "semver" => validate_semver(input),
            "date" => validate_date(input),
            "base64" => validate_base64(input),
            "hex" => validate_hex(input),
            "phone" => validate_phone(input),
            _ => (false, format!("Unknown format: {}", format), None),
        };

        let duration = start.elapsed();

        debug!("Validate {}: valid={}", format, valid);

        let mut response = serde_json::json!({
            "valid": valid,
            "format": format,
            "input": input,
            "message": message,
        });

        if let Some(details) = details {
            response["details"] = details;
        }

        Ok(ToolResult::success(tool_use_id, response).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

fn validate_email(input: &str) -> (bool, String, Option<serde_json::Value>) {
    // Simple email regex
    let email_regex = regex::Regex::new(
        r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
    ).unwrap();

    if email_regex.is_match(input) {
        let parts: Vec<&str> = input.split('@').collect();
        let details = serde_json::json!({
            "local": parts.get(0).unwrap_or(&""),
            "domain": parts.get(1).unwrap_or(&""),
        });
        (true, "Valid email address".to_string(), Some(details))
    } else {
        (false, "Invalid email address format".to_string(), None)
    }
}

fn validate_url(input: &str) -> (bool, String, Option<serde_json::Value>) {
    match url::Url::parse(input) {
        Ok(parsed) => {
            let details = serde_json::json!({
                "scheme": parsed.scheme(),
                "host": parsed.host_str(),
                "port": parsed.port(),
                "path": parsed.path(),
                "query": parsed.query(),
            });
            (true, "Valid URL".to_string(), Some(details))
        }
        Err(e) => (false, format!("Invalid URL: {}", e), None),
    }
}

fn validate_json(input: &str) -> (bool, String, Option<serde_json::Value>) {
    match serde_json::from_str::<serde_json::Value>(input) {
        Ok(value) => {
            let type_name = match &value {
                serde_json::Value::Null => "null",
                serde_json::Value::Bool(_) => "boolean",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::Object(_) => "object",
            };
            let details = serde_json::json!({
                "type": type_name,
            });
            (true, "Valid JSON".to_string(), Some(details))
        }
        Err(e) => (false, format!("Invalid JSON: {}", e), None),
    }
}

fn validate_uuid(input: &str) -> (bool, String, Option<serde_json::Value>) {
    match uuid::Uuid::parse_str(input) {
        Ok(uuid) => {
            let version = match uuid.get_version() {
                Some(uuid::Version::Nil) => "nil",
                Some(uuid::Version::Mac) => "v1",
                Some(uuid::Version::Dce) => "v2",
                Some(uuid::Version::Md5) => "v3",
                Some(uuid::Version::Random) => "v4",
                Some(uuid::Version::Sha1) => "v5",
                Some(uuid::Version::SortMac) => "v6",
                Some(uuid::Version::SortRand) => "v7",
                Some(uuid::Version::Custom) => "v8",
                Some(_) => "unknown",
                None => "unknown",
            };
            let details = serde_json::json!({
                "version": version,
                "variant": format!("{:?}", uuid.get_variant()),
            });
            (true, "Valid UUID".to_string(), Some(details))
        }
        Err(e) => (false, format!("Invalid UUID: {}", e), None),
    }
}

fn validate_ip(input: &str) -> (bool, String, Option<serde_json::Value>) {
    if let Ok(addr) = input.parse::<std::net::IpAddr>() {
        let details = serde_json::json!({
            "version": if addr.is_ipv4() { "v4" } else { "v6" },
            "loopback": addr.is_loopback(),
            "multicast": addr.is_multicast(),
        });
        (true, "Valid IP address".to_string(), Some(details))
    } else {
        (false, "Invalid IP address".to_string(), None)
    }
}

fn validate_ipv4(input: &str) -> (bool, String, Option<serde_json::Value>) {
    if let Ok(addr) = input.parse::<std::net::Ipv4Addr>() {
        let details = serde_json::json!({
            "loopback": addr.is_loopback(),
            "private": addr.is_private(),
            "broadcast": addr.is_broadcast(),
            "octets": addr.octets(),
        });
        (true, "Valid IPv4 address".to_string(), Some(details))
    } else {
        (false, "Invalid IPv4 address".to_string(), None)
    }
}

fn validate_ipv6(input: &str) -> (bool, String, Option<serde_json::Value>) {
    if let Ok(addr) = input.parse::<std::net::Ipv6Addr>() {
        let details = serde_json::json!({
            "loopback": addr.is_loopback(),
            "multicast": addr.is_multicast(),
            "segments": addr.segments(),
        });
        (true, "Valid IPv6 address".to_string(), Some(details))
    } else {
        (false, "Invalid IPv6 address".to_string(), None)
    }
}

fn validate_semver(input: &str) -> (bool, String, Option<serde_json::Value>) {
    // Simple semver regex
    let semver_regex = regex::Regex::new(
        r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
    ).unwrap();

    if let Some(caps) = semver_regex.captures(input) {
        let details = serde_json::json!({
            "major": caps.get(1).map(|m| m.as_str()),
            "minor": caps.get(2).map(|m| m.as_str()),
            "patch": caps.get(3).map(|m| m.as_str()),
            "prerelease": caps.get(4).map(|m| m.as_str()),
            "build": caps.get(5).map(|m| m.as_str()),
        });
        (true, "Valid semantic version".to_string(), Some(details))
    } else {
        (false, "Invalid semantic version".to_string(), None)
    }
}

fn validate_date(input: &str) -> (bool, String, Option<serde_json::Value>) {
    // Try ISO 8601 format first
    if chrono::DateTime::parse_from_rfc3339(input).is_ok() {
        return (true, "Valid ISO 8601 date".to_string(), Some(serde_json::json!({"format": "ISO 8601"})));
    }

    // Try common date formats
    let formats = [
        ("%Y-%m-%d", "YYYY-MM-DD"),
        ("%Y/%m/%d", "YYYY/MM/DD"),
        ("%m/%d/%Y", "MM/DD/YYYY"),
        ("%d/%m/%Y", "DD/MM/YYYY"),
        ("%Y-%m-%d %H:%M:%S", "YYYY-MM-DD HH:MM:SS"),
    ];

    for (fmt, name) in formats {
        if chrono::NaiveDateTime::parse_from_str(input, fmt).is_ok() ||
           chrono::NaiveDate::parse_from_str(input, fmt).is_ok() {
            return (true, "Valid date".to_string(), Some(serde_json::json!({"format": name})));
        }
    }

    (false, "Invalid date format".to_string(), None)
}

fn validate_base64(input: &str) -> (bool, String, Option<serde_json::Value>) {
    use base64::{engine::general_purpose, Engine};

    match general_purpose::STANDARD.decode(input) {
        Ok(bytes) => {
            let details = serde_json::json!({
                "decoded_length": bytes.len(),
            });
            (true, "Valid base64".to_string(), Some(details))
        }
        Err(_) => {
            // Try URL-safe variant
            match general_purpose::URL_SAFE.decode(input) {
                Ok(bytes) => {
                    let details = serde_json::json!({
                        "decoded_length": bytes.len(),
                        "variant": "url_safe",
                    });
                    (true, "Valid base64 (URL-safe)".to_string(), Some(details))
                }
                Err(e) => (false, format!("Invalid base64: {}", e), None),
            }
        }
    }
}

fn validate_hex(input: &str) -> (bool, String, Option<serde_json::Value>) {
    match hex::decode(input) {
        Ok(bytes) => {
            let details = serde_json::json!({
                "decoded_length": bytes.len(),
            });
            (true, "Valid hexadecimal".to_string(), Some(details))
        }
        Err(e) => (false, format!("Invalid hexadecimal: {}", e), None),
    }
}

fn validate_phone(input: &str) -> (bool, String, Option<serde_json::Value>) {
    // Simple phone number regex (E.164 format and common formats)
    let phone_regex = regex::Regex::new(
        r"^(\+?1?[-.\s]?)?\(?[2-9]\d{2}\)?[-.\s]?\d{3}[-.\s]?\d{4}$|^\+[1-9]\d{6,14}$"
    ).unwrap();

    if phone_regex.is_match(input) {
        let digits: String = input.chars().filter(|c| c.is_ascii_digit()).collect();
        let details = serde_json::json!({
            "digits": digits,
            "digit_count": digits.len(),
        });
        (true, "Valid phone number".to_string(), Some(details))
    } else {
        (false, "Invalid phone number format".to_string(), None)
    }
}

/// Tool for checking if a value is empty/null/blank.
pub struct IsEmptyTool;

impl IsEmptyTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IsEmptyTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for IsEmptyTool {
    fn name(&self) -> &str {
        "is_empty"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "is_empty".to_string(),
            description: "Check if a value is empty, null, blank, or has a default value."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "value": {
                        "description": "The value to check (any type)"
                    },
                    "trim": {
                        "type": "boolean",
                        "default": true,
                        "description": "Trim whitespace before checking strings"
                    }
                },
                "required": ["value"]
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

        let value = args
            .get("value")
            .ok_or_else(|| crate::error::AgentError::tool_execution("value is required"))?;

        let trim = args
            .get("trim")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let (is_empty, reason) = match value {
            serde_json::Value::Null => (true, "null"),
            serde_json::Value::Bool(_) => (false, "boolean value"),
            serde_json::Value::Number(n) => {
                if n.as_f64() == Some(0.0) {
                    (false, "zero (not considered empty)")
                } else {
                    (false, "non-zero number")
                }
            }
            serde_json::Value::String(s) => {
                let check_str = if trim { s.trim() } else { s.as_str() };
                if check_str.is_empty() {
                    (true, "empty string")
                } else {
                    (false, "non-empty string")
                }
            }
            serde_json::Value::Array(arr) => {
                if arr.is_empty() {
                    (true, "empty array")
                } else {
                    (false, "non-empty array")
                }
            }
            serde_json::Value::Object(obj) => {
                if obj.is_empty() {
                    (true, "empty object")
                } else {
                    (false, "non-empty object")
                }
            }
        };

        let duration = start.elapsed();

        debug!("IsEmpty: {} ({})", is_empty, reason);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "is_empty": is_empty,
                "reason": reason,
                "type": match value {
                    serde_json::Value::Null => "null",
                    serde_json::Value::Bool(_) => "boolean",
                    serde_json::Value::Number(_) => "number",
                    serde_json::Value::String(_) => "string",
                    serde_json::Value::Array(_) => "array",
                    serde_json::Value::Object(_) => "object",
                },
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
    fn test_validate_tool_creation() {
        let tool = ValidateTool::new();
        assert_eq!(tool.name(), "validate");
    }

    #[test]
    fn test_is_empty_tool_creation() {
        let tool = IsEmptyTool::new();
        assert_eq!(tool.name(), "is_empty");
    }

    #[tokio::test]
    async fn test_validate_email_valid() {
        let tool = ValidateTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "test@example.com",
                    "format": "email"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("valid").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_validate_email_invalid() {
        let tool = ValidateTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "invalid-email",
                    "format": "email"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("valid").and_then(|v| v.as_bool()),
            Some(false)
        );
    }

    #[tokio::test]
    async fn test_validate_url() {
        let tool = ValidateTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "https://example.com/path?query=1",
                    "format": "url"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("valid").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_validate_json() {
        let tool = ValidateTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": r#"{"key": "value"}"#,
                    "format": "json"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("valid").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_validate_uuid() {
        let tool = ValidateTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "550e8400-e29b-41d4-a716-446655440000",
                    "format": "uuid"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("valid").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_validate_ipv4() {
        let tool = ValidateTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "192.168.1.1",
                    "format": "ipv4"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("valid").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_validate_semver() {
        let tool = ValidateTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "1.2.3-beta.1+build.456",
                    "format": "semver"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("valid").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_is_empty_null() {
        let tool = IsEmptyTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "value": null
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("is_empty").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_is_empty_string() {
        let tool = IsEmptyTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "value": "   "
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("is_empty").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[tokio::test]
    async fn test_is_empty_array() {
        let tool = IsEmptyTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "value": []
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("is_empty").and_then(|v| v.as_bool()),
            Some(true)
        );
    }
}
