//! JSON and YAML tools for data manipulation.
//!
//! Provides tools for parsing, transforming, and querying
//! JSON and YAML data structures.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Tool for querying JSON data using JSONPath-like expressions.
pub struct JsonQueryTool;

impl JsonQueryTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonQueryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for JsonQueryTool {
    fn name(&self) -> &str {
        "json_query"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "json_query".to_string(),
            description: "Query JSON data using path expressions (e.g., '.items[0].name')."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "json": {
                        "type": "string",
                        "description": "JSON string to query"
                    },
                    "path": {
                        "type": "string",
                        "description": "Path expression (e.g., '.items[0].name', '.users[*].email')"
                    }
                },
                "required": ["json", "path"]
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

        let json_str = args
            .get("json")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("json is required"))?;

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("path is required"))?;

        // Parse JSON
        let json: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Invalid JSON: {}", e)))?;

        // Simple path query implementation
        let result = query_json(&json, path)?;

        let duration = start.elapsed();

        debug!("JSON query completed: path={}", path);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
                "path": path,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Query JSON value by path expression.
fn query_json(json: &serde_json::Value, path: &str) -> Result<serde_json::Value> {
    let path = path.trim();

    // Handle root path
    if path == "." || path.is_empty() {
        return Ok(json.clone());
    }

    // Remove leading dot if present
    let path = path.strip_prefix('.').unwrap_or(path);

    let mut current = json;
    let parts = parse_path(path);

    for part in parts {
        match part {
            PathPart::Key(key) => {
                current = current.get(&key).ok_or_else(|| {
                    crate::error::AgentError::tool_execution(format!("Key not found: {}", key))
                })?;
            }
            PathPart::Index(idx) => {
                current = current.get(idx).ok_or_else(|| {
                    crate::error::AgentError::tool_execution(format!("Index out of bounds: {}", idx))
                })?;
            }
            PathPart::Wildcard => {
                // Return all elements of array
                if let Some(arr) = current.as_array() {
                    return Ok(serde_json::Value::Array(arr.clone()));
                } else {
                    return Err(crate::error::AgentError::tool_execution(
                        "Wildcard can only be used on arrays",
                    ));
                }
            }
        }
    }

    Ok(current.clone())
}

enum PathPart {
    Key(String),
    Index(usize),
    Wildcard,
}

fn parse_path(path: &str) -> Vec<PathPart> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_bracket = false;

    for ch in path.chars() {
        match ch {
            '[' => {
                if !current.is_empty() {
                    parts.push(PathPart::Key(current.clone()));
                    current.clear();
                }
                in_bracket = true;
            }
            ']' => {
                if in_bracket {
                    let content = current.trim();
                    if content == "*" {
                        parts.push(PathPart::Wildcard);
                    } else if let Ok(idx) = content.parse::<usize>() {
                        parts.push(PathPart::Index(idx));
                    } else {
                        // Quoted key
                        let key = content.trim_matches(|c| c == '\'' || c == '"');
                        parts.push(PathPart::Key(key.to_string()));
                    }
                    current.clear();
                    in_bracket = false;
                }
            }
            '.' if !in_bracket => {
                if !current.is_empty() {
                    parts.push(PathPart::Key(current.clone()));
                    current.clear();
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        parts.push(PathPart::Key(current));
    }

    parts
}

/// Tool for transforming JSON data.
pub struct JsonTransformTool;

impl JsonTransformTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonTransformTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for JsonTransformTool {
    fn name(&self) -> &str {
        "json_transform"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "json_transform".to_string(),
            description: "Transform JSON data by picking, omitting, or renaming fields.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "json": {
                        "type": "string",
                        "description": "JSON string to transform"
                    },
                    "pick": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Fields to keep (picks only these fields)"
                    },
                    "omit": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Fields to remove"
                    },
                    "rename": {
                        "type": "object",
                        "additionalProperties": {"type": "string"},
                        "description": "Fields to rename (old_name: new_name)"
                    },
                    "flatten": {
                        "type": "boolean",
                        "default": false,
                        "description": "Flatten nested objects"
                    }
                },
                "required": ["json"]
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

        let json_str = args
            .get("json")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("json is required"))?;

        let pick: Option<Vec<String>> = args
            .get("pick")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        let omit: Option<Vec<String>> = args
            .get("omit")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

        let rename: Option<std::collections::HashMap<String, String>> = args
            .get("rename")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            });

        let flatten = args
            .get("flatten")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Parse JSON
        let json: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Invalid JSON: {}", e)))?;

        // Transform
        let result = transform_json(json, pick.as_deref(), omit.as_deref(), rename.as_ref(), flatten)?;

        let duration = start.elapsed();

        debug!("JSON transform completed");

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "result": result,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

