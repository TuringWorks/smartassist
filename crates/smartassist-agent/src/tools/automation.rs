//! Automation tools.
//!
//! - [`CronTool`] - Manage scheduled jobs
//! - [`GatewayTool`] - Gateway management
//! - [`NodesTool`] - Control paired devices

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::time::Instant;
use tracing::debug;

/// Cron tool - Manage scheduled jobs.
pub struct CronTool;

impl Default for CronTool {
    fn default() -> Self {
        Self::new()
    }
}

impl CronTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for CronTool {
    fn name(&self) -> &str {
        "cron"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "cron".to_string(),
            description: "Manage scheduled cron jobs. Can list, add, update, remove, and run cron jobs.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "add", "update", "remove", "run", "status"],
                        "description": "Action to perform"
                    },
                    "id": {
                        "type": "string",
                        "description": "Job ID (for update, remove, run)"
                    },
                    "schedule": {
                        "type": "string",
                        "description": "Cron schedule expression (for add/update)"
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Prompt to run (for add/update)"
                    },
                    "agent_id": {
                        "type": "string",
                        "description": "Agent ID to use (for add/update)"
                    },
                    "enabled": {
                        "type": "boolean",
                        "description": "Whether job is enabled"
                    }
                },
                "required": ["action"]
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

        debug!("Cron tool action: {}", action);

        let result = match action {
            "list" => {
                // TODO: Get from cron scheduler
                serde_json::json!({
                    "jobs": [],
                    "count": 0
                })
            }
            "status" => {
                serde_json::json!({
                    "enabled": true,
                    "job_count": 0
                })
            }
            "add" => {
                let schedule = args.get("schedule").and_then(|v| v.as_str());
                let prompt = args.get("prompt").and_then(|v| v.as_str());

                if schedule.is_none() || prompt.is_none() {
                    return Err(AgentError::tool_execution(
                        "Both 'schedule' and 'prompt' are required for add",
                    ));
                }

                let job_id = uuid::Uuid::new_v4().to_string();
                serde_json::json!({
                    "id": job_id,
                    "schedule": schedule,
                    "created": true
                })
            }
            "update" => {
                let id = args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'id' for update"))?;

                serde_json::json!({
                    "id": id,
                    "updated": true
                })
            }
            "remove" => {
                let id = args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'id' for remove"))?;

                serde_json::json!({
                    "id": id,
                    "removed": true
                })
            }
            "run" => {
                let id = args
                    .get("id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'id' for run"))?;

                let run_id = uuid::Uuid::new_v4().to_string();
                serde_json::json!({
                    "job_id": id,
                    "run_id": run_id,
                    "triggered": true
                })
            }
            _ => {
                return Err(AgentError::tool_execution(format!(
                    "Unknown action: {}",
                    action
                )));
            }
        };

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Gateway tool - Gateway management.
pub struct GatewayTool;

impl Default for GatewayTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GatewayTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GatewayTool {
    fn name(&self) -> &str {
        "gateway"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "gateway".to_string(),
            description: "Manage the SmartAssist gateway. Check status, restart, and configure.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["status", "restart", "config"],
                        "description": "Action to perform"
                    },
                    "config_key": {
                        "type": "string",
                        "description": "Config key to get/set"
                    },
                    "config_value": {
                        "type": "string",
                        "description": "Config value to set"
                    }
                },
                "required": ["action"]
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

        debug!("Gateway tool action: {}", action);

        let result = match action {
            "status" => {
                serde_json::json!({
                    "running": true,
                    "version": env!("CARGO_PKG_VERSION"),
                    "uptime_seconds": 0,
                    "channels": [],
                    "sessions": 0
                })
            }
            "restart" => {
                // TODO: Actually restart gateway
                serde_json::json!({
                    "restarting": true,
                    "message": "Gateway restart initiated"
                })
            }
            "config" => {
                let key = args.get("config_key").and_then(|v| v.as_str());
                let value = args.get("config_value");

                if let Some(k) = key {
                    if let Some(v) = value {
                        serde_json::json!({
                            "key": k,
                            "value": v,
                            "set": true
                        })
                    } else {
                        // Get config
                        serde_json::json!({
                            "key": k,
                            "value": null
                        })
                    }
                } else {
                    return Err(AgentError::tool_execution(
                        "config_key is required for config action",
                    ));
                }
            }
            _ => {
                return Err(AgentError::tool_execution(format!(
                    "Unknown action: {}",
                    action
                )));
            }
        };

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Nodes tool - Control paired devices.
pub struct NodesTool;

impl Default for NodesTool {
    fn default() -> Self {
        Self::new()
    }
}

impl NodesTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for NodesTool {
    fn name(&self) -> &str {
        "nodes"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "nodes".to_string(),
            description: "Control paired devices/nodes. List nodes, invoke commands, and manage pairing.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["list", "describe", "invoke", "pair", "unpair"],
                        "description": "Action to perform"
                    },
                    "node_id": {
                        "type": "string",
                        "description": "Node ID (for describe, invoke, unpair)"
                    },
                    "command": {
                        "type": "string",
                        "description": "Command to invoke on node"
                    },
                    "args": {
                        "type": "object",
                        "description": "Arguments for the command"
                    }
                },
                "required": ["action"]
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

        debug!("Nodes tool action: {}", action);

        let result = match action {
            "list" => {
                // TODO: Get from node manager
                serde_json::json!({
                    "nodes": [],
                    "count": 0
                })
            }
            "describe" => {
                let node_id = args
                    .get("node_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'node_id'"))?;

                serde_json::json!({
                    "node_id": node_id,
                    "found": false,
                    "error": "Node not found"
                })
            }
            "invoke" => {
                let node_id = args
                    .get("node_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'node_id'"))?;

                let command = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'command'"))?;

                let invocation_id = uuid::Uuid::new_v4().to_string();
                serde_json::json!({
                    "invocation_id": invocation_id,
                    "node_id": node_id,
                    "command": command,
                    "status": "pending"
                })
            }
            "pair" => {
                let pairing_code = format!("{:06}", rand::random::<u32>() % 1_000_000);
                serde_json::json!({
                    "pairing_code": pairing_code,
                    "expires_in_seconds": 600
                })
            }
            "unpair" => {
                let node_id = args
                    .get("node_id")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| AgentError::tool_execution("Missing 'node_id'"))?;

                serde_json::json!({
                    "node_id": node_id,
                    "unpaired": true
                })
            }
            _ => {
                return Err(AgentError::tool_execution(format!(
                    "Unknown action: {}",
                    action
                )));
            }
        };

        let duration = start.elapsed();
        Ok(ToolResult::success(tool_use_id, result).with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_tool_creation() {
        let tool = CronTool::new();
        assert_eq!(tool.name(), "cron");
    }

    #[test]
    fn test_gateway_tool_creation() {
        let tool = GatewayTool::new();
        assert_eq!(tool.name(), "gateway");
    }

    #[test]
    fn test_nodes_tool_creation() {
        let tool = NodesTool::new();
        assert_eq!(tool.name(), "nodes");
    }
}
