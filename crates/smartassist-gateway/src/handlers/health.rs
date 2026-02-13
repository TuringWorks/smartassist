//! Health and status RPC method handlers.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use tracing::debug;

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Overall health status.
    pub status: String,

    /// Gateway version.
    pub version: String,

    /// Uptime in seconds.
    pub uptime_seconds: u64,

    /// Component health.
    pub components: ComponentHealth,
}

/// Component health status.
#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    /// Session manager status.
    pub sessions: ComponentStatus,

    /// Channel manager status.
    pub channels: ComponentStatus,

    /// Agent runtime status.
    pub agent: ComponentStatus,
}

/// Individual component status.
#[derive(Debug, Serialize)]
pub struct ComponentStatus {
    /// Status (ok, degraded, error).
    pub status: String,

    /// Optional message.
    pub message: Option<String>,
}

impl ComponentStatus {
    pub fn ok() -> Self {
        Self {
            status: "ok".to_string(),
            message: None,
        }
    }

    pub fn not_configured() -> Self {
        Self {
            status: "not_configured".to_string(),
            message: Some("Component not configured".to_string()),
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            status: "error".to_string(),
            message: Some(msg.into()),
        }
    }
}

/// Health method handler.
pub struct HealthHandler {
    _context: Arc<HandlerContext>,
    start_time: std::time::Instant,
}

impl HealthHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self {
            _context: context,
            start_time: std::time::Instant::now(),
        }
    }
}

#[async_trait]
impl MethodHandler for HealthHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Health check request");

        // Sessions are always available (in-memory storage)
        let sessions_status = ComponentStatus::ok();

        // Channels status based on active count
        let channels_status = ComponentStatus::ok();

        let response = HealthResponse {
            status: "ok".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            components: ComponentHealth {
                sessions: sessions_status,
                channels: channels_status,
                agent: ComponentStatus::ok(),
            },
        };

        serde_json::to_value(response).map_err(|e| GatewayError::Internal(e.to_string()))
    }
}

/// System status response.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    /// Gateway name.
    pub name: String,

    /// Gateway version.
    pub version: String,

    /// Platform.
    pub platform: String,

    /// Architecture.
    pub arch: String,

    /// Active sessions count.
    pub active_sessions: usize,

    /// Active channels count.
    pub active_channels: usize,

    /// Memory usage (if available).
    pub memory_mb: Option<f64>,

    /// CPU usage (if available).
    pub cpu_percent: Option<f64>,
}

/// Status method handler.
pub struct StatusHandler {
    context: Arc<HandlerContext>,
}

impl StatusHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for StatusHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Status request");

        let active_sessions = self.context.sessions.read().await.len();
        let active_channels = self
            .context
            .active_channels
            .load(std::sync::atomic::Ordering::Relaxed);

        let response = StatusResponse {
            name: "smartassist-gateway".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            active_sessions,
            active_channels,
            memory_mb: None, // TODO: Get actual memory usage
            cpu_percent: None, // TODO: Get actual CPU usage
        };

        serde_json::to_value(response).map_err(|e| GatewayError::Internal(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_status_ok() {
        let status = ComponentStatus::ok();
        assert_eq!(status.status, "ok");
        assert!(status.message.is_none());
    }

    #[test]
    fn test_component_status_error() {
        let status = ComponentStatus::error("Something went wrong");
        assert_eq!(status.status, "error");
        assert_eq!(status.message, Some("Something went wrong".to_string()));
    }
}
