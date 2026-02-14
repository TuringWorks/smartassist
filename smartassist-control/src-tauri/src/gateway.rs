//! Gateway process management.
//!
//! Spawns and manages the `smartassist gateway run` subprocess.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::process::{Child, Command};

/// Status of the gateway process.
#[derive(Debug, Clone, Serialize)]
pub struct GatewayStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: u16,
    pub uptime_secs: Option<u64>,
}

struct Inner {
    child: tokio::sync::Mutex<Option<Child>>,
    started_at: std::sync::Mutex<Option<Instant>>,
    port: AtomicU16,
    pid: AtomicU32,
    running: AtomicBool,
}

/// Manages the gateway subprocess lifecycle.
#[derive(Clone)]
pub struct GatewayManager {
    inner: Arc<Inner>,
}

impl GatewayManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                child: tokio::sync::Mutex::new(None),
                started_at: std::sync::Mutex::new(None),
                port: AtomicU16::new(18789),
                pid: AtomicU32::new(0),
                running: AtomicBool::new(false),
            }),
        }
    }

    /// Find the `smartassist` binary.
    fn find_binary() -> Option<std::path::PathBuf> {
        // 1. Check PATH
        if let Ok(output) = std::process::Command::new("which")
            .arg("smartassist")
            .output()
        {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(std::path::PathBuf::from(path));
                }
            }
        }

        // 2. Check same directory as current executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                let candidate = dir.join("smartassist");
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }

        None
    }

    /// Start the gateway subprocess.
    pub async fn start(&self, port: u16, provider: &str) -> Result<(), String> {
        let mut guard = self.inner.child.lock().await;

        // Check if already running
        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(None) => return Err("Gateway is already running".to_string()),
                _ => {
                    // Process exited, clean up
                    *guard = None;
                }
            }
        }

        let binary = Self::find_binary().ok_or_else(|| {
            "Could not find 'smartassist' binary. Ensure it's in your PATH.".to_string()
        })?;

        let child = Command::new(&binary)
            .args([
                "gateway",
                "run",
                "--port",
                &port.to_string(),
                "--bind",
                "loopback",
                "--provider",
                provider,
            ])
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Failed to start gateway: {}", e))?;

        let pid = child.id().unwrap_or(0);
        *guard = Some(child);
        self.inner.port.store(port, Ordering::Relaxed);
        self.inner.pid.store(pid, Ordering::Relaxed);
        self.inner.running.store(true, Ordering::Relaxed);
        *self.inner.started_at.lock().unwrap() = Some(Instant::now());

        Ok(())
    }

    /// Stop the gateway subprocess.
    pub async fn stop(&self) -> Result<(), String> {
        let mut guard = self.inner.child.lock().await;
        if let Some(ref mut child) = *guard {
            child
                .kill()
                .await
                .map_err(|e| format!("Failed to stop gateway: {}", e))?;
            *guard = None;
            self.inner.running.store(false, Ordering::Relaxed);
            self.inner.pid.store(0, Ordering::Relaxed);
            *self.inner.started_at.lock().unwrap() = None;
            Ok(())
        } else {
            Err("Gateway is not running".to_string())
        }
    }

    /// Refresh the running status by checking the child process.
    pub async fn refresh_status(&self) {
        let mut guard = self.inner.child.lock().await;
        if let Some(ref mut child) = *guard {
            match child.try_wait() {
                Ok(None) => {
                    // Still running
                    self.inner.running.store(true, Ordering::Relaxed);
                }
                _ => {
                    // Process exited
                    *guard = None;
                    self.inner.running.store(false, Ordering::Relaxed);
                    self.inner.pid.store(0, Ordering::Relaxed);
                    *self.inner.started_at.lock().unwrap() = None;
                }
            }
        } else {
            self.inner.running.store(false, Ordering::Relaxed);
        }
    }

    /// Get the current gateway status (non-async, uses cached atomics).
    pub fn status(&self) -> GatewayStatus {
        let running = self.inner.running.load(Ordering::Relaxed);
        let pid = if running {
            let p = self.inner.pid.load(Ordering::Relaxed);
            if p > 0 { Some(p) } else { None }
        } else {
            None
        };
        let uptime_secs = if running {
            self.inner
                .started_at
                .lock()
                .unwrap()
                .map(|t| t.elapsed().as_secs())
        } else {
            None
        };

        GatewayStatus {
            running,
            pid,
            port: self.inner.port.load(Ordering::Relaxed),
            uptime_secs,
        }
    }
}
