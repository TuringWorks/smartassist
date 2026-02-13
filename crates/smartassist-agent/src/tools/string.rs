//! String manipulation tools.
//!
//! Provides tools for text processing, manipulation,
//! and transformation.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Tool for string case conversion.
pub struct CaseTool;

impl CaseTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CaseTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CaseTool {
    fn name(&self) -> &str {
        "case"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "case".to_string(),
            description: "Convert string case (upper, lower, title, camel, snake, kebab)."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The input string"
                    },
                    "to": {
                        "type": "string",
                        "enum": ["upper", "lower", "title", "camel", "pascal", "snake", "kebab", "constant"],
                        "description": "Target case"
                    }
                },
                "required": ["input", "to"]
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

        let to = args
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("to is required"))?;

        let result = match to {
            "upper" => input.to_uppercase(),
            "lower" => input.to_lowercase(),
            "title" => to_title_case(input),
            "camel" => to_camel_case(input),
            "pascal" => to_pascal_case(input),
            "snake" => to_snake_case(input),
            "kebab" => to_kebab_case(input),
            "constant" => to_snake_case(input).to_uppercase(),
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown case: {}", to),
                ));
            }
        };

        let duration = start.elapsed();

        debug!("Case conversion: {} -> {}", to, result.len());

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "original": input,
                "case": to,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

fn to_title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn split_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for ch in s.chars() {
        if ch == '_' || ch == '-' || ch == ' ' {
            if !current.is_empty() {
                words.push(current.clone());
                current.clear();
            }
        } else if ch.is_uppercase() && !current.is_empty() && current.chars().last().map(|c| c.is_lowercase()).unwrap_or(false) {
            words.push(current.clone());
            current.clear();
            current.push(ch);
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        words.push(current);
    }

    words
}

fn to_camel_case(s: &str) -> String {
    let words = split_words(s);
    words
        .iter()
        .enumerate()
        .map(|(i, word)| {
            if i == 0 {
                word.to_lowercase()
            } else {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str(),
                }
            }
        })
        .collect()
}

fn to_pascal_case(s: &str) -> String {
    let words = split_words(s);
    words
        .iter()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str().to_lowercase().as_str(),
            }
        })
        .collect()
}

fn to_snake_case(s: &str) -> String {
    let words = split_words(s);
    words
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

fn to_kebab_case(s: &str) -> String {
    let words = split_words(s);
    words
        .iter()
        .map(|w| w.to_lowercase())
        .collect::<Vec<_>>()
        .join("-")
}

/// Tool for splitting and joining strings.
pub struct SplitJoinTool;

impl SplitJoinTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SplitJoinTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SplitJoinTool {
    fn name(&self) -> &str {
        "split_join"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "split_join".to_string(),
            description: "Split a string into parts or join parts into a string.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["split", "join"],
                        "description": "Operation to perform"
                    },
                    "input": {
                        "type": "string",
                        "description": "Input string (for split)"
                    },
                    "parts": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Parts to join (for join)"
                    },
                    "delimiter": {
                        "type": "string",
                        "default": ",",
                        "description": "Delimiter for split/join"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of splits"
                    },
                    "trim": {
                        "type": "boolean",
                        "default": false,
                        "description": "Trim whitespace from parts"
                    }
                },
                "required": ["operation"]
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

        let delimiter = args
            .get("delimiter")
            .and_then(|v| v.as_str())
            .unwrap_or(",");

        let trim = args
            .get("trim")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        match operation {
            "split" => {
                let input = args
                    .get("input")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| crate::error::AgentError::tool_execution("input is required for split"))?;

                let limit = args.get("limit").and_then(|v| v.as_u64()).map(|n| n as usize);

                let parts: Vec<String> = if let Some(n) = limit {
                    input.splitn(n, delimiter).map(|s| {
                        if trim { s.trim().to_string() } else { s.to_string() }
                    }).collect()
                } else {
                    input.split(delimiter).map(|s| {
                        if trim { s.trim().to_string() } else { s.to_string() }
                    }).collect()
                };

                let duration = start.elapsed();

                debug!("Split: {} parts", parts.len());

                Ok(ToolResult::success(
                    tool_use_id,
                    serde_json::json!({
                        "parts": parts,
                        "count": parts.len(),
                        "delimiter": delimiter,
                    }),
                )
                .with_duration(duration))
            }
            "join" => {
                let parts: Vec<String> = args
                    .get("parts")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| {
                                if trim { s.trim().to_string() } else { s.to_string() }
                            }))
                            .collect()
                    })
                    .ok_or_else(|| crate::error::AgentError::tool_execution("parts is required for join"))?;

                let result = parts.join(delimiter);

                let duration = start.elapsed();

                debug!("Join: {} chars", result.len());

                Ok(ToolResult::success(
                    tool_use_id,
                    serde_json::json!({
                        "result": result,
                        "part_count": parts.len(),
                        "delimiter": delimiter,
                    }),
                )
                .with_duration(duration))
            }
            _ => Ok(ToolResult::error(
                tool_use_id,
                format!("Unknown operation: {}", operation),
            )),
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for string replacement with regex support.
pub struct ReplaceTool;

