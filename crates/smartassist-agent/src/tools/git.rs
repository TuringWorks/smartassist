//! Git tools for version control operations.
//!
//! Provides tools for common git operations like status,
//! diff, log, and branch management.

use crate::tools::{Tool, ToolContext};
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Instant;
use tokio::process::Command;
use tracing::debug;

/// Tool for getting git status.
pub struct GitStatusTool;

impl GitStatusTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitStatusTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &str {
        "git_status"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "git_status".to_string(),
            description: "Get git repository status including staged, modified, and untracked files."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Repository path (defaults to current directory)"
                    },
                    "short": {
                        "type": "boolean",
                        "default": false,
                        "description": "Use short format output"
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

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| ctx.cwd.clone());

        let short = args
            .get("short")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&path);
        cmd.arg("status");
        if short {
            cmd.arg("--short");
        }
        cmd.arg("--porcelain=v2");
        cmd.arg("--branch");

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to run git: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            return Ok(ToolResult::error(
                tool_use_id,
                format!("git status failed: {}", stderr),
            ));
        }

        // Parse porcelain v2 output
        let mut branch = String::new();
        let mut staged: Vec<String> = Vec::new();
        let mut modified: Vec<String> = Vec::new();
        let mut untracked: Vec<String> = Vec::new();
        let mut ahead = 0i32;
        let mut behind = 0i32;

        for line in stdout.lines() {
            if line.starts_with("# branch.head ") {
                branch = line.trim_start_matches("# branch.head ").to_string();
            } else if line.starts_with("# branch.ab ") {
                let ab = line.trim_start_matches("# branch.ab ");
                for part in ab.split_whitespace() {
                    if let Some(n) = part.strip_prefix('+') {
                        ahead = n.parse().unwrap_or(0);
                    } else if let Some(n) = part.strip_prefix('-') {
                        behind = n.parse().unwrap_or(0);
                    }
                }
            } else if line.starts_with("1 ") || line.starts_with("2 ") {
                // Changed entry
                let parts: Vec<&str> = line.split(' ').collect();
                if parts.len() >= 9 {
                    let xy = parts[1];
                    let file = parts[8..].join(" ");
                    let x = xy.chars().next().unwrap_or('.');
                    let y = xy.chars().nth(1).unwrap_or('.');

                    if x != '.' {
                        staged.push(file.clone());
                    }
                    if y != '.' {
                        modified.push(file);
                    }
                }
            } else if line.starts_with("? ") {
                // Untracked
                let file = line[2..].to_string();
                untracked.push(file);
            }
        }

        let duration = start.elapsed();

        debug!("Git status: branch={}, staged={}, modified={}, untracked={}",
               branch, staged.len(), modified.len(), untracked.len());

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "branch": branch,
                "ahead": ahead,
                "behind": behind,
                "staged": staged,
                "modified": modified,
                "untracked": untracked,
                "clean": staged.is_empty() && modified.is_empty() && untracked.is_empty(),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for viewing git log.
pub struct GitLogTool;

impl GitLogTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitLogTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &str {
        "git_log"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "git_log".to_string(),
            description: "View git commit history.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Repository path (defaults to current directory)"
                    },
                    "count": {
                        "type": "integer",
                        "default": 10,
                        "description": "Number of commits to show"
                    },
                    "format": {
                        "type": "string",
                        "enum": ["oneline", "short", "full"],
                        "default": "short",
                        "description": "Output format"
                    },
                    "author": {
                        "type": "string",
                        "description": "Filter by author"
                    },
                    "since": {
                        "type": "string",
                        "description": "Show commits after date (e.g., '2024-01-01')"
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

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| ctx.cwd.clone());

        let count = args
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or(10) as usize;

        let format = args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("short");

        let author = args.get("author").and_then(|v| v.as_str());
        let since = args.get("since").and_then(|v| v.as_str());

        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&path);
        cmd.arg("log");
        cmd.arg(format!("-{}", count));
        cmd.arg("--format=%H|%h|%an|%ae|%ai|%s");

        if let Some(author) = author {
            cmd.arg(format!("--author={}", author));
        }
        if let Some(since) = since {
            cmd.arg(format!("--since={}", since));
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to run git: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(
                tool_use_id,
                format!("git log failed: {}", stderr),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut commits: Vec<serde_json::Value> = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(6, '|').collect();
            if parts.len() >= 6 {
                commits.push(serde_json::json!({
                    "hash": parts[0],
                    "short_hash": parts[1],
                    "author_name": parts[2],
                    "author_email": parts[3],
                    "date": parts[4],
                    "message": parts[5],
                }));
            }
        }

        let duration = start.elapsed();

        debug!("Git log: {} commits", commits.len());

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "commits": commits,
                "count": commits.len(),
                "format": format,
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for viewing git diff.
pub struct GitDiffTool;

impl GitDiffTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitDiffTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &str {
        "git_diff"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "git_diff".to_string(),
            description: "View git diff for changes.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Repository path (defaults to current directory)"
                    },
                    "staged": {
                        "type": "boolean",
                        "default": false,
                        "description": "Show staged changes only"
                    },
                    "file": {
                        "type": "string",
                        "description": "Show diff for specific file"
                    },
                    "stat": {
                        "type": "boolean",
                        "default": false,
                        "description": "Show only statistics (additions/deletions)"
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

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| ctx.cwd.clone());

        let staged = args
            .get("staged")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let file = args.get("file").and_then(|v| v.as_str());

        let stat = args
            .get("stat")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&path);
        cmd.arg("diff");

        if staged {
            cmd.arg("--cached");
        }

        if stat {
            cmd.arg("--stat");
        }

        if let Some(f) = file {
            cmd.arg("--").arg(f);
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to run git: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(
                tool_use_id,
                format!("git diff failed: {}", stderr),
            ));
        }

        let diff = String::from_utf8_lossy(&output.stdout).to_string();

        // Count additions and deletions
        let mut additions = 0;
        let mut deletions = 0;
        for line in diff.lines() {
            if line.starts_with('+') && !line.starts_with("+++") {
                additions += 1;
            } else if line.starts_with('-') && !line.starts_with("---") {
                deletions += 1;
            }
        }

        let duration = start.elapsed();

        debug!("Git diff: +{} -{}", additions, deletions);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "diff": diff,
                "additions": additions,
                "deletions": deletions,
                "staged": staged,
                "has_changes": !diff.is_empty(),
            }),
        )
        .with_duration(duration))
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::Custom
    }
}

