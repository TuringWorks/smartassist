//! Diff tools for comparing text and files.
//!
//! Provides tools for generating and viewing diffs between
//! text content or files.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use similar::{ChangeTag, TextDiff};
use std::path::PathBuf;
use std::time::Instant;
use tracing::debug;

/// Tool for generating diffs between text.
pub struct DiffTool;

impl DiffTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DiffTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for DiffTool {
    fn name(&self) -> &str {
        "diff"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "diff".to_string(),
            description: "Generate a diff between two pieces of text or files.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "old_text": {
                        "type": "string",
                        "description": "The original text"
                    },
                    "new_text": {
                        "type": "string",
                        "description": "The new text"
                    },
                    "old_file": {
                        "type": "string",
                        "description": "Path to the original file (alternative to old_text)"
                    },
                    "new_file": {
                        "type": "string",
                        "description": "Path to the new file (alternative to new_text)"
                    },
                    "context_lines": {
                        "type": "integer",
                        "default": 3,
                        "description": "Number of context lines around changes"
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
        ctx: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        // Get old content
        let old_content = if let Some(old_text) = args.get("old_text").and_then(|v| v.as_str()) {
            old_text.to_string()
        } else if let Some(old_file) = args.get("old_file").and_then(|v| v.as_str()) {
            let path = resolve_path(old_file, &ctx.cwd);
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to read old file: {}", e)))?
        } else {
            return Ok(ToolResult::error(
                tool_use_id,
                "Either old_text or old_file must be provided",
            ));
        };

        // Get new content
        let new_content = if let Some(new_text) = args.get("new_text").and_then(|v| v.as_str()) {
            new_text.to_string()
        } else if let Some(new_file) = args.get("new_file").and_then(|v| v.as_str()) {
            let path = resolve_path(new_file, &ctx.cwd);
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to read new file: {}", e)))?
        } else {
            return Ok(ToolResult::error(
                tool_use_id,
                "Either new_text or new_file must be provided",
            ));
        };

        let context_lines = args
            .get("context_lines")
            .and_then(|v| v.as_u64())
            .unwrap_or(3) as usize;

        // Generate diff
        let diff = TextDiff::from_lines(&old_content, &new_content);

        // Build unified diff output
        let mut unified_diff = String::new();
        let mut additions = 0;
        let mut deletions = 0;
        let mut changes: Vec<serde_json::Value> = Vec::new();

        for (idx, group) in diff.grouped_ops(context_lines).iter().enumerate() {
            if idx > 0 {
                unified_diff.push_str("...\n");
            }

            for op in group {
                for change in diff.iter_changes(op) {
                    let tag = match change.tag() {
                        ChangeTag::Delete => {
                            deletions += 1;
                            "-"
                        }
                        ChangeTag::Insert => {
                            additions += 1;
                            "+"
                        }
                        ChangeTag::Equal => " ",
                    };

                    let line_content = change.value();
                    unified_diff.push_str(&format!("{}{}", tag, line_content));

                    if change.tag() != ChangeTag::Equal {
                        changes.push(serde_json::json!({
                            "type": if change.tag() == ChangeTag::Insert { "add" } else { "delete" },
                            "old_line": change.old_index().map(|i| i + 1),
                            "new_line": change.new_index().map(|i| i + 1),
                            "content": line_content.trim_end(),
                        }));
                    }
                }
            }
        }

        let duration = start.elapsed();

        debug!(
            "Generated diff: {} additions, {} deletions",
            additions, deletions
        );

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "diff": unified_diff,
                "additions": additions,
                "deletions": deletions,
                "changes": changes,
                "has_changes": additions > 0 || deletions > 0,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for applying a patch/diff to text.
pub struct PatchTool;

impl PatchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PatchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for PatchTool {
    fn name(&self) -> &str {
        "patch"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "patch".to_string(),
            description: "Preview changes to text using search and replace.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The original text"
                    },
                    "file": {
                        "type": "string",
                        "description": "Path to the file (alternative to text)"
                    },
                    "search": {
                        "type": "string",
                        "description": "Text to search for"
                    },
                    "replace": {
                        "type": "string",
                        "description": "Replacement text"
                    },
                    "preview_only": {
                        "type": "boolean",
                        "default": true,
                        "description": "Only preview, don't apply changes"
                    }
                },
                "required": ["search", "replace"]
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

        let search = args
            .get("search")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("search is required"))?;

        let replace = args
            .get("replace")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::error::AgentError::tool_execution("replace is required"))?;

        let preview_only = args
            .get("preview_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        // Get content
        let (content, file_path) = if let Some(text) = args.get("text").and_then(|v| v.as_str()) {
            (text.to_string(), None)
        } else if let Some(file) = args.get("file").and_then(|v| v.as_str()) {
            let path = resolve_path(file, &ctx.cwd);
            let content = tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to read file: {}", e)))?;
            (content, Some(path))
        } else {
            return Ok(ToolResult::error(
                tool_use_id,
                "Either text or file must be provided",
            ));
        };

        // Check if search string exists
        let occurrences = content.matches(search).count();
        if occurrences == 0 {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("Search string not found: '{}'", search),
            ));
        }

        // Apply replacement
        let new_content = content.replace(search, replace);

        // Generate diff for preview
        let diff = TextDiff::from_lines(&content, &new_content);
        let mut diff_output = String::new();

        for change in diff.iter_all_changes() {
            let tag = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            diff_output.push_str(&format!("{}{}", tag, change));
        }

        // Apply if not preview only
        if !preview_only {
            if let Some(ref path) = file_path {
                tokio::fs::write(path, &new_content)
                    .await
                    .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to write file: {}", e)))?;
            }
        }

        let duration = start.elapsed();

        debug!(
            "Patch: {} occurrences replaced, preview_only={}",
            occurrences, preview_only
        );

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "occurrences": occurrences,
                "preview_only": preview_only,
                "applied": !preview_only,
                "diff": diff_output,
                "file": file_path.map(|p| p.to_string_lossy().to_string()),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Resolve a path relative to the working directory.
fn resolve_path(path: &str, cwd: &std::path::Path) -> PathBuf {
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_tool_creation() {
        let tool = DiffTool::new();
        assert_eq!(tool.name(), "diff");
    }

    #[test]
    fn test_patch_tool_creation() {
        let tool = PatchTool::new();
        assert_eq!(tool.name(), "patch");
    }

    #[tokio::test]
    async fn test_diff_text() {
        let tool = DiffTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "old_text": "line1\nline2\nline3",
                    "new_text": "line1\nmodified\nline3"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(result.output.get("has_changes").and_then(|v| v.as_bool()).unwrap_or(false));
    }

    #[tokio::test]
    async fn test_diff_no_changes() {
        let tool = DiffTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "old_text": "same content",
                    "new_text": "same content"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(!result.output.get("has_changes").and_then(|v| v.as_bool()).unwrap_or(true));
    }

    #[tokio::test]
    async fn test_patch_preview() {
        let tool = PatchTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "text": "hello world",
                    "search": "world",
                    "replace": "universe",
                    "preview_only": true
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert_eq!(result.output.get("occurrences").and_then(|v| v.as_u64()), Some(1));
        assert_eq!(result.output.get("applied").and_then(|v| v.as_bool()), Some(false));
    }

    #[tokio::test]
    async fn test_patch_not_found() {
        let tool = PatchTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({
                    "text": "hello world",
                    "search": "notfound",
                    "replace": "replacement"
                }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(result.is_error);
    }
}
