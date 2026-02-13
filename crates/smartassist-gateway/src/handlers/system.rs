//! System RPC method handlers.
//!
//! Handles system presence, heartbeats, and logs.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// System presence info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPresence {
    /// Gateway version.
    pub version: String,
    /// Platform (darwin, linux, windows).
    pub platform: String,
    /// Architecture.
    pub arch: String,
    /// Hostname.
    pub hostname: String,
    /// Active channels.
    pub channels: Vec<String>,
    /// Connected devices.
    pub devices: Vec<String>,
    /// Uptime in seconds.
    pub uptime_seconds: u64,
}

/// System presence handler.
pub struct SystemPresenceHandler {
    context: Arc<HandlerContext>,
    start_time: std::time::Instant,
}

impl SystemPresenceHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self {
            context,
            start_time: std::time::Instant::now(),
        }
    }
}

#[async_trait]
impl MethodHandler for SystemPresenceHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("System presence request");

        let _active_channels = self
            .context
            .active_channels
            .load(std::sync::atomic::Ordering::Relaxed);

        let presence = SystemPresence {
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            hostname: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "unknown".to_string()),
            channels: vec![], // TODO: Get from channel manager
            devices: vec![],  // TODO: Get from device manager
            uptime_seconds: self.start_time.elapsed().as_secs(),
        };

        Ok(serde_json::to_value(presence).unwrap())
    }
}

/// Parameters for system-event method.
#[derive(Debug, Deserialize)]
pub struct SystemEventParams {
    /// Event type.
    pub event: String,
    /// Event payload.
    pub payload: Option<serde_json::Value>,
}

/// System event handler.
pub struct SystemEventHandler {
    _context: Arc<HandlerContext>,
}

impl SystemEventHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for SystemEventHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SystemEventParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("System event: {}", params.event);

        // TODO: Actually process/broadcast the event

        Ok(serde_json::json!({
            "event": params.event,
            "received": true,
        }))
    }
}

/// Last heartbeat handler.
pub struct LastHeartbeatHandler {
    _context: Arc<HandlerContext>,
}

impl LastHeartbeatHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for LastHeartbeatHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Last heartbeat request");

        // TODO: Track actual heartbeats
        Ok(serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "connected": true,
        }))
    }
}

/// Parameters for set-heartbeats method.
#[derive(Debug, Deserialize)]
pub struct SetHeartbeatsParams {
    /// Whether to enable heartbeats.
    pub enabled: bool,
    /// Interval in milliseconds.
    pub interval_ms: Option<u64>,
}

/// Set heartbeats handler.
pub struct SetHeartbeatsHandler {
    _context: Arc<HandlerContext>,
}

impl SetHeartbeatsHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for SetHeartbeatsHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SetHeartbeatsParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Set heartbeats: enabled={}", params.enabled);

        Ok(serde_json::json!({
            "enabled": params.enabled,
            "interval_ms": params.interval_ms.unwrap_or(30_000),
        }))
    }
}

/// Parameters for logs.tail method.
#[derive(Debug, Deserialize)]
pub struct LogsTailParams {
    /// Number of lines to return.
    pub lines: Option<usize>,
    /// Log level filter.
    pub level: Option<String>,
    /// Component filter.
    pub component: Option<String>,
}

/// Logs tail handler.
pub struct LogsTailHandler {
    _context: Arc<HandlerContext>,
}

impl LogsTailHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for LogsTailHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: LogsTailParams = params
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        debug!("Logs tail: lines={:?}", params.lines);

        // TODO: Actually read logs
        Ok(serde_json::json!({
            "logs": [],
            "count": 0,
        }))
    }
}

impl Default for LogsTailParams {
    fn default() -> Self {
        Self {
            lines: Some(100),
            level: None,
            component: None,
        }
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for SystemEventParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for SetHeartbeatsParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_presence_serialization() {
        let presence = SystemPresence {
            version: "0.1.0".to_string(),
            platform: "darwin".to_string(),
            arch: "aarch64".to_string(),
            hostname: "test-host".to_string(),
            channels: vec!["telegram".to_string()],
            devices: vec![],
            uptime_seconds: 3600,
        };

        let json = serde_json::to_value(&presence).unwrap();
        assert_eq!(json["version"], "0.1.0");
        assert_eq!(json["uptime_seconds"], 3600);
    }
}
