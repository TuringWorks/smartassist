//! Pseudo-terminal (PTY) handling for interactive commands.

use crate::error::SandboxError;
use crate::Result;
use portable_pty::{native_pty_system, CommandBuilder, PtyPair, PtySize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// PTY session configuration.
#[derive(Debug, Clone)]
pub struct PtyConfig {
    /// Initial terminal size (columns).
    pub cols: u16,

    /// Initial terminal size (rows).
    pub rows: u16,

    /// Shell to use.
    pub shell: String,

    /// Working directory.
    pub cwd: Option<PathBuf>,

    /// Environment variables.
    pub env: HashMap<String, String>,

    /// Command to run (if not interactive shell).
    pub command: Option<String>,

    /// Arguments for the command.
    pub args: Vec<String>,
}

impl Default for PtyConfig {
    fn default() -> Self {
        Self {
            cols: 80,
            rows: 24,
            shell: std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
            cwd: None,
            env: HashMap::new(),
            command: None,
            args: vec![],
        }
    }
}

impl PtyConfig {
    /// Create a new PTY config with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set terminal size.
    pub fn with_size(mut self, cols: u16, rows: u16) -> Self {
        self.cols = cols;
        self.rows = rows;
        self
    }

    /// Set working directory.
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set environment variable.
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set command to run.
    pub fn with_command(mut self, cmd: impl Into<String>) -> Self {
        self.command = Some(cmd.into());
        self
    }

    /// Set command arguments.
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }
}

/// A PTY session for interactive command execution.
pub struct PtySession {
    /// The PTY pair (master and slave).
    pair: PtyPair,

    /// Child process handle.
    child: Box<dyn portable_pty::Child + Send + Sync>,

    /// Reader for PTY output.
    reader: Arc<Mutex<Box<dyn Read + Send>>>,

    /// Writer for PTY input.
    writer: Arc<Mutex<Box<dyn Write + Send>>>,

    /// Session configuration.
    config: PtyConfig,

    /// Whether the session is still running.
    running: Arc<Mutex<bool>>,
}

impl PtySession {
    /// Create a new PTY session with the given configuration.
    pub fn new(config: PtyConfig) -> Result<Self> {
        let pty_system = native_pty_system();

        let pair = pty_system
            .openpty(PtySize {
                rows: config.rows,
                cols: config.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| SandboxError::pty(e.to_string()))?;

        let mut cmd = if let Some(ref command) = config.command {
            let mut builder = CommandBuilder::new(command);
            builder.args(&config.args);
            builder
        } else {
            CommandBuilder::new(&config.shell)
        };

        if let Some(ref cwd) = config.cwd {
            cmd.cwd(cwd);
        }

        for (key, value) in &config.env {
            cmd.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| SandboxError::pty(e.to_string()))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| SandboxError::pty(e.to_string()))?;

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| SandboxError::pty(e.to_string()))?;

        Ok(Self {
            pair,
            child,
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
            config,
            running: Arc::new(Mutex::new(true)),
        })
    }

    /// Write data to the PTY.
    pub async fn write(&self, data: &[u8]) -> Result<()> {
        let mut writer = self.writer.lock().await;
        writer
            .write_all(data)
            .map_err(|e| SandboxError::pty(e.to_string()))?;
        writer
            .flush()
            .map_err(|e| SandboxError::pty(e.to_string()))?;
        Ok(())
    }

    /// Write a string to the PTY.
    pub async fn write_str(&self, s: &str) -> Result<()> {
        self.write(s.as_bytes()).await
    }

    /// Read data from the PTY (non-blocking, returns available data).
    pub async fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let mut reader = self.reader.lock().await;
        reader
            .read(buf)
            .map_err(|e| SandboxError::pty(e.to_string()))
    }

    /// Resize the PTY.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<()> {
        self.pair
            .master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| SandboxError::pty(e.to_string()))
    }

    /// Check if the PTY session is still running.
    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
    }

    /// Wait for the PTY session to exit.
    pub fn wait(&mut self) -> Result<ExitStatus> {
        let status = self
            .child
            .wait()
            .map_err(|e| SandboxError::pty(e.to_string()))?;

        *self.running.blocking_lock() = false;

        Ok(ExitStatus {
            code: status.exit_code(),
            success: status.success(),
        })
    }

    /// Kill the PTY session.
    pub fn kill(&mut self) -> Result<()> {
        self.child
            .kill()
            .map_err(|e| SandboxError::pty(e.to_string()))?;
        *self.running.blocking_lock() = false;
        Ok(())
    }

    /// Get the PTY configuration.
    pub fn config(&self) -> &PtyConfig {
        &self.config
    }
}

/// Exit status of a PTY session.
#[derive(Debug, Clone, Copy)]
pub struct ExitStatus {
    /// Exit code (if available).
    pub code: u32,

    /// Whether the process exited successfully.
    pub success: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_config_default() {
        let config = PtyConfig::default();
        assert_eq!(config.cols, 80);
        assert_eq!(config.rows, 24);
    }

    #[test]
    fn test_pty_config_builder() {
        let config = PtyConfig::new()
            .with_size(120, 40)
            .with_cwd("/tmp")
            .with_env("TERM", "xterm-256color");

        assert_eq!(config.cols, 120);
        assert_eq!(config.rows, 40);
        assert_eq!(config.cwd, Some(PathBuf::from("/tmp")));
        assert_eq!(config.env.get("TERM"), Some(&"xterm-256color".to_string()));
    }
}
