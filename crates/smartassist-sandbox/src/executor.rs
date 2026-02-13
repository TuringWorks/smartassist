//! Command execution within sandbox.

use crate::error::SandboxError;
use crate::profile::SandboxProfile;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;
use tracing::{debug, warn};

/// Context for command execution.
#[derive(Debug, Clone)]
pub struct ExecutionContext {
    /// Working directory.
    pub cwd: PathBuf,

    /// Environment variables.
    pub env: HashMap<String, String>,

    /// Sandbox profile to apply.
    pub profile: SandboxProfile,

    /// Shell to use for command execution.
    pub shell: String,

    /// Shell flag for command execution.
    pub shell_flag: String,

    /// User ID to run as (Linux).
    pub uid: Option<u32>,

    /// Group ID to run as (Linux).
    pub gid: Option<u32>,
}

impl Default for ExecutionContext {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")),
            env: std::env::vars().collect(),
            profile: SandboxProfile::standard(),
            shell: "/bin/sh".to_string(),
            shell_flag: "-c".to_string(),
            uid: None,
            gid: None,
        }
    }
}

impl ExecutionContext {
    /// Create a new execution context with the given working directory.
    pub fn new(cwd: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            ..Default::default()
        }
    }

    /// Set the sandbox profile.
    pub fn with_profile(mut self, profile: SandboxProfile) -> Self {
        self.profile = profile;
        self
    }

    /// Set an environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set multiple environment variables.
    pub fn with_envs(mut self, vars: HashMap<String, String>) -> Self {
        self.env.extend(vars);
        self
    }

    /// Clear environment and set only specified variables.
    pub fn with_clean_env(mut self, vars: HashMap<String, String>) -> Self {
        self.env = vars;
        self
    }

    /// Set user ID to run as.
    pub fn with_uid(mut self, uid: u32) -> Self {
        self.uid = Some(uid);
        self
    }

    /// Set group ID to run as.
    pub fn with_gid(mut self, gid: u32) -> Self {
        self.gid = Some(gid);
        self
    }
}

/// Output from command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionOutput {
    /// Exit code (0 for success).
    pub exit_code: i32,

    /// Standard output.
    pub stdout: String,

    /// Standard error.
    pub stderr: String,

    /// Combined output (interleaved stdout/stderr if captured together).
    pub combined: Option<String>,

    /// Execution duration in milliseconds.
    pub duration_ms: u64,

    /// Whether the command was killed due to timeout.
    pub timed_out: bool,

    /// Whether the command was killed due to resource limits.
    pub resource_limited: bool,

    /// Signal that killed the process (if any).
    pub signal: Option<i32>,
}

impl ExecutionOutput {
    /// Check if execution was successful.
    pub fn success(&self) -> bool {
        self.exit_code == 0 && !self.timed_out && !self.resource_limited
    }

    /// Get output, preferring combined if available.
    pub fn output(&self) -> &str {
        self.combined.as_deref().unwrap_or(&self.stdout)
    }
}

/// Command executor with sandbox support.
pub struct CommandExecutor {
    /// Execution context.
    context: ExecutionContext,

    /// Maximum output size to capture.
    max_output_size: usize,
}

impl CommandExecutor {
    /// Create a new command executor with the given context.
    pub fn new(context: ExecutionContext) -> Self {
        Self {
            context,
            max_output_size: 10 * 1024 * 1024, // 10 MB default
        }
    }

    /// Set maximum output size.
    pub fn with_max_output_size(mut self, size: usize) -> Self {
        self.max_output_size = size;
        self
    }

    /// Execute a command.
    pub async fn execute(&self, command: &str) -> Result<ExecutionOutput> {
        self.execute_with_timeout(command, None).await
    }

    /// Execute a command with explicit timeout.
    pub async fn execute_with_timeout(
        &self,
        command: &str,
        timeout_secs: Option<u64>,
    ) -> Result<ExecutionOutput> {
        let timeout_duration = Duration::from_secs(
            timeout_secs.unwrap_or(self.context.profile.limits.wall_time_secs),
        );

        let start = Instant::now();

        let result = timeout(timeout_duration, self.run_command(command)).await;

        match result {
            Ok(Ok(mut output)) => {
                output.duration_ms = start.elapsed().as_millis() as u64;
                Ok(output)
            }
            Ok(Err(e)) => Err(e),
            Err(_) => {
                // Timeout occurred
                Ok(ExecutionOutput {
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Command timed out after {} seconds", timeout_duration.as_secs()),
                    combined: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    timed_out: true,
                    resource_limited: false,
                    signal: Some(9), // SIGKILL
                })
            }
        }
    }

