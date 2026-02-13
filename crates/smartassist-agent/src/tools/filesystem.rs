//! File system tools.
//!
//! - [`ReadTool`] - Read file contents
//! - [`WriteTool`] - Write file contents
//! - [`EditTool`] - Edit file with search/replace
//! - [`GlobTool`] - Find files by pattern
//! - [`GrepTool`] - Search file contents

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use regex::Regex;
use std::path::PathBuf;
use std::time::Instant;
use tracing::debug;

/// Read tool - Read file contents with optional line range.
pub struct ReadTool;

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read".to_string(),
            description: "Read the contents of a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to read"
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Line offset to start reading from (0-indexed)"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of lines to read"
                    }
                },
                "required": ["path"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'path' argument"))?;

        let full_path = resolve_path(path, &context.cwd);

        let content = tokio::fs::read_to_string(&full_path).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to read file '{}': {}", path, e))
        })?;

        let offset = args
            .get("offset")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

        let lines: Vec<&str> = content.lines().skip(offset).collect();
        let lines = match limit {
            Some(l) => &lines[..l.min(lines.len())],
            None => &lines[..],
        };

        // Add line numbers (1-indexed)
        let numbered: Vec<String> = lines
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{:>6}\t{}", offset + i + 1, line))
            .collect();

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "content": numbered.join("\n"),
                "lines": lines.len(),
                "path": full_path.to_string_lossy(),
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }
}

/// Write tool - Write content to a file.
pub struct WriteTool;

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write".to_string(),
            description: "Write content to a file".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to write"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write to the file"
                    }
                },
                "required": ["path", "content"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'path' argument"))?;

        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'content' argument"))?;

        let full_path = resolve_path(path, &context.cwd);

        // Ensure parent directory exists
        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                AgentError::tool_execution(format!("Failed to create directory: {}", e))
            })?;
        }

        tokio::fs::write(&full_path, content).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to write file '{}': {}", path, e))
        })?;

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "path": full_path.to_string_lossy(),
                "bytes_written": content.len(),
            }))
            .with_duration(duration),
        )
    }

    fn requires_approval(&self, _args: &serde_json::Value) -> bool {
        true // File writes should require approval
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }
}

/// Edit tool - Edit file with exact string replacement.
pub struct EditTool {
    /// Maximum file size to edit (bytes).
    max_file_size: usize,
}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}

impl EditTool {
    /// Create a new edit tool.
    pub fn new() -> Self {
        Self {
            max_file_size: 10 * 1024 * 1024, // 10 MB
        }
    }

    /// Set maximum file size.
    pub fn with_max_file_size(mut self, size: usize) -> Self {
        self.max_file_size = size;
        self
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "edit".to_string(),
            description: "Edit a file by replacing exact text. The old_string must be unique in the file.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path to the file to edit"
                    },
                    "old_string": {
                        "type": "string",
                        "description": "The exact text to replace (must be unique in the file)"
                    },
                    "new_string": {
                        "type": "string",
                        "description": "The text to replace it with"
                    },
                    "replace_all": {
                        "type": "boolean",
                        "description": "Replace all occurrences instead of requiring unique match"
                    }
                },
                "required": ["path", "old_string", "new_string"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'path' argument"))?;

        let old_string = args
            .get("old_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'old_string' argument"))?;

        let new_string = args
            .get("new_string")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'new_string' argument"))?;

        let replace_all = args
            .get("replace_all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let full_path = resolve_path(path, &context.cwd);

        // Read file
        let content = tokio::fs::read_to_string(&full_path).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to read file '{}': {}", path, e))
        })?;

        if content.len() > self.max_file_size {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("File too large ({} bytes, max {})", content.len(), self.max_file_size),
            ));
        }

        // Count occurrences
        let count = content.matches(old_string).count();

        if count == 0 {
            return Ok(ToolResult::error(
                tool_use_id,
                "old_string not found in file",
            ));
        }

        if count > 1 && !replace_all {
            return Ok(ToolResult::error(
                tool_use_id,
                format!(
                    "old_string matches {} times. Use replace_all=true or provide more context for unique match.",
                    count
                ),
            ));
        }

        // Perform replacement
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };

        // Write back
        tokio::fs::write(&full_path, &new_content).await.map_err(|e| {
            AgentError::tool_execution(format!("Failed to write file '{}': {}", path, e))
        })?;

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "path": full_path.to_string_lossy(),
                "replacements": if replace_all { count } else { 1 },
            }))
            .with_duration(duration),
        )
    }

    fn requires_approval(&self, _args: &serde_json::Value) -> bool {
        true // File edits should require approval
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }
}

