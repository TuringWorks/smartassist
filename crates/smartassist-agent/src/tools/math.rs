//! Math and random tools.
//!
//! Provides tools for mathematical calculations,
//! random number generation, and UUID generation.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use rand::Rng;
use std::time::Instant;
use tracing::debug;

/// Tool for basic math calculations.
pub struct CalcTool;

impl CalcTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CalcTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CalcTool {
    fn name(&self) -> &str {
        "calc"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "calc".to_string(),
            description: "Perform basic mathematical calculations.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["add", "subtract", "multiply", "divide", "power", "sqrt", "abs", "round", "floor", "ceil", "mod", "min", "max"],
                        "description": "Mathematical operation"
                    },
                    "a": {
                        "type": "number",
                        "description": "First operand"
                    },
                    "b": {
                        "type": "number",
                        "description": "Second operand (for binary operations)"
                    },
                    "precision": {
                        "type": "integer",
                        "default": 10,
                        "description": "Decimal precision for rounding"
                    }
                },
                "required": ["operation", "a"]
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

        let operation = args
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("operation is required"))?;

        let a = args
            .get("a")
            .and_then(|v| v.as_f64())
            .ok_or_else(|| crate::error::AgentError::tool_execution("a is required"))?;

        let b = args.get("b").and_then(|v| v.as_f64());

        let precision = args
            .get("precision")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as i32;

        let result = match operation {
            "add" => {
                let b = b.ok_or_else(|| crate::error::AgentError::tool_execution("b is required for add"))?;
                a + b
            }
            "subtract" => {
                let b = b.ok_or_else(|| crate::error::AgentError::tool_execution("b is required for subtract"))?;
                a - b
            }
            "multiply" => {
                let b = b.ok_or_else(|| crate::error::AgentError::tool_execution("b is required for multiply"))?;
                a * b
            }
            "divide" => {
                let b = b.ok_or_else(|| crate::error::AgentError::tool_execution("b is required for divide"))?;
                if b == 0.0 {
                    return Ok(ToolResult::error(tool_use_id, "Division by zero"));
                }
                a / b
            }
            "power" => {
                let b = b.ok_or_else(|| crate::error::AgentError::tool_execution("b is required for power"))?;
                a.powf(b)
            }
            "sqrt" => {
                if a < 0.0 {
                    return Ok(ToolResult::error(tool_use_id, "Cannot take square root of negative number"));
                }
                a.sqrt()
            }
            "abs" => a.abs(),
            "round" => {
                let factor = 10_f64.powi(precision);
                (a * factor).round() / factor
            }
            "floor" => a.floor(),
            "ceil" => a.ceil(),
            "mod" => {
                let b = b.ok_or_else(|| crate::error::AgentError::tool_execution("b is required for mod"))?;
                a % b
            }
            "min" => {
                let b = b.ok_or_else(|| crate::error::AgentError::tool_execution("b is required for min"))?;
                a.min(b)
            }
            "max" => {
                let b = b.ok_or_else(|| crate::error::AgentError::tool_execution("b is required for max"))?;
                a.max(b)
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown operation: {}", operation),
                ));
            }
        };

        let duration = start.elapsed();

        debug!("Calc: {} = {}", operation, result);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "operation": operation,
                "a": a,
                "b": b,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for generating random values.
pub struct RandomTool;