    /// Run a command (internal implementation).
    async fn run_command(&self, command: &str) -> Result<ExecutionOutput> {
        debug!("Executing command: {}", command);

        // Filter environment according to profile rules
        let env = self.filter_environment();

        let mut cmd = Command::new(&self.context.shell);
        cmd.arg(&self.context.shell_flag)
            .arg(command)
            .current_dir(&self.context.cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .envs(&env);

        // Apply platform-specific sandbox settings
        #[cfg(target_os = "linux")]
        self.apply_linux_sandbox(&mut cmd)?;

        #[cfg(target_os = "macos")]
        self.apply_macos_sandbox(&mut cmd)?;

        let mut child = cmd.spawn().map_err(|e| {
            SandboxError::execution_failed(format!("Failed to spawn command: {}", e))
        })?;

        let stdout_handle = child.stdout.take();
        let stderr_handle = child.stderr.take();

        // Read stdout and stderr concurrently
        let (stdout, stderr) = tokio::join!(
            read_stream(stdout_handle, self.max_output_size),
            read_stream(stderr_handle, self.max_output_size),
        );

        let status = child.wait().await.map_err(|e| {
            SandboxError::execution_failed(format!("Failed to wait for command: {}", e))
        })?;

        let exit_code = status.code().unwrap_or(-1);
        let signal = if !status.success() && status.code().is_none() {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                status.signal()
            }
            #[cfg(not(unix))]
            {
                None
            }
        } else {
            None
        };

        Ok(ExecutionOutput {
            exit_code,
            stdout: stdout.unwrap_or_default(),
            stderr: stderr.unwrap_or_default(),
            combined: None,
            duration_ms: 0, // Set by caller
            timed_out: false,
            resource_limited: false,
            signal,
        })
    }

    /// Filter environment variables according to profile rules.
    fn filter_environment(&self) -> HashMap<String, String> {
        let rules = &self.context.profile.environment;
        let mut env = HashMap::new();

        if rules.inherit {
            // Start with context env, filter out blocked vars
            for (key, value) in &self.context.env {
                if !rules.blocked.contains(key) {
                    env.insert(key.clone(), value.clone());
                }
            }
        } else {
            // Only include explicitly allowed vars
            for key in &rules.allowed {
                if let Some(value) = self.context.env.get(key) {
                    env.insert(key.clone(), value.clone());
                }
            }
        }

        // Apply explicit settings
        for (key, value) in &rules.set {
            env.insert(key.clone(), value.clone());
        }

        // Always ensure blocked vars are removed
        for var in &rules.blocked {
            env.remove(var);
        }

        env
    }

    #[cfg(target_os = "linux")]
    fn apply_linux_sandbox(&self, _cmd: &mut Command) -> Result<()> {
        // Linux-specific sandbox setup would be done in a pre_exec hook
        // This is a placeholder for the actual implementation
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn apply_macos_sandbox(&self, _cmd: &mut Command) -> Result<()> {
        // macOS sandbox-exec would be configured here
        // This is a placeholder for the actual implementation
        Ok(())
    }

    /// Get the execution context.
    pub fn context(&self) -> &ExecutionContext {
        &self.context
    }

    /// Get a mutable reference to the execution context.
    pub fn context_mut(&mut self) -> &mut ExecutionContext {
        &mut self.context
    }
}

/// Read from an async stream with size limit.
async fn read_stream(
    handle: Option<impl tokio::io::AsyncRead + Unpin>,
    max_size: usize,
) -> Option<String> {
    let handle = handle?;
    let mut reader = BufReader::new(handle);
    let mut output = Vec::new();
    let mut total_read = 0;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => break, // EOF
            Ok(n) => {
                total_read += n;
                if total_read > max_size {
                    output.extend_from_slice(b"\n[Output truncated]\n");
                    break;
                }
                output.extend_from_slice(line.as_bytes());
            }
            Err(e) => {
                warn!("Error reading stream: {}", e);
                break;
            }
        }
    }

    String::from_utf8(output).ok()
}

/// Execute a simple command without sandboxing.
pub async fn execute_simple(command: &str, cwd: Option<&PathBuf>) -> Result<ExecutionOutput> {
    let context = ExecutionContext {
        cwd: cwd.cloned().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"))),
        profile: SandboxProfile {
            name: "none".to_string(),
            environment: crate::profile::EnvironmentRules {
                inherit: true,
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    };

    CommandExecutor::new(context).execute(command).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_simple_execution() {
        let result = execute_simple("echo hello", None).await.unwrap();
        assert!(result.success());
        assert!(result.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_execution_with_context() {
        let context = ExecutionContext::new("/tmp")
            .with_env("TEST_VAR", "test_value");

        let executor = CommandExecutor::new(context);
        let result = executor.execute("echo $TEST_VAR").await.unwrap();
        assert!(result.success());
    }

    #[tokio::test]
    async fn test_execution_failure() {
        let result = execute_simple("exit 1", None).await.unwrap();
        assert!(!result.success());
        assert_eq!(result.exit_code, 1);
    }

    #[tokio::test]
    async fn test_environment_filtering() {
        let context = ExecutionContext {
            profile: SandboxProfile {
                environment: crate::profile::EnvironmentRules {
                    inherit: false,
                    allowed: ["PATH"].iter().map(|s| s.to_string()).collect(),
                    ..Default::default()
                },
                ..Default::default()
            },
            ..Default::default()
        };

        let executor = CommandExecutor::new(context);
        let env = executor.filter_environment();

        assert!(env.contains_key("PATH") || env.is_empty()); // PATH may or may not be set
        assert!(!env.contains_key("LD_PRELOAD"));
    }
}
