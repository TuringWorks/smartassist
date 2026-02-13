//! Comparison and assertion tools.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;

/// Tool for comparing two values.
pub struct CompareTool;

impl CompareTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CompareTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct CompareArgs {
    /// First value
    a: serde_json::Value,
    /// Second value
    b: serde_json::Value,
    /// Comparison operator
    operator: String,
}

#[async_trait]
impl Tool for CompareTool {
    fn name(&self) -> &str {
        "compare"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "compare".to_string(),
            description: "Compare two values with various operators".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "a": {
                        "description": "First value"
                    },
                    "b": {
                        "description": "Second value"
                    },
                    "operator": {
                        "type": "string",
                        "enum": ["eq", "ne", "lt", "le", "gt", "ge", "contains", "starts_with", "ends_with"],
                        "description": "Comparison operator"
                    }
                },
                "required": ["a", "b", "operator"]
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
        let args: CompareArgs = serde_json::from_value(args)?;

        let result = match args.operator.as_str() {
            "eq" => args.a == args.b,
            "ne" => args.a != args.b,
            "lt" | "le" | "gt" | "ge" => {
                // Numeric comparison
                let a = args.a.as_f64();
                let b = args.b.as_f64();
                match (a, b) {
                    (Some(a), Some(b)) => match args.operator.as_str() {
                        "lt" => a < b,
                        "le" => a <= b,
                        "gt" => a > b,
                        "ge" => a >= b,
                        _ => false,
                    },
                    _ => {
                        return Ok(ToolResult::error(
                            tool_use_id,
                            "Numeric comparison requires both values to be numbers".to_string(),
                        ));
                    }
                }
            }
            "contains" => {
                let a_str = value_to_string(&args.a);
                let b_str = value_to_string(&args.b);
                a_str.contains(&b_str)
            }
            "starts_with" => {
                let a_str = value_to_string(&args.a);
                let b_str = value_to_string(&args.b);
                a_str.starts_with(&b_str)
            }
            "ends_with" => {
                let a_str = value_to_string(&args.a);
                let b_str = value_to_string(&args.b);
                a_str.ends_with(&b_str)
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown operator: {}", args.operator),
                ));
            }
        };

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "result": result,
                "operator": args.operator,
                "a": args.a,
                "b": args.b
            }),
        ).with_duration(start.elapsed()))
    }
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => "null".to_string(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

/// Tool for assertions (returns error if condition is false).
pub struct AssertTool;

impl AssertTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AssertTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct AssertArgs {
    /// Condition to assert
    condition: bool,
    /// Error message if assertion fails
    #[serde(default)]
    message: Option<String>,
}

#[async_trait]
impl Tool for AssertTool {
    fn name(&self) -> &str {
        "assert"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "assert".to_string(),
            description: "Assert that a condition is true".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "condition": {
                        "type": "boolean",
                        "description": "Condition to assert"
                    },
                    "message": {
                        "type": "string",
                        "description": "Error message if assertion fails"
                    }
                },
                "required": ["condition"]
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
        let args: AssertArgs = serde_json::from_value(args)?;

        if args.condition {
            Ok(ToolResult::success(
                tool_use_id,
                json!({
                    "passed": true
                }),
            ).with_duration(start.elapsed()))
        } else {
            let message = args.message.unwrap_or_else(|| "Assertion failed".to_string());
            Ok(ToolResult::error(tool_use_id, message))
        }
    }
}

/// Tool for checking if a value matches a pattern.
pub struct MatchTool;

impl MatchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MatchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct MatchArgs {
    /// Value to match
    value: String,
    /// Pattern to match against (regex)
    pattern: String,
    /// Case insensitive
    #[serde(default)]
    ignore_case: Option<bool>,
}

#[async_trait]
impl Tool for MatchTool {
    fn name(&self) -> &str {
        "match"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "match".to_string(),
            description: "Check if a value matches a regex pattern".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "value": {
                        "type": "string",
                        "description": "Value to match"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern"
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "description": "Case insensitive matching"
                    }
                },
                "required": ["value", "pattern"]
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
        let args: MatchArgs = serde_json::from_value(args)?;

