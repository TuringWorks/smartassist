//! System execution tools.
//!
//! - [`BashTool`] - Execute shell commands

use super::{Tool, ToolContext};
use crate::error::AgentError;
use crate::Result;
use async_trait::async_trait;
use smartassist_core::types::{ToolDefinition, ToolExecutionConfig, ToolGroup, ToolResult};
use smartassist_sandbox::{CommandExecutor, ExecutionContext};
use regex::Regex;
use std::time::Instant;
use tracing::warn;

/// Shell metacharacters that indicate potential command injection in paths/arguments.
const SHELL_METACHARACTERS: &[char] = &['`', '$', '|', '&', ';', '\n', '\r', '\0'];

/// Bash tool - Execute shell commands with sandboxing.
pub struct BashTool {
    /// Allowed commands (regex patterns).
    allowed_patterns: Vec<String>,

    /// Blocked commands (regex patterns).
    blocked_patterns: Vec<String>,

    /// Compiled blocked regexes.
    blocked_regexes: Vec<Regex>,
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}

impl BashTool {
    /// Create a new Bash tool with default security patterns.
    pub fn new() -> Self {
        let blocked_patterns = vec![
            // Destructive filesystem operations
            r"rm\s+-rf\s+/".to_string(),
            r"rm\s+-fr\s+/".to_string(),
            // Privilege escalation
            r"\bsudo\b".to_string(),
            r"\bsu\s+-".to_string(),
            r"\bdoas\b".to_string(),
            // Overly permissive permissions
            r"chmod\s+777".to_string(),
            r"chmod\s+a\+rwx".to_string(),
            // Device writes
            r">\s*/dev/".to_string(),
            // Filesystem destruction
            r"\bmkfs\b".to_string(),
            r"\bdd\s+if=".to_string(),
            // Command substitution in arguments (CVE-2026-25157 vector)
            r"\$\(.*\bssh\b".to_string(),
            r"`.*\bssh\b".to_string(),
            // Encoding-based bypass attempts
            r"\\x[0-9a-fA-F]{2}.*\bssh\b".to_string(),
            // Null byte injection
            r"\\0|\\x00|\x00".to_string(),
        ];

        let blocked_regexes = blocked_patterns
            .iter()
            .filter_map(|p| Regex::new(p).ok())
            .collect();

        Self {
            allowed_patterns: vec![],
            blocked_patterns,
            blocked_regexes,
        }
    }

    /// Add an allowed pattern.
    pub fn allow(mut self, pattern: impl Into<String>) -> Self {
        self.allowed_patterns.push(pattern.into());
        self
    }

    /// Add a blocked pattern.
    pub fn block(mut self, pattern: impl Into<String>) -> Self {
        let pattern_str = pattern.into();
        if let Ok(re) = Regex::new(&pattern_str) {
            self.blocked_regexes.push(re);
        }
        self.blocked_patterns.push(pattern_str);
        self
    }

    /// Check if a command is blocked.
    fn is_blocked(&self, command: &str) -> bool {
        // Check for null bytes
        if command.contains('\0') {
            warn!("Blocked command with null bytes");
            return true;
        }

        for re in &self.blocked_regexes {
            if re.is_match(command) {
                warn!("Blocked command: {}", command);
                return true;
            }
        }
        false
    }