fn transform_json(
    mut json: serde_json::Value,
    pick: Option<&[String]>,
    omit: Option<&[String]>,
    rename: Option<&std::collections::HashMap<String, String>>,
    flatten: bool,
) -> Result<serde_json::Value> {
    // Only transform objects at the top level
    if let Some(obj) = json.as_object_mut() {
        // Pick fields
        if let Some(fields) = pick {
            let picked: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .filter(|(k, _)| fields.contains(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            *obj = picked;
        }

        // Omit fields
        if let Some(fields) = omit {
            for field in fields {
                obj.remove(field);
            }
        }

        // Rename fields
        if let Some(renames) = rename {
            for (old_name, new_name) in renames {
                if let Some(value) = obj.remove(old_name) {
                    obj.insert(new_name.clone(), value);
                }
            }
        }

        // Flatten nested objects
        if flatten {
            let flattened = flatten_object(obj, "");
            *obj = flattened;
        }

        Ok(serde_json::Value::Object(obj.clone()))
    } else if let Some(arr) = json.as_array() {
        // Transform each element in array
        let transformed: Vec<serde_json::Value> = arr
            .iter()
            .map(|item| transform_json(item.clone(), pick, omit, rename, flatten))
            .collect::<Result<Vec<_>>>()?;
        Ok(serde_json::Value::Array(transformed))
    } else {
        Ok(json)
    }
}

fn flatten_object(obj: &serde_json::Map<String, serde_json::Value>, prefix: &str) -> serde_json::Map<String, serde_json::Value> {
    let mut result = serde_json::Map::new();

    for (key, value) in obj {
        let new_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        if let Some(nested) = value.as_object() {
            let nested_flat = flatten_object(nested, &new_key);
            for (nk, nv) in nested_flat {
                result.insert(nk, nv);
            }
        } else {
            result.insert(new_key, value.clone());
        }
    }

    result
}

/// Tool for parsing and converting YAML.
pub struct YamlTool;

impl YamlTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for YamlTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for YamlTool {
    fn name(&self) -> &str {
        "yaml"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "yaml".to_string(),
            description: "Parse YAML to JSON or convert JSON to YAML.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "YAML or JSON string to convert"
                    },
                    "to_format": {
                        "type": "string",
                        "enum": ["json", "yaml"],
                        "default": "json",
                        "description": "Output format"
                    },
                    "pretty": {
                        "type": "boolean",
                        "default": true,
                        "description": "Pretty print the output"
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

        let to_format = args
            .get("to_format")
            .and_then(|v| v.as_str())
            .unwrap_or("json");

        let pretty = args
            .get("pretty")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Try to parse as YAML first (which also handles JSON)
        let value: serde_json::Value = serde_yaml::from_str(input)
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Parse error: {}", e)))?;

        // Convert to output format
        let output = match to_format {
            "yaml" => {
                serde_yaml::to_string(&value)
                    .map_err(|e| crate::error::AgentError::tool_execution(format!("YAML error: {}", e)))?
            }
            _ => {
                if pretty {
                    serde_json::to_string_pretty(&value)
                        .map_err(|e| crate::error::AgentError::tool_execution(format!("JSON error: {}", e)))?
                } else {
                    serde_json::to_string(&value)
                        .map_err(|e| crate::error::AgentError::tool_execution(format!("JSON error: {}", e)))?
                }
            }
        };

        let duration = start.elapsed();

        debug!("YAML conversion completed: to_format={}", to_format);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "output": output,
                "format": to_format,
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
    fn test_json_query_tool_creation() {
        let tool = JsonQueryTool::new();
        assert_eq!(tool.name(), "json_query");
    }

    #[test]
    fn test_json_transform_tool_creation() {
        let tool = JsonTransformTool::new();
        assert_eq!(tool.name(), "json_transform");
    }

    #[test]
    fn test_yaml_tool_creation() {
        let tool = YamlTool::new();
        assert_eq!(tool.name(), "yaml");
    }

    #[tokio::test]
    async fn test_json_query_simple() {
        let tool = JsonQueryTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "json": r#"{"name": "test", "value": 42}"#,
                    "path": ".name"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_str()),
            Some("test")
        );
    }

    #[tokio::test]
    async fn test_json_query_array() {
        let tool = JsonQueryTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "json": r#"{"items": [{"id": 1}, {"id": 2}]}"#,
                    "path": ".items[0].id"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(
            result.output.get("result").and_then(|v| v.as_u64()),
            Some(1)
        );
    }

    #[tokio::test]
    async fn test_json_transform_pick() {
        let tool = JsonTransformTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "json": r#"{"a": 1, "b": 2, "c": 3}"#,
                    "pick": ["a", "c"]
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let result_obj = result.output.get("result").unwrap();
        assert!(result_obj.get("a").is_some());
        assert!(result_obj.get("b").is_none());
        assert!(result_obj.get("c").is_some());
    }

    #[tokio::test]
    async fn test_json_transform_omit() {
        let tool = JsonTransformTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "json": r#"{"a": 1, "b": 2, "c": 3}"#,
                    "omit": ["b"]
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let result_obj = result.output.get("result").unwrap();
        assert!(result_obj.get("a").is_some());
        assert!(result_obj.get("b").is_none());
        assert!(result_obj.get("c").is_some());
    }

    #[tokio::test]
    async fn test_yaml_to_json() {
        let tool = YamlTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": "name: test\nvalue: 42",
                    "to_format": "json"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.get("output").is_some());
    }

    #[tokio::test]
    async fn test_json_to_yaml() {
        let tool = YamlTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "input": r#"{"name": "test", "value": 42}"#,
                    "to_format": "yaml"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.get("output").is_some());
    }
}