        let pattern = if args.ignore_case.unwrap_or(false) {
            format!("(?i){}", args.pattern)
        } else {
            args.pattern.clone()
        };

        let regex = regex::Regex::new(&pattern)
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Invalid regex: {}", e)))?;

        let matches = regex.is_match(&args.value);
        let captures: Vec<String> = if matches {
            regex.captures(&args.value)
                .map(|caps| {
                    caps.iter()
                        .filter_map(|m| m.map(|m| m.as_str().to_string()))
                        .collect()
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "matches": matches,
                "pattern": args.pattern,
                "captures": captures
            }),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for version comparison.
pub struct VersionCompareTool;

impl VersionCompareTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VersionCompareTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct VersionCompareArgs {
    /// First version
    version_a: String,
    /// Second version
    version_b: String,
}

#[async_trait]
impl Tool for VersionCompareTool {
    fn name(&self) -> &str {
        "version_compare"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "version_compare".to_string(),
            description: "Compare two semantic version strings".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "version_a": {
                        "type": "string",
                        "description": "First version (e.g., 1.2.3)"
                    },
                    "version_b": {
                        "type": "string",
                        "description": "Second version (e.g., 1.2.4)"
                    }
                },
                "required": ["version_a", "version_b"]
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
        let args: VersionCompareArgs = serde_json::from_value(args)?;

        // Parse versions (simple semver-like parsing)
        let parse_version = |v: &str| -> Vec<u32> {
            v.trim_start_matches('v')
                .split('.')
                .filter_map(|p| {
                    // Take only the numeric part
                    let numeric: String = p.chars().take_while(|c| c.is_ascii_digit()).collect();
                    numeric.parse::<u32>().ok()
                })
                .collect()
        };

        let a = parse_version(&args.version_a);
        let b = parse_version(&args.version_b);

        // Compare
        let mut comparison = std::cmp::Ordering::Equal;
        for i in 0..a.len().max(b.len()) {
            let va = a.get(i).copied().unwrap_or(0);
            let vb = b.get(i).copied().unwrap_or(0);
            match va.cmp(&vb) {
                std::cmp::Ordering::Equal => continue,
                ord => {
                    comparison = ord;
                    break;
                }
            }
        }

        let (result_str, is_equal, is_greater, is_less) = match comparison {
            std::cmp::Ordering::Equal => ("equal", true, false, false),
            std::cmp::Ordering::Greater => ("greater", false, true, false),
            std::cmp::Ordering::Less => ("less", false, false, true),
        };

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "version_a": args.version_a,
                "version_b": args.version_b,
                "comparison": result_str,
                "is_equal": is_equal,
                "a_greater_than_b": is_greater,
                "a_less_than_b": is_less
            }),
        ).with_duration(start.elapsed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_compare_eq() {
        let tool = CompareTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "a": 42,
                "b": 42,
                "operator": "eq"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["result"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_compare_gt() {
        let tool = CompareTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "a": 10,
                "b": 5,
                "operator": "gt"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["result"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_compare_contains() {
        let tool = CompareTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "a": "hello world",
                "b": "world",
                "operator": "contains"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["result"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_assert_pass() {
        let tool = AssertTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "condition": true
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_assert_fail() {
        let tool = AssertTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "condition": false,
                "message": "Custom error message"
            }),
            &context,
        ).await.unwrap();

        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_match_regex() {
        let tool = MatchTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "value": "hello123world",
                "pattern": r"\d+"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["matches"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_version_compare() {
        let tool = VersionCompareTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "version_a": "1.2.3",
                "version_b": "1.2.4"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["a_less_than_b"].as_bool().unwrap());
    }

    #[tokio::test]
    async fn test_version_compare_equal() {
        let tool = VersionCompareTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "version_a": "v2.0.0",
                "version_b": "2.0.0"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["is_equal"].as_bool().unwrap());
    }
}