    /// Check if command matches dangerous patterns requiring approval.
    fn is_dangerous(&self, command: &str) -> bool {
        let dangerous_patterns = [
            "rm ", "rmdir", "mv ", "cp ", "> ", ">> ",
            "curl ", "wget ", "pip install", "npm install",
            "chmod", "chown", "kill ", "pkill",
            "git push", "git reset",
            "docker ", "kubectl ", "ssh ",
        ];

        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return true;
            }
        }

        // Command substitution and pipe chains into dangerous commands
        if command.contains("$(") || command.contains('`') {
            return true;
        }

        false
    }

    /// Validate and sanitize a working directory path.
    /// Rejects paths with shell metacharacters to prevent command injection (CVE-2026-25157).
    fn validate_path(path: &str) -> std::result::Result<std::path::PathBuf, AgentError> {
        // Reject paths with shell metacharacters
        if path.chars().any(|c| SHELL_METACHARACTERS.contains(&c)) {
            return Err(AgentError::tool_execution(format!(
                "Path contains shell metacharacters: {}",
                path
            )));
        }

        // Reject paths with control characters
        if path.chars().any(|c| c.is_control()) {
            return Err(AgentError::tool_execution(
                "Path contains control characters",
            ));
        }

        Ok(std::path::PathBuf::from(path))
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "bash".to_string(),
            description: "Execute a bash command in a sandboxed environment".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The bash command to execute"
                    },
                    "timeout": {
                        "type": "integer",
                        "description": "Timeout in seconds (default: 120, max: 600)"
                    },
                    "cwd": {
                        "type": "string",
                        "description": "Working directory for the command"
                    }
                },
                "required": ["command"]
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

        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AgentError::tool_execution("Missing 'command' argument"))?;

        // Check if command is blocked
        if self.is_blocked(command) {
            return Ok(ToolResult::error(
                tool_use_id,
                "Command is blocked by security policy",
            ));
        }

        // Determine working directory with path validation
        let cwd = if let Some(path_str) = args.get("cwd").and_then(|v| v.as_str()) {
            Self::validate_path(path_str)?
        } else {
            context.cwd.clone()
        };

        // Set up execution context
        let exec_context = ExecutionContext::new(&cwd)
            .with_profile(context.sandbox_profile.clone())
            .with_envs(context.env.clone());

        let executor = CommandExecutor::new(exec_context);

        // Get timeout (default 120s, max 600s)
        let timeout = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .map(|t| t.min(600))
            .unwrap_or(120);

        // Execute command
        let output = executor.execute_with_timeout(command, Some(timeout)).await?;
        let duration = start.elapsed();

        let result_output = serde_json::json!({
            "stdout": output.stdout,
            "stderr": output.stderr,
            "exit_code": output.exit_code,
            "timed_out": output.timed_out,
            "duration_ms": duration.as_millis() as u64,
        });

        if output.success() {
            Ok(ToolResult::success(tool_use_id, result_output).with_duration(duration))
        } else {
            Ok(ToolResult {
                tool_use_id: tool_use_id.to_string(),
                output: result_output,
                is_error: true,
                duration_ms: Some(duration.as_millis() as u64),
            })
        }
    }

    fn requires_approval(&self, args: &serde_json::Value) -> bool {
        if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
            self.is_dangerous(command)
        } else {
            true // Require approval if we can't parse the command
        }
    }

    fn group(&self) -> ToolGroup {
        ToolGroup::System
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bash_tool_creation() {
        let tool = BashTool::new();
        assert_eq!(tool.name(), "bash");
    }

    #[test]
    fn test_bash_tool_blocked() {
        let tool = BashTool::new();

        assert!(tool.is_blocked("rm -rf /"));
        assert!(tool.is_blocked("rm -fr /home"));
        assert!(tool.is_blocked("sudo rm something"));
        assert!(tool.is_blocked("doas cat /etc/shadow"));
        assert!(!tool.is_blocked("ls -la"));
        assert!(!tool.is_blocked("echo hello"));
    }

    #[test]
    fn test_bash_tool_blocks_null_bytes() {
        let tool = BashTool::new();
        assert!(tool.is_blocked("cat file\0.txt"));
    }

    #[test]
    fn test_bash_tool_dangerous() {
        let tool = BashTool::new();

        assert!(tool.is_dangerous("rm -rf ./build"));
        assert!(tool.is_dangerous("curl http://example.com"));
        assert!(tool.is_dangerous("git push origin main"));
        assert!(tool.is_dangerous("docker run ubuntu"));
        assert!(tool.is_dangerous("ssh user@host"));
        assert!(!tool.is_dangerous("ls -la"));
    }

    #[test]
    fn test_bash_tool_dangerous_command_substitution() {
        let tool = BashTool::new();

        assert!(tool.is_dangerous("echo $(whoami)"));
        assert!(tool.is_dangerous("echo `whoami`"));
    }

    #[test]
    fn test_bash_tool_requires_approval() {
        let tool = BashTool::new();

        assert!(tool.requires_approval(&serde_json::json!({ "command": "rm -rf ./build" })));
        assert!(tool.requires_approval(&serde_json::json!({ "command": "git push" })));
        assert!(tool.requires_approval(&serde_json::json!({ "command": "echo $(id)" })));
        assert!(!tool.requires_approval(&serde_json::json!({ "command": "ls -la" })));
    }

    #[test]
    fn test_custom_blocked_patterns() {
        let tool = BashTool::new()
            .block(r"^docker\s+")
            .block(r"^kubectl\s+");

        assert!(tool.is_blocked("docker run"));
        assert!(tool.is_blocked("kubectl delete"));
    }

    #[test]
    fn test_path_validation_clean() {
        assert!(BashTool::validate_path("/home/user/project").is_ok());
        assert!(BashTool::validate_path("/tmp/build-output").is_ok());
    }

    #[test]
    fn test_path_validation_rejects_metacharacters() {
        assert!(BashTool::validate_path("/home/user/$(whoami)").is_err());
        assert!(BashTool::validate_path("/home/user/`id`").is_err());
        assert!(BashTool::validate_path("/home/user;rm -rf /").is_err());
        assert!(BashTool::validate_path("/home/user|cat /etc/passwd").is_err());
        assert!(BashTool::validate_path("/home/user&bg").is_err());
    }

    #[test]
    fn test_path_validation_rejects_null_bytes() {
        assert!(BashTool::validate_path("/home/user\0/evil").is_err());
    }
}