impl RandomTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RandomTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for RandomTool {
    fn name(&self) -> &str {
        "random"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "random".to_string(),
            description: "Generate random numbers, strings, or pick random items.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "type": {
                        "type": "string",
                        "enum": ["integer", "float", "string", "choice", "shuffle", "bytes"],
                        "default": "integer",
                        "description": "Type of random value to generate"
                    },
                    "min": {
                        "type": "number",
                        "default": 0,
                        "description": "Minimum value (for integer/float)"
                    },
                    "max": {
                        "type": "number",
                        "default": 100,
                        "description": "Maximum value (for integer/float)"
                    },
                    "length": {
                        "type": "integer",
                        "default": 16,
                        "description": "Length (for string/bytes)"
                    },
                    "charset": {
                        "type": "string",
                        "enum": ["alphanumeric", "alpha", "numeric", "hex", "base64"],
                        "default": "alphanumeric",
                        "description": "Character set for string generation"
                    },
                    "items": {
                        "type": "array",
                        "items": {},
                        "description": "Items to choose from or shuffle"
                    },
                    "count": {
                        "type": "integer",
                        "default": 1,
                        "description": "Number of values to generate"
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
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let value_type = args
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("integer");

        let count = args
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let mut rng = rand::thread_rng();

        let result: serde_json::Value = match value_type {
            "integer" => {
                let min = args.get("min").and_then(|v| v.as_i64()).unwrap_or(0);
                let max = args.get("max").and_then(|v| v.as_i64()).unwrap_or(100);

                if count == 1 {
                    serde_json::json!(rng.gen_range(min..=max))
                } else {
                    let values: Vec<i64> = (0..count).map(|_| rng.gen_range(min..=max)).collect();
                    serde_json::json!(values)
                }
            }
            "float" => {
                let min = args.get("min").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let max = args.get("max").and_then(|v| v.as_f64()).unwrap_or(1.0);

                if count == 1 {
                    serde_json::json!(rng.gen_range(min..max))
                } else {
                    let values: Vec<f64> = (0..count).map(|_| rng.gen_range(min..max)).collect();
                    serde_json::json!(values)
                }
            }
            "string" => {
                let length = args.get("length").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
                let charset = args.get("charset").and_then(|v| v.as_str()).unwrap_or("alphanumeric");

                let chars: Vec<char> = match charset {
                    "alpha" => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".chars().collect(),
                    "numeric" => "0123456789".chars().collect(),
                    "hex" => "0123456789abcdef".chars().collect(),
                    "base64" => "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".chars().collect(),
                    _ => "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789".chars().collect(),
                };

                let generate_string = |rng: &mut rand::rngs::ThreadRng, len: usize| -> String {
                    (0..len).map(|_| chars[rng.gen_range(0..chars.len())]).collect()
                };

                if count == 1 {
                    serde_json::json!(generate_string(&mut rng, length))
                } else {
                    let values: Vec<String> = (0..count).map(|_| generate_string(&mut rng, length)).collect();
                    serde_json::json!(values)
                }
            }
            "choice" => {
                let items = args
                    .get("items")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| crate::error::AgentError::tool_execution("items is required for choice"))?;

                if items.is_empty() {
                    return Ok(ToolResult::error(tool_use_id, "items array is empty"));
                }

                if count == 1 {
                    items[rng.gen_range(0..items.len())].clone()
                } else {
                    let choices: Vec<serde_json::Value> = (0..count)
                        .map(|_| items[rng.gen_range(0..items.len())].clone())
                        .collect();
                    serde_json::json!(choices)
                }
            }
            "shuffle" => {
                let items = args
                    .get("items")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| crate::error::AgentError::tool_execution("items is required for shuffle"))?;

                let mut shuffled = items.clone();
                for i in (1..shuffled.len()).rev() {
                    let j = rng.gen_range(0..=i);
                    shuffled.swap(i, j);
                }
                serde_json::json!(shuffled)
            }
            "bytes" => {
                let length = args.get("length").and_then(|v| v.as_u64()).unwrap_or(16) as usize;
                let bytes: Vec<u8> = (0..length).map(|_| rng.gen()).collect();
                serde_json::json!(hex::encode(bytes))
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown type: {}", value_type),
                ));
            }
        };

        let duration = start.elapsed();

        debug!("Random {}: generated", value_type);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "type": value_type,
                "count": count,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for generating UUIDs.
pub struct UuidTool;

