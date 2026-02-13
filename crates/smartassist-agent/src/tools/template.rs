//! Template and text formatting tools.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use regex::Regex;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

/// Tool for simple template string substitution.
pub struct TemplateTool;

impl TemplateTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TemplateTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct TemplateArgs {
    /// Template string with {{variable}} placeholders
    template: String,
    /// Variables to substitute
    variables: HashMap<String, serde_json::Value>,
}

#[async_trait]
impl Tool for TemplateTool {
    fn name(&self) -> &str {
        "template"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "template".to_string(),
            description: "Substitute variables in a template string".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "template": {
                        "type": "string",
                        "description": "Template string with {{variable}} placeholders"
                    },
                    "variables": {
                        "type": "object",
                        "description": "Variables to substitute"
                    }
                },
                "required": ["template", "variables"]
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
        let args: TemplateArgs = serde_json::from_value(args)?;

        let re = Regex::new(r"\{\{(\w+)\}\}")
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Regex error: {}", e)))?;

        let mut result = args.template.clone();
        let mut substitutions = Vec::new();
        let mut missing = Vec::new();

        for cap in re.captures_iter(&args.template) {
            let var_name = &cap[1];
            if let Some(value) = args.variables.get(var_name) {
                let value_str = match value {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    serde_json::Value::Bool(b) => b.to_string(),
                    serde_json::Value::Null => "null".to_string(),
                    _ => serde_json::to_string(value).unwrap_or_default(),
                };
                result = result.replace(&format!("{{{{{}}}}}", var_name), &value_str);
                substitutions.push(var_name.to_string());
            } else {
                missing.push(var_name.to_string());
            }
        }

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "result": result,
                "substitutions": substitutions,
                "missing": missing
            }),
        ).with_duration(start.elapsed()))
    }
}

/// Tool for formatting strings.
pub struct FormatTool;

impl FormatTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FormatTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct FormatArgs {
    /// Value to format
    value: serde_json::Value,
    /// Format type
    format_type: String,
    /// Additional options
    #[serde(default)]
    options: Option<serde_json::Value>,
}

#[async_trait]
impl Tool for FormatTool {
    fn name(&self) -> &str {
        "format"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "format".to_string(),
            description: "Format a value in different ways".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "value": {
                        "description": "Value to format"
                    },
                    "format_type": {
                        "type": "string",
                        "enum": ["json", "json_pretty", "number", "percent", "bytes", "duration"],
                        "description": "Format type"
                    },
                    "options": {
                        "type": "object",
                        "description": "Additional formatting options"
                    }
                },
                "required": ["value", "format_type"]
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
        let args: FormatArgs = serde_json::from_value(args)?;

        let formatted = match args.format_type.as_str() {
            "json" => {
                serde_json::to_string(&args.value)
                    .map_err(|e| crate::error::AgentError::tool_execution(format!("JSON error: {}", e)))?
            }
            "json_pretty" => {
                serde_json::to_string_pretty(&args.value)
                    .map_err(|e| crate::error::AgentError::tool_execution(format!("JSON error: {}", e)))?
            }
            "number" => {
                let num = args.value.as_f64()
                    .ok_or_else(|| crate::error::AgentError::tool_execution("Value must be a number".to_string()))?;
                let decimals = args.options
                    .as_ref()
                    .and_then(|o| o.get("decimals"))
                    .and_then(|d| d.as_u64())
                    .unwrap_or(2) as usize;
                format!("{:.prec$}", num, prec = decimals)
            }
            "percent" => {
                let num = args.value.as_f64()
                    .ok_or_else(|| crate::error::AgentError::tool_execution("Value must be a number".to_string()))?;
                let decimals = args.options
                    .as_ref()
                    .and_then(|o| o.get("decimals"))
                    .and_then(|d| d.as_u64())
                    .unwrap_or(1) as usize;
                format!("{:.prec$}%", num * 100.0, prec = decimals)
            }
            "bytes" => {
                let bytes = args.value.as_u64()
                    .ok_or_else(|| crate::error::AgentError::tool_execution("Value must be a number".to_string()))?;
                format_bytes(bytes)
            }
            "duration" => {
                let ms = args.value.as_u64()
                    .ok_or_else(|| crate::error::AgentError::tool_execution("Value must be a number".to_string()))?;
                format_duration(ms)
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown format type: {}", args.format_type),
                ));
            }
        };

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "formatted": formatted,
                "format_type": args.format_type
            }),
        ).with_duration(start.elapsed()))
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.2}s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{}m {}s", mins, secs)
    } else {
        let hours = ms / 3_600_000;
        let mins = (ms % 3_600_000) / 60_000;
        format!("{}h {}m", hours, mins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_template_substitution() {
        let tool = TemplateTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "template": "Hello, {{name}}! You have {{count}} messages.",
                "variables": {
                    "name": "World",
                    "count": 42
                }
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert_eq!(output["result"], "Hello, World! You have 42 messages.");
    }

    #[tokio::test]
    async fn test_template_missing_variable() {
        let tool = TemplateTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "template": "Hello, {{name}}! Your ID is {{id}}.",
                "variables": {
                    "name": "User"
                }
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(output["missing"].as_array().unwrap().contains(&json!("id")));
    }

    #[tokio::test]
    async fn test_format_bytes() {
        let tool = FormatTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "value": 1536,
                "format_type": "bytes"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert_eq!(output["formatted"], "1.50 KB");
    }

    #[tokio::test]
    async fn test_format_percent() {
        let tool = FormatTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "value": 0.856,
                "format_type": "percent"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert_eq!(output["formatted"], "85.6%");
    }

    #[tokio::test]
    async fn test_format_duration() {
        let tool = FormatTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "value": 125000,
                "format_type": "duration"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert_eq!(output["formatted"], "2m 5s");
    }
}