/// Glob tool - Find files matching a pattern.
pub struct GlobTool {
    /// Maximum results to return.
    max_results: usize,
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobTool {
    /// Create a new glob tool.
    pub fn new() -> Self {
        Self { max_results: 1000 }
    }

    /// Set maximum results.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "glob".to_string(),
            description: "Find files matching a glob pattern (e.g., '**/*.rs', 'src/**/*.ts')".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "The glob pattern to match files"
                    },
                    "path": {
                        "type": "string",
                        "description": "Base directory to search from (defaults to cwd)"
                    }
                },
                "required": ["pattern"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'pattern' argument"))?;

        let base_path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| resolve_path(p, &context.cwd))
            .unwrap_or_else(|| context.cwd.clone());

        // Construct full pattern
        let full_pattern = base_path.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        debug!("Glob pattern: {}", pattern_str);

        // Execute glob
        let entries: Vec<String> = glob::glob(&pattern_str)
            .map_err(|e| AgentError::tool_execution(format!("Invalid glob pattern: {}", e)))?
            .filter_map(|entry| entry.ok())
            .take(self.max_results)
            .map(|path| {
                path.strip_prefix(&base_path)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string()
            })
            .collect();

        let truncated = entries.len() >= self.max_results;

        let duration = start.elapsed();
        Ok(
            ToolResult::success(tool_use_id, serde_json::json!({
                "matches": entries,
                "count": entries.len(),
                "truncated": truncated,
                "base_path": base_path.to_string_lossy(),
            }))
            .with_duration(duration),
        )
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
    }
}

/// Grep tool - Search file contents with regex.
pub struct GrepTool {
    /// Maximum results to return.
    max_results: usize,
    /// Context lines before match.
    default_context_before: usize,
    /// Context lines after match.
    default_context_after: usize,
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

impl GrepTool {
    /// Create a new grep tool.
    pub fn new() -> Self {
        Self {
            max_results: 500,
            default_context_before: 0,
            default_context_after: 0,
        }
    }

    /// Set maximum results.
    pub fn with_max_results(mut self, max: usize) -> Self {
        self.max_results = max;
        self
    }

