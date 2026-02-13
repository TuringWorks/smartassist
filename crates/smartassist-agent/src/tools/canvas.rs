//! Canvas tool.
//!
//! - [`CanvasTool`] - Control node canvases (present/hide/navigate/eval/snapshot/A2UI)

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Canvas tool - Control node canvases.
pub struct CanvasTool {
    /// Default timeout in milliseconds.
    timeout_ms: u64,
}

impl Default for CanvasTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CanvasTool {
    pub fn new() -> Self {
        Self {
            timeout_ms: 20_000,
        }
    }

    /// Set default timeout.
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

#[async_trait]
impl Tool for CanvasTool {
    fn name(&self) -> &str {
        "canvas"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "canvas".to_string(),
            description: "Control node canvases (present/hide/navigate/eval/snapshot/A2UI). Use snapshot to capture the rendered UI.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["present", "hide", "navigate", "eval", "snapshot", "a2ui_push", "a2ui_reset"],
                        "description": "Canvas action to perform"
                    },
                    "node": {
                        "type": "string",
                        "description": "Node id, name, or IP to target"
                    },
                    "gatewayUrl": {
                        "type": "string",
                        "description": "Gateway URL override"
                    },
                    "gatewayToken": {
                        "type": "string",
                        "description": "Gateway token override"
                    },
                    "timeoutMs": {
                        "type": "integer",
                        "description": "Action timeout in milliseconds"
                    },
                    "target": {
                        "type": "string",
                        "description": "Target URL/path to load (for 'present' action)"
                    },
                    "x": {
                        "type": "number",
                        "description": "Placement x coordinate in pixels (for 'present')"
                    },
                    "y": {
                        "type": "number",
                        "description": "Placement y coordinate in pixels (for 'present')"
                    },
                    "width": {
                        "type": "number",
                        "description": "Placement width in pixels (for 'present')"
                    },
                    "height": {
                        "type": "number",
                        "description": "Placement height in pixels (for 'present')"
                    },
                    "url": {
                        "type": "string",
                        "description": "URL to navigate to (for 'navigate' action)"
                    },
                    "javaScript": {
                        "type": "string",
                        "description": "JavaScript code to evaluate (for 'eval' action)"
                    },
                    "outputFormat": {
                        "type": "string",
                        "enum": ["png", "jpg", "jpeg"],
                        "description": "Image format for snapshot (default: png)"
                    },
                    "maxWidth": {
                        "type": "number",
                        "description": "Maximum width in pixels (for 'snapshot')"
                    },
                    "quality": {
                        "type": "number",
                        "description": "JPEG quality 0-1 (for 'snapshot')"
                    },
                    "delayMs": {
                        "type": "integer",
                        "description": "Delay before capture in ms (for 'snapshot')"
                    },
                    "jsonl": {
                        "type": "string",
                        "description": "A2UI JSONL payload (for 'a2ui_push')"
                    },
                    "jsonlPath": {
                        "type": "string",
                        "description": "Path to A2UI JSONL file (for 'a2ui_push')"
                    }
                },
                "required": ["action", "node"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        _context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'action' argument"))?;

        let node = args
            .get("node")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'node' argument"))?;

        let _timeout = args
            .get("timeoutMs")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.timeout_ms);

        debug!("Canvas action: {} on node: {}", action, node);