/// Tool for listing git branches.
pub struct GitBranchTool;

impl GitBranchTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GitBranchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for GitBranchTool {
    fn name(&self) -> &str {
        "git_branch"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "git_branch".to_string(),
            description: "List git branches and show current branch.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Repository path (defaults to current directory)"
                    },
                    "all": {
                        "type": "boolean",
                        "default": false,
                        "description": "List all branches including remote"
                    },
                    "remote": {
                        "type": "boolean",
                        "default": false,
                        "description": "List remote branches only"
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

        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| ctx.cwd.clone());

        let all = args
            .get("all")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let remote = args
            .get("remote")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut cmd = Command::new("git");
        cmd.arg("-C").arg(&path);
        cmd.arg("branch");

        if all {
            cmd.arg("-a");
        } else if remote {
            cmd.arg("-r");
        }

        cmd.arg("--format=%(HEAD)|%(refname:short)|%(upstream:short)|%(objectname:short)");

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let output = cmd
            .output()
            .await
            .map_err(|e| crate::error::AgentError::tool_execution(format!("Failed to run git: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Ok(ToolResult::error(
                tool_use_id,
                format!("git branch failed: {}", stderr),
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut branches: Vec<serde_json::Value> = Vec::new();
        let mut current_branch = String::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(4, '|').collect();
            if parts.len() >= 4 {
                let is_current = parts[0] == "*";
                let name = parts[1].to_string();
                let upstream = if parts[2].is_empty() {
                    None
                } else {
                    Some(parts[2].to_string())
                };
                let commit = parts[3].to_string();

                if is_current {
                    current_branch = name.clone();
                }

                branches.push(serde_json::json!({
                    "name": name,
                    "current": is_current,
                    "upstream": upstream,
                    "commit": commit,
                }));
            }
        }

        let duration = start.elapsed();

        debug!("Git branches: {} total, current={}", branches.len(), current_branch);

        Ok(ToolResult::success(
            tool_use_id,
            serde_json::json!({
                "branches": branches,
                "current": current_branch,
                "count": branches.len(),
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
    fn test_git_status_tool_creation() {
        let tool = GitStatusTool::new();
        assert_eq!(tool.name(), "git_status");
    }

    #[test]
    fn test_git_log_tool_creation() {
        let tool = GitLogTool::new();
        assert_eq!(tool.name(), "git_log");
    }

    #[test]
    fn test_git_diff_tool_creation() {
        let tool = GitDiffTool::new();
        assert_eq!(tool.name(), "git_diff");
    }

    #[test]
    fn test_git_branch_tool_creation() {
        let tool = GitBranchTool::new();
        assert_eq!(tool.name(), "git_branch");
    }

    // Integration tests that require a git repository
    #[tokio::test]
    async fn test_git_status_execute() {
        let tool = GitStatusTool::new();
        let ctx = ToolContext::default();

        // This test will work in any git repo
        let result = tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await;

        // May fail if not in a git repo, which is expected
        if let Ok(result) = result {
            if !result.is_error {
                assert!(result.output.get("branch").is_some());
            }
        }
    }

    #[tokio::test]
    async fn test_git_log_execute() {
        let tool = GitLogTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute(
                "test_id",
                serde_json::json!({"count": 5}),
                &ctx,
            )
            .await;

        if let Ok(result) = result {
            if !result.is_error {
                assert!(result.output.get("commits").is_some());
            }
        }
    }

    #[tokio::test]
    async fn test_git_branch_execute() {
        let tool = GitBranchTool::new();
        let ctx = ToolContext::default();

        let result = tool
            .execute("test_id", serde_json::json!({}), &ctx)
            .await;

        if let Ok(result) = result {
            if !result.is_error {
                assert!(result.output.get("branches").is_some());
            }
        }
    }
}