    /// Set default context lines.
    pub fn with_context(mut self, before: usize, after: usize) -> Self {
        self.default_context_before = before;
        self.default_context_after = after;
        self
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct GrepMatch {
    file: String,
    line: usize,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_before: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    context_after: Option<Vec<String>>,
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "grep".to_string(),
            description: "Search file contents using regex pattern".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Regex pattern to search for"
                    },
                    "path": {
                        "type": "string",
                        "description": "File or directory to search (defaults to cwd)"
                    },
                    "glob": {
                        "type": "string",
                        "description": "Glob pattern to filter files (e.g., '*.rs')"
                    },
                    "case_insensitive": {
                        "type": "boolean",
                        "description": "Case-insensitive search"
                    },
                    "context": {
                        "type": "integer",
                        "description": "Lines of context before and after match"
                    },
                    "files_only": {
                        "type": "boolean",
                        "description": "Only return file names, not matches"
                    }
                },
                "required": ["pattern"]
            }),
            execution: ToolExecutionConfig::default(),
        }
    }

    async fn execute(
        &self,
        tool_use_id: &str,
        args: serde_json::Value,
        context: &ToolContext,
    ) -> Result<ToolResult> {
        let start = Instant::now();

        let pattern = args
            .get("pattern")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'pattern' argument"))?;

        let case_insensitive = args
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let context_lines = args
            .get("context")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize);

        let files_only = args
            .get("files_only")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Build regex
        let regex_pattern = if case_insensitive {
            format!("(?i){}", pattern)
        } else {
            pattern.to_string()
        };

        let regex = Regex::new(&regex_pattern)
            .map_err(|e| AgentError::tool_execution(format!("Invalid regex: {}", e)))?;

        // Get files to search
        let base_path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|p| resolve_path(p, &context.cwd))
            .unwrap_or_else(|| context.cwd.clone());

        let file_glob = args
            .get("glob")
            .and_then(|v| v.as_str())
            .unwrap_or("**/*");

        // Handle the case where base_path is a file vs directory
        let files: Vec<PathBuf> = if base_path.is_file() {
            // If path is a file, just search that file
            vec![base_path.clone()]
        } else {
            // If path is a directory, use glob
            let glob_pattern = base_path.join(file_glob);
            glob::glob(&glob_pattern.to_string_lossy())
                .map_err(|e| AgentError::tool_execution(format!("Invalid glob: {}", e)))?
                .filter_map(|e| e.ok())
                .filter(|p| p.is_file())
                .collect()
        };

        let mut matches: Vec<GrepMatch> = Vec::new();
        let mut files_with_matches: Vec<String> = Vec::new();

        for file_path in files {
            if matches.len() >= self.max_results {
                break;
            }

            // Try to read as text
            let content = match tokio::fs::read_to_string(&file_path).await {
                Ok(c) => c,
                Err(_) => continue, // Skip binary files
            };

            let lines: Vec<&str> = content.lines().collect();
            let mut file_has_match = false;

            for (line_idx, line) in lines.iter().enumerate() {
                if regex.is_match(line) {
                    file_has_match = true;

                    if !files_only {
                        let rel_path = file_path
                            .strip_prefix(&context.cwd)
                            .unwrap_or(&file_path)
                            .to_string_lossy()
                            .to_string();

                        let ctx = context_lines.unwrap_or(self.default_context_before);

                        let context_before = if ctx > 0 {
                            let start = line_idx.saturating_sub(ctx);
                            Some(
                                lines[start..line_idx]
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect(),
                            )
                        } else {
                            None
                        };

                        let context_after = if ctx > 0 {
                            let end = (line_idx + 1 + ctx).min(lines.len());
                            Some(
                                lines[line_idx + 1..end]
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect(),
                            )
                        } else {
                            None
                        };

                        matches.push(GrepMatch {
                            file: rel_path,
                            line: line_idx + 1, // 1-indexed
                            content: line.to_string(),
                            context_before,
                            context_after,
                        });

                        if matches.len() >= self.max_results {
                            break;
                        }
                    }
                }
            }

            if file_has_match {
                let rel_path = file_path
                    .strip_prefix(&context.cwd)
                    .unwrap_or(&file_path)
                    .to_string_lossy()
                    .to_string();
                files_with_matches.push(rel_path);
            }
        }

        let duration = start.elapsed();

        if files_only {
            Ok(
                ToolResult::success(tool_use_id, serde_json::json!({
                    "files": files_with_matches,
                    "count": files_with_matches.len(),
                }))
                .with_duration(duration),
            )
        } else {
            Ok(
                ToolResult::success(tool_use_id, serde_json::json!({
                    "matches": matches,
                    "count": matches.len(),
                    "files_count": files_with_matches.len(),
                    "truncated": matches.len() >= self.max_results,
                }))
                .with_duration(duration),
            )
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::FileSystem
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
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_read_tool() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "line1\nline2\nline3").unwrap();

        let context = ToolContext {
            cwd: dir.path().to_path_buf(),
            ..Default::default()
        };

        let tool = ReadTool;
        let result = tool
            .execute(
                "test-1",
                serde_json::json!({ "path": "test.txt" }),
                &context,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
    }

    #[tokio::test]
    async fn test_write_tool() {
        let dir = tempdir().unwrap();

        let context = ToolContext {
            cwd: dir.path().to_path_buf(),
            ..Default::default()
        };

        let tool = WriteTool;
        let result = tool
            .execute(
                "test-1",
                serde_json::json!({
                    "path": "new_file.txt",
                    "content": "Hello, World!"
                }),
                &context,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        assert!(dir.path().join("new_file.txt").exists());
    }

    #[tokio::test]
    async fn test_edit_tool() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "Hello, World!").unwrap();

        let context = ToolContext {
            cwd: dir.path().to_path_buf(),
            ..Default::default()
        };

        let tool = EditTool::new();
        let result = tool
            .execute(
                "test-1",
                serde_json::json!({
                    "path": "test.txt",
                    "old_string": "World",
                    "new_string": "Rust"
                }),
                &context,
            )
            .await
            .unwrap();

        assert!(!result.is_error);

        let content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, Rust!");
    }

    #[tokio::test]
    async fn test_glob_tool() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("file1.txt"), "content").unwrap();
        std::fs::write(dir.path().join("file2.txt"), "content").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        std::fs::write(dir.path().join("subdir/file3.txt"), "content").unwrap();

        let context = ToolContext {
            cwd: dir.path().to_path_buf(),
            ..Default::default()
        };

        let tool = GlobTool::new();
        let result = tool
            .execute(
                "test-1",
                serde_json::json!({ "pattern": "**/*.txt" }),
                &context,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let count = result.output.get("count").and_then(|v| v.as_u64()).unwrap();
        assert_eq!(count, 3);
    }

    #[tokio::test]
    async fn test_grep_tool() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("test.txt"), "Hello World\nFoo Bar\nHello Rust").unwrap();

        let context = ToolContext {
            cwd: dir.path().to_path_buf(),
            ..Default::default()
        };

        let tool = GrepTool::new();
        let result = tool
            .execute(
                "test-1",
                serde_json::json!({
                    "pattern": "Hello",
                    "path": "test.txt"
                }),
                &context,
            )
            .await
            .unwrap();

        assert!(!result.is_error);
        let count = result.output.get("count").and_then(|v| v.as_u64()).unwrap();
        assert_eq!(count, 2);
    }
}