        // TODO: Implement actual gateway invocation (node.invoke with canvas.* commands)
        let result = match action {
            "present" => {
                let target = args.get("target").and_then(|v| v.as_str());
                let x = args.get("x").and_then(|v| v.as_f64());
                let y = args.get("y").and_then(|v| v.as_f64());
                let width = args.get("width").and_then(|v| v.as_f64());
                let height = args.get("height").and_then(|v| v.as_f64());

                let mut placement = serde_json::Map::new();
                if let Some(v) = x { placement.insert("x".into(), v.into()); }
                if let Some(v) = y { placement.insert("y".into(), v.into()); }
                if let Some(v) = width { placement.insert("width".into(), v.into()); }
                if let Some(v) = height { placement.insert("height".into(), v.into()); }

                serde_json::json!({
                    "action": "present",
                    "node": node,
                    "command": "canvas.present",
                    "target": target,
                    "placement": placement,
                    "success": false,
                    "message": "Canvas gateway invocation not yet implemented"
                })
            }
            "hide" => {
                serde_json::json!({
                    "action": "hide",
                    "node": node,
                    "command": "canvas.hide",
                    "success": false,
                    "message": "Canvas gateway invocation not yet implemented"
                })
            }
            "navigate" => {
                let url = args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'url' for navigate"))?;

                serde_json::json!({
                    "action": "navigate",
                    "node": node,
                    "command": "canvas.navigate",
                    "url": url,
                    "success": false,
                    "message": "Canvas gateway invocation not yet implemented"
                })
            }
            "eval" => {
                let js = args
                    .get("javaScript")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'javaScript' for eval"))?;

                serde_json::json!({
                    "action": "eval",
                    "node": node,
                    "command": "canvas.eval",
                    "script_length": js.len(),
                    "result": null,
                    "success": false,
                    "message": "Canvas gateway invocation not yet implemented"
                })
            }
            "snapshot" => {
                let output_format = args
                    .get("outputFormat")
                    .and_then(|v| v.as_str())
                    .unwrap_or("png");

                let format = match output_format {
                    "jpg" | "jpeg" => "jpeg",
                    _ => "png",
                };

                let max_width = args.get("maxWidth").and_then(|v| v.as_f64());
                let quality = args.get("quality").and_then(|v| v.as_f64());
                let delay_ms = args.get("delayMs").and_then(|v| v.as_u64());

                serde_json::json!({
                    "action": "snapshot",
                    "node": node,
                    "command": "canvas.snapshot",
                    "format": format,
                    "maxWidth": max_width,
                    "quality": quality,
                    "delayMs": delay_ms,
                    "success": false,
                    "message": "Canvas gateway invocation not yet implemented"
                })
            }
            "a2ui_push" => {
                let jsonl = args.get("jsonl").and_then(|v| v.as_str());
                let jsonl_path = args.get("jsonlPath").and_then(|v| v.as_str());

                if jsonl.is_none() && jsonl_path.is_none() {
                    return Err(AgentError::tool_execution(
                        "Either 'jsonl' or 'jsonlPath' is required for a2ui_push",
                    ));
                }

                serde_json::json!({
                    "action": "a2ui_push",
                    "node": node,
                    "command": "canvas.a2ui.pushJSONL",
                    "has_jsonl": jsonl.is_some(),
                    "has_jsonl_path": jsonl_path.is_some(),
                    "success": false,
                    "message": "Canvas gateway invocation not yet implemented"
                })
            }
            "a2ui_reset" => {
                serde_json::json!({
                    "action": "a2ui_reset",
                    "node": node,
                    "command": "canvas.a2ui.reset",
                    "success": false,
                    "message": "Canvas gateway invocation not yet implemented"
                })
            }
            _ => {
                return Err(AgentError::tool_execution(format!(
                    "Unknown canvas action: {}",
                    action
                )));
            }
        };

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Ui
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_tool_creation() {
        let tool = CanvasTool::new();
        assert_eq!(tool.name(), "canvas");
        assert_eq!(tool.timeout_ms, 20_000);
    }

    #[test]
    fn test_canvas_tool_timeout() {
        let tool = CanvasTool::new().with_timeout(60_000);
        assert_eq!(tool.timeout_ms, 60_000);
    }

    #[test]
    fn test_canvas_tool_group() {
        let tool = CanvasTool::new();
        assert_eq!(tool.group(), ToolGroup::Ui);
    }

    #[test]
    fn test_canvas_tool_definition() {
        let tool = CanvasTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "canvas");

        let actions = def.input_schema["properties"]["action"]["enum"]
            .as_array()
            .unwrap();
        assert_eq!(actions.len(), 7);
    }

    #[tokio::test]
    async fn test_canvas_missing_action() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute("test", serde_json::json!({"node": "test-node"}), &ctx)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_canvas_missing_node() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute("test", serde_json::json!({"action": "present"}), &ctx)
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_canvas_present() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test",
                serde_json::json!({
                    "action": "present",
                    "node": "my-ipad",
                    "target": "https://example.com",
                    "x": 0,
                    "y": 0,
                    "width": 1024,
                    "height": 768
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.output["action"], "present");
        assert_eq!(result.output["command"], "canvas.present");
    }

    #[tokio::test]
    async fn test_canvas_navigate_requires_url() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test",
                serde_json::json!({"action": "navigate", "node": "n1"}),
                &ctx,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_canvas_eval_requires_javascript() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test",
                serde_json::json!({"action": "eval", "node": "n1"}),
                &ctx,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_canvas_snapshot_default_format() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test",
                serde_json::json!({"action": "snapshot", "node": "n1"}),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.output["format"], "png");
    }

    #[tokio::test]
    async fn test_canvas_snapshot_jpeg_format() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test",
                serde_json::json!({"action": "snapshot", "node": "n1", "outputFormat": "jpg"}),
                &ctx,
            )
            .await
            .unwrap();

        assert_eq!(result.output["format"], "jpeg");
    }

    #[tokio::test]
    async fn test_canvas_a2ui_push_requires_content() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test",
                serde_json::json!({"action": "a2ui_push", "node": "n1"}),
                &ctx,
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_canvas_a2ui_push_with_jsonl() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test",
                serde_json::json!({
                    "action": "a2ui_push",
                    "node": "n1",
                    "jsonl": "{\"type\":\"text\",\"text\":\"hello\"}"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.output["has_jsonl"], true);
    }

    #[tokio::test]
    async fn test_canvas_a2ui_reset() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test",
                serde_json::json!({"action": "a2ui_reset", "node": "n1"}),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.output["command"], "canvas.a2ui.reset");
    }

    #[tokio::test]
    async fn test_canvas_unknown_action() {
        let tool = CanvasTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test",
                serde_json::json!({"action": "unknown", "node": "n1"}),
                &ctx,
            )
            .await;

        assert!(result.is_err());
    }
}
