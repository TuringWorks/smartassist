//! Environment variable tools.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;

/// Tool for getting environment variables.
pub struct EnvGetTool;

impl EnvGetTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnvGetTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct EnvGetArgs {
    /// Name of the environment variable
    name: String,
    /// Default value if not set
    default: Option<String>,
}

#[async_trait]
impl Tool for EnvGetTool {
    fn name(&self) -> &str {
        "env_get"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "env_get".to_string(),
            description: "Get an environment variable value".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "Name of the environment variable"
                    },
                    "default": {
                        "type": "string",
                        "description": "Default value if variable is not set"
                    }
                },
                "required": ["name"]
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
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: EnvGetArgs = serde_json::from_value(args)?;

        // First check the context env, then fall back to system env
        let value = context.env.get(&args.name)
            .cloned()
            .or_else(|| std::env::var(&args.name).ok())
            .or(args.default);

        match value {
            Some(val) => Ok(ToolResult::success(
                tool_use_id,
                json!({
                    "name": args.name,
                    "value": val,
                    "found": true
                }),
            ).with_duration(start.elapsed())),
            None => Ok(ToolResult::success(
                tool_use_id,
                json!({
                    "name": args.name,
                    "value": null,
                    "found": false
                }),
            ).with_duration(start.elapsed())),
        }
    }
}

/// Tool for listing environment variables.
pub struct EnvListTool;

impl EnvListTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnvListTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct EnvListArgs {
    /// Pattern to filter variable names (prefix match)
    #[serde(default)]
    prefix: Option<String>,
    /// Whether to include values (default: false for security)
    #[serde(default)]
    include_values: Option<bool>,
}

#[async_trait]
impl Tool for EnvListTool {
    fn name(&self) -> &str {
        "env_list"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "env_list".to_string(),
            description: "List environment variable names (optionally with values)".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "prefix": {
                        "type": "string",
                        "description": "Filter to variables starting with this prefix"
                    },
                    "include_values": {
                        "type": "boolean",
                        "description": "Include values (default: false for security)"
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
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: EnvListArgs = serde_json::from_value(args)?;
        let include_values = args.include_values.unwrap_or(false);

        // Collect from both context and system env
        let mut vars: Vec<_> = context.env.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        // Add system env vars not in context
        for (k, v) in std::env::vars() {
            if !context.env.contains_key(&k) {
                vars.push((k, v));
            }
        }

        // Filter by prefix if provided
        if let Some(ref prefix) = args.prefix {
            vars.retain(|(k, _)| k.starts_with(prefix));
        }

        // Sort by name
        vars.sort_by(|(a, _), (b, _)| a.cmp(b));

        let result = if include_values {
            json!({
                "count": vars.len(),
                "variables": vars.into_iter()
                    .map(|(k, v)| json!({"name": k, "value": v}))
                    .collect::<Vec<_>>()
            })
        } else {
            json!({
                "count": vars.len(),
                "names": vars.into_iter().map(|(k, _)| k).collect::<Vec<_>>()
            })
        };

        Ok(ToolResult::success(tool_use_id, result)
            .with_duration(start.elapsed()))
    }
}

/// Tool for checking if environment variables exist.
pub struct EnvCheckTool;

impl EnvCheckTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EnvCheckTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct EnvCheckArgs {
    /// Names of environment variables to check
    names: Vec<String>,
}

#[async_trait]
impl Tool for EnvCheckTool {
    fn name(&self) -> &str {
        "env_check"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "env_check".to_string(),
            description: "Check if environment variables are set".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "names": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Names of environment variables to check"
                    }
                },
                "required": ["names"]
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
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();
        let args: EnvCheckArgs = serde_json::from_value(args)?;

        let results: Vec<_> = args.names.iter()
            .map(|name| {
                let exists = context.env.contains_key(name) || std::env::var(name).is_ok();
                json!({
                    "name": name,
                    "exists": exists
                })
            })
            .collect();

        let all_exist = results.iter().all(|r| r["exists"].as_bool().unwrap_or(false));
        let missing: Vec<_> = results.iter()
            .filter(|r| !r["exists"].as_bool().unwrap_or(false))
            .filter_map(|r| r["name"].as_str())
            .collect();

        Ok(ToolResult::success(
            tool_use_id,
            json!({
                "all_exist": all_exist,
                "missing": missing,
                "results": results
            }),
        ).with_duration(start.elapsed()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_env_get_existing() {
        let tool = EnvGetTool::new();
        let context = ToolContext::default();

        // PATH should exist on most systems
        let result = tool.execute(
            "test",
            json!({
                "name": "PATH"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_env_get_with_default() {
        let tool = EnvGetTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "name": "NONEXISTENT_VAR_12345",
                "default": "default_value"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert_eq!(output["value"], "default_value");
    }

    #[tokio::test]
    async fn test_env_list() {
        let tool = EnvListTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "prefix": "PATH"
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_env_check() {
        let tool = EnvCheckTool::new();
        let context = ToolContext::default();

        let result = tool.execute(
            "test",
            json!({
                "names": ["PATH", "NONEXISTENT_VAR_12345"]
            }),
            &context,
        ).await.unwrap();

        assert!(!result.is_error);
        let output: serde_json::Value = serde_json::from_value(result.output).unwrap();
        assert!(!output["all_exist"].as_bool().unwrap());
        assert!(output["missing"].as_array().unwrap().contains(&json!("NONEXISTENT_VAR_12345")));
    }
}