impl UuidTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UuidTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for UuidTool {
    fn name(&self) -> &str {
        "uuid"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "uuid".to_string(),
            description: "Generate UUID (Universally Unique Identifier).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "version": {
                        "type": "string",
                        "enum": ["v4", "v7"],
                        "default": "v4",
                        "description": "UUID version (v4=random, v7=time-based)"
                    },
                    "count": {
                        "type": "integer",
                        "default": 1,
                        "description": "Number of UUIDs to generate"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["hyphenated", "simple", "urn"],
                        "default": "hyphenated",
                        "description": "Output format"
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
        _ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let _version = args
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("v4");

        let count = args
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;

        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("hyphenated");

        let format_uuid = |uuid: uuid::Uuid| -> String {
            match format {
                "simple" => uuid.simple().to_string(),
                "urn" => uuid.urn().to_string(),
                _ => uuid.hyphenated().to_string(),
            }
        };

        let generate_uuid = || -> uuid::Uuid {
            // Always use v4 for now (v7 requires newer uuid crate version)
            uuid::Uuid::new_v4()
        };

        // Note: v7 requested but using v4 as fallback
        let actual_version = "v4";

        let result: serde_json::Value = if count == 1 {
            serde_json::json!(format_uuid(generate_uuid()))
        } else {
            let uuids: Vec<String> = (0..count).map(|_| format_uuid(generate_uuid())).collect();
            serde_json::json!(uuids)
        };

        let duration = start.elapsed();

        debug!("UUID {}: generated {} UUIDs", actual_version, count);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "version": actual_version,
                "format": format,
                "count": count,
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
    fn test_calc_tool_creation() {
        let tool = CalcTool::new();
        assert_eq!(tool.name(), "calc");
    }

    #[test]
    fn test_random_tool_creation() {
        let tool = RandomTool::new();
        assert_eq!(tool.name(), "random");
    }

    #[test]
    fn test_uuid_tool_creation() {
        let tool = UuidTool::new();
        assert_eq!(tool.name(), "uuid");
    }

    #[tokio::test]
    async fn test_calc_add() {
        let tool = CalcTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "operation": "add",
                    "a": 5,
                    "b": 3
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_f64()),
            Some(8.0)
        );
    }

    #[tokio::test]
    async fn test_calc_sqrt() {
        let tool = CalcTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "operation": "sqrt",
                    "a": 16
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_f64()),
            Some(4.0)
        );
    }

    #[tokio::test]
    async fn test_calc_divide_by_zero() {
        let tool = CalcTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "operation": "divide",
                    "a": 10,
                    "b": 0
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_random_integer() {
        let tool = RandomTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "type": "integer",
                    "min": 1,
                    "max": 10
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let value = result.output.get("result").and_then(|v| v.as_i64()).unwrap();
        assert!(value >= 1 && value <= 10);
    }

    #[tokio::test]
    async fn test_random_string() {
        let tool = RandomTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "type": "string",
                    "length": 8,
                    "charset": "hex"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let value = result.output.get("result").and_then(|v| v.as_str()).unwrap();
        assert_eq!(value.len(), 8);
    }

    #[tokio::test]
    async fn test_random_choice() {
        let tool = RandomTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "type": "choice",
                    "items": ["a", "b", "c"]
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let value = result.output.get("result").and_then(|v| v.as_str()).unwrap();
        assert!(["a", "b", "c"].contains(&value));
    }

    #[tokio::test]
    async fn test_uuid_v4() {
        let tool = UuidTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "version": "v4"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let uuid_str = result.output.get("result").and_then(|v| v.as_str()).unwrap();
        assert!(uuid::Uuid::parse_str(uuid_str).is_ok());
    }

    #[tokio::test]
    async fn test_uuid_count() {
        let tool = UuidTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "count": 3
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let uuids = result.output.get("result").and_then(|v| v.as_array()).unwrap();
        assert_eq!(uuids.len(), 3);
    }
}