impl ReplaceTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReplaceTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReplaceTool {
    fn name(&self) -> &str {
        "replace"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "replace".to_string(),
            description: "Replace text in a string (supports regex).".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The input string"
                    },
                    "pattern": {
                        "type": "string",
                        "description": "Pattern to search for"
                    },
                    "replacement": {
                        "type": "string",
                        "description": "Replacement text"
                    },
                    "regex": {
                        "type": "boolean",
                        "default": false,
                        "description": "Treat pattern as regex"
                    },
                    "all": {
                        "type": "boolean",
                        "default": true,
                        "description": "Replace all occurrences"
                    },
                    "case_insensitive": {
                        "type": "boolean",
                        "default": false,
                        "description": "Case insensitive matching"
                    }
                },
                "required": ["input", "pattern", "replacement"]
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

        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("pattern is required"))?;

        let replacement = args
            .get("replacement")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("replacement is required"))?;

        let use_regex = args
            .get("regex")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let replace_all = args
            .get("all")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let case_insensitive = args
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let (result, count) = if use_regex {
            let pattern_str = if case_insensitive {
                format!("(?i){}", pattern)
            } else {
                pattern.to_string()
            };

            let re = regex::Regex::new(&pattern_str)
                .map_err(|e| crate::error::AgentError::tool_execution(format!("Invalid regex: {}", e)))?;

            let matches = re.find_iter(input).count();

            let replaced = if replace_all {
                re.replace_all(input, replacement).to_string()
            } else {
                re.replace(input, replacement).to_string()
            };

            (replaced, matches)
        } else {
            let search = if case_insensitive {
                input.to_lowercase()
            } else {
                input.to_string()
            };

            let pattern_lower = if case_insensitive {
                pattern.to_lowercase()
            } else {
                pattern.to_string()
            };

            let count = search.matches(&pattern_lower).count();

            let result = if case_insensitive {
                // Case insensitive literal replacement
                let mut result = input.to_string();
                let mut start_idx = 0;
                while let Some(pos) = result[start_idx..].to_lowercase().find(&pattern_lower) {
                    let actual_pos = start_idx + pos;
                    result = format!(
                        "{}{}{}",
                        &result[..actual_pos],
                        replacement,
                        &result[actual_pos + pattern.len()..]
                    );
                    if !replace_all {
                        break;
                    }
                    start_idx = actual_pos + replacement.len();
                }
                result
            } else if replace_all {
                input.replace(pattern, replacement)
            } else {
                input.replacen(pattern, replacement, 1)
            };

            (result, count)
        };

        let duration = start.elapsed();

        debug!("Replace: {} occurrences", count);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "original": input,
                "replacements": if replace_all { count } else { count.min(1) },
                "pattern": pattern,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for trimming and padding strings.
pub struct TrimPadTool;

