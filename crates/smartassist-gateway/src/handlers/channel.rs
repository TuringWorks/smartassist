//! Channel RPC method handlers.
//!
//! Handles channel listing and health checks.

use super::HandlerContext;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::Serialize;
use std::sync::Arc;
use tracing::debug;

/// Channel info in list response.
#[derive(Debug, Serialize)]
pub struct ChannelInfo {
    /// Channel instance ID.
    pub id: String,

    /// Channel type.
    pub channel_type: String,

    /// Whether the channel is connected.
    pub connected: bool,

    /// Health status.
    pub status: String,
}

/// Channel list method handler.
pub struct ChannelListHandler {
    context: Arc<HandlerContext>,
}

impl ChannelListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ChannelListHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Channel list request");

        let channels = if let Some(ref manager) = self.context.channel_manager {
            let ids = manager.list_channels().await;
            let health = manager.health().await;
            ids.into_iter()
                .map(|id| {
                    let h = health.get(&id);
                    ChannelInfo {
                        id: id.clone(),
                        channel_type: "unknown".to_string(),
                        connected: h.map(|h| h.status == smartassist_core::types::HealthStatus::Healthy).unwrap_or(false),
                        status: h.map(|h| format!("{:?}", h.status)).unwrap_or_else(|| "unknown".to_string()),
                    }
                })
                .collect::<Vec<_>>()
        } else {
            vec![]
        };

        Ok(serde_json::json!({
            "channels": channels,
            "count": channels.len(),
        }))
    }
}

/// Channel health method handler.
pub struct ChannelHealthHandler {
    context: Arc<HandlerContext>,
}

impl ChannelHealthHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ChannelHealthHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Channel health request");

        let health = if let Some(ref manager) = self.context.channel_manager {
            let stats = manager.status().await;
            serde_json::json!({
                "running": stats.running,
                "channels_total": stats.channels_total,
                "channels_connected": stats.channels_connected,
                "channels_enabled": stats.channels_enabled,
                "queue_pending": stats.queue_pending,
                "queue_delivered": stats.queue_delivered,
            })
        } else {
            serde_json::json!({
                "running": false,
                "channels_total": 0,
                "channels_connected": 0,
                "channels_enabled": 0,
                "queue_pending": 0,
                "queue_delivered": 0,
            })
        };

        Ok(health)
    }
}
