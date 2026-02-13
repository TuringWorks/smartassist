//! Jupyter notebook editing tools.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::debug;

/// Tool for editing Jupyter notebooks.
pub struct NotebookEditTool;

impl NotebookEditTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NotebookEditTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Edit mode for notebook cells.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum EditMode {
    /// Replace cell content.
    Replace,
    /// Insert a new cell.
    Insert,
    /// Delete a cell.
    Delete,
}

#[async_trait]
impl Tool for NotebookEditTool {
    fn name(&self) -> &str {
        "notebook_edit"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "notebook_edit".to_string(),
            description: "Edit a Jupyter notebook cell. Can replace, insert, or delete cells."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "notebook_path": {
                        "type": "string",
                        "description": "Absolute path to the notebook file"
                    },
                    "cell_id": {
                        "type": "string",
                        "description": "ID of the cell to edit (for replace/delete)"
                    },
                    "cell_type": {
                        "type": "string",
                        "enum": ["code", "markdown"],
                        "description": "Type of cell (required for insert)"
                    },
                    "edit_mode": {
                        "type": "string",
                        "enum": ["replace", "insert", "delete"],
                        "default": "replace",
                        "description": "Edit operation to perform"
                    },
                    "new_source": {
                        "type": "string",
                        "description": "New content for the cell"
                    }
                },
                "required": ["notebook_path", "new_source"]
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
        let start = Instant::now();

        let notebook_path = args
            .get("notebook_path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("notebook_path is required"))?;

        let new_source = args
            .get("new_source")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let edit_mode = args
            .get("edit_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("replace");

        let cell_id = args.get("cell_id").and_then(|v| v.as_str());

        let cell_type = args
            .get("cell_type")
            .and_then(|v| v.as_str())
            .unwrap_or("code");

        // Resolve path
        let path = if std::path::Path::new(notebook_path).is_absolute() {
            std::path::PathBuf::from(notebook_path)
        } else {
            ctx.cwd.join(notebook_path)
        };

        debug!(
            "Notebook edit: path={}, mode={}, cell_id={:?}",
            path.display(),
            edit_mode,
            cell_id
        );

        // Read notebook
        let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
            crate::error::AgentError::tool_execution(format!("Failed to read notebook: {}", e))
        })?;

        let mut notebook: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| {
                crate::error::AgentError::tool_execution(format!("Invalid notebook JSON: {}", e))
            })?;

        let cells = notebook
            .get_mut("cells")
            .and_then(|c| c.as_array_mut())
            .ok_or_else(|| crate::error::AgentError::tool_execution("Invalid notebook structure"))?;

        match edit_mode {
            "replace" => {
                // Find cell by ID or index
                let cell_index = if let Some(id) = cell_id {
                    cells.iter().position(|c| {
                        c.get("id").and_then(|v| v.as_str()) == Some(id)
                    })
                } else {
                    Some(0)
                };

                if let Some(idx) = cell_index {
                    if idx < cells.len() {
                        let source_lines: Vec<String> =
                            new_source.lines().map(|l| format!("{}\n", l)).collect();
                        cells[idx]["source"] = serde_json::json!(source_lines);
                    }
                }
            }
            "insert" => {
                let new_cell = serde_json::json!({
                    "cell_type": cell_type,
                    "source": new_source.lines().map(|l| format!("{}\n", l)).collect::<Vec<_>>(),
                    "metadata": {},
                    "id": uuid::Uuid::new_v4().to_string()
                });

                // Insert after cell_id or at end
                let insert_pos = if let Some(id) = cell_id {
                    cells
                        .iter()
                        .position(|c| c.get("id").and_then(|v| v.as_str()) == Some(id))
                        .map(|i| i + 1)
                        .unwrap_or(cells.len())
                } else {
                    cells.len()
                };

                cells.insert(insert_pos, new_cell);
            }
            "delete" => {
                if let Some(id) = cell_id {
                    cells.retain(|c| c.get("id").and_then(|v| v.as_str()) != Some(id));
                }
            }
            _ => {
                return Ok(ToolResult::error(
                    tool_use_id,
                    format!("Unknown edit mode: {}", edit_mode),
                ));
            }
        }

        // Get cell count before dropping the mutable borrow
        let cell_count = cells.len();

        // Write back
        let output = serde_json::to_string_pretty(&notebook).map_err(|e| {
            crate::error::AgentError::tool_execution(format!("Failed to serialize notebook: {}", e))
        })?;

        tokio::fs::write(&path, output).await.map_err(|e| {
            crate::error::AgentError::tool_execution(format!("Failed to write notebook: {}", e))
        })?;

        let duration = start.elapsed();

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "success": true,
                "path": path.display().to_string(),
                "edit_mode": edit_mode,
                "cell_count": cell_count
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
    fn test_notebook_tool_definition() {
        let tool = NotebookEditTool::new();
        let def = tool.definition();

        assert_eq!(def.name, "notebook_edit");
        assert!(def.description.contains("Jupyter"));
    }
}