impl TrimPadTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TrimPadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for TrimPadTool {
    fn name(&self) -> &str {
        "trim_pad"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "trim_pad".to_string(),
            description: "Trim whitespace or pad string to a specific length.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "The input string"
                    },
                    "operation": {
                        "type": "string",
                        "enum": ["trim", "trim_start", "trim_end", "pad_start", "pad_end", "center"],
                        "description": "Operation to perform"
                    },
                    "length": {
                        "type": "integer",
                        "description": "Target length (for pad operations)"
                    },
                    "char": {
                        "type": "string",
                        "default": " ",
                        "description": "Padding character"
                    }
                },
                "required": ["input", "operation"]
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
            .ok_or_else(|| crate::error::AgentError::tool_execution("operation is required"))?;

        let pad_char = args
            .get("char")
            .and_then(|v| v.as_str())
            .and_then(|s| s.chars().next())
            .unwrap_or(' ');

        let length = args.get("length").and_then(|v| v.as_u64()).map(|n| n as usize);

        let result = match operation {
            "trim" => input.trim().to_string(),
            "trim_start" => input.trim_start().to_string(),
            "trim_end" => input.trim_end().to_string(),
            "pad_start" => {
                let len = length.ok_or_else(|| crate::error::AgentError::tool_execution("length is required for padding"))?;
                if input.len() >= len {
                    input.to_string()
                } else {
                    let padding: String = std::iter::repeat(pad_char).take(len - input.len()).collect();
                    format!("{}{}", padding, input)
                }
            }
            "pad_end" => {
                let len = length.ok_or_else(|| crate::error::AgentError::tool_execution("length is required for padding"))?;
                if input.len() >= len {
                    input.to_string()
                } else {
                    let padding: String = std::iter::repeat(pad_char).take(len - input.len()).collect();
                    format!("{}{}", input, padding)
                }
            }
            "center" => {
                let len = length.ok_or_else(|| crate::error::AgentError::tool_execution("length is required for center"))?;
                if input.len() >= len {
                    input.to_string()
                } else {
                    let total_pad = len - input.len();
                    let left_pad = total_pad / 2;
                    let right_pad = total_pad - left_pad;
                    let left: String = std::iter::repeat(pad_char).take(left_pad).collect();
                    let right: String = std::iter::repeat(pad_char).take(right_pad).collect();
                    format!("{}{}{}", left, input, right)
                }
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown operation: {}", operation),
                ));
            }
        };

        let duration = start.elapsed();

        debug!("Trim/Pad: {} -> {} chars", input.len(), result.len());

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "original_length": input.len(),
                "result_length": result.len(),
                "operation": operation,
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
    fn test_case_tool_creation() {
        let tool = CaseTool::new();
        assert_eq!(tool.name(), "case");
    }

    #[test]
    fn test_split_join_tool_creation() {
        let tool = SplitJoinTool::new();
        assert_eq!(tool.name(), "split_join");
    }

    #[test]
    fn test_replace_tool_creation() {
        let tool = ReplaceTool::new();
        assert_eq!(tool.name(), "replace");
    }

    #[test]
    fn test_trim_pad_tool_creation() {
        let tool = TrimPadTool::new();
        assert_eq!(tool.name(), "trim_pad");
    }

    #[tokio::test]
    async fn test_case_upper() {
        let tool = CaseTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "hello world",
                    "to": "upper"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("HELLO WORLD")
        );
    }

    #[tokio::test]
    async fn test_case_snake() {
        let tool = CaseTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "helloWorld",
                    "to": "snake"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("hello_world")
        );
    }

    #[tokio::test]
    async fn test_case_camel() {
        let tool = CaseTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "hello_world",
                    "to": "camel"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("helloWorld")
        );
    }

    #[tokio::test]
    async fn test_split() {
        let tool = SplitJoinTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "operation": "split",
                    "input": "a,b,c",
                    "delimiter": ","
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("count").and_then(|v| v.as_u64()),
            Some(3)
        );
    }

    #[tokio::test]
    async fn test_join() {
        let tool = SplitJoinTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "operation": "join",
                    "parts": ["a", "b", "c"],
                    "delimiter": "-"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("a-b-c")
        );
    }

    #[tokio::test]
    async fn test_replace() {
        let tool = ReplaceTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "hello world world",
                    "pattern": "world",
                    "replacement": "universe"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("hello universe universe")
        );
    }

    #[tokio::test]
    async fn test_replace_regex() {
        let tool = ReplaceTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "abc123def456",
                    "pattern": "\\d+",
                    "replacement": "X",
                    "regex": true
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("abcXdefX")
        );
    }

    #[tokio::test]
    async fn test_trim() {
        let tool = TrimPadTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "  hello  ",
                    "operation": "trim"
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
    async fn test_pad_start() {
        let tool = TrimPadTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "42",
                    "operation": "pad_start",
                    "length": 5,
                    "char": "0"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("00042")
        );
    }
}
