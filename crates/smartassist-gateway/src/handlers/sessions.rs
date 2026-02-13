//! Session RPC method handlers.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Parameters for sessions.list method.
#[derive(Debug, Default, Deserialize)]
pub struct SessionsListParams {
    /// Filter by agent ID.
    pub agent_id: Option<String>,

    /// Filter by status.
    pub status: Option<String>,

    /// Maximum sessions to return.
    pub limit: Option<usize>,

    /// Offset for pagination.
    pub offset: Option<usize>,
}

/// Session info in list response.
#[derive(Debug, Serialize)]
pub struct SessionInfo {
    /// Session key.
    pub key: String,

    /// Agent ID.
    pub agent_id: Option<String>,

    /// Session status.
    pub status: String,

    /// Created timestamp.
    pub created_at: String,

    /// Last activity timestamp.
    pub last_activity: Option<String>,

    /// Message count.
    pub message_count: usize,
}

/// Sessions list method handler.
pub struct SessionsListHandler {
    context: Arc<HandlerContext>,
}

impl SessionsListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for SessionsListHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SessionsListParams = params
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        debug!("Sessions list request");

        let sessions = self.context.sessions.read().await;
        let limit = params.limit.unwrap_or(100);
        let offset = params.offset.unwrap_or(0);

        let mut session_infos: Vec<SessionInfo> = sessions
            .values()
            .filter(|s| {
                // Apply filters
                if let Some(ref agent_id) = params.agent_id {
                    if s.agent_id.as_ref() != Some(agent_id) {
                        return false;
                    }
                }
                if let Some(ref status) = params.status {
                    if &s.status != status {
                        return false;
                    }
                }
                true
            })
            .skip(offset)
            .take(limit)
            .map(|s| SessionInfo {
                key: s.key.clone(),
                agent_id: s.agent_id.clone(),
                status: s.status.clone(),
                created_at: s.created_at.to_rfc3339(),
                last_activity: s.last_activity.map(|t| t.to_rfc3339()),
                message_count: s.messages.len(),
            })
            .collect();

        session_infos.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(serde_json::json!({
            "sessions": session_infos,
            "total": session_infos.len(),
        }))
    }
}

/// Parameters for sessions.resolve method.
#[derive(Debug, Deserialize)]
pub struct SessionsResolveParams {
    /// Label or identifier to resolve.
    pub label: String,
}

/// Sessions resolve method handler.
pub struct SessionsResolveHandler {
    context: Arc<HandlerContext>,
}

impl SessionsResolveHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for SessionsResolveHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SessionsResolveParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Sessions resolve request for label: {}", params.label);

        let sessions = self.context.sessions.read().await;
        let session = sessions.get(&params.label);

        match session {
            Some(s) => Ok(serde_json::json!({
                "found": true,
                "session_key": s.key,
                "agent_id": s.agent_id,
            })),
            None => Ok(serde_json::json!({
                "found": false,
                "label": params.label,
            })),
        }
    }
}

/// Parameters for sessions.patch method.
#[derive(Debug, Deserialize)]
pub struct SessionsPatchParams {
    /// Session key.
    pub session_key: String,

    /// New status (optional).
    pub status: Option<String>,

    /// New agent ID (optional).
    pub agent_id: Option<String>,

    /// Metadata to merge (optional).
    pub metadata: Option<serde_json::Value>,
}

/// Sessions patch method handler.
pub struct SessionsPatchHandler {
    context: Arc<HandlerContext>,
}

impl SessionsPatchHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for SessionsPatchHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SessionsPatchParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Sessions patch request for: {}", params.session_key);

        let mut sessions = self.context.sessions.write().await;
        let session = sessions.get_mut(&params.session_key).ok_or_else(|| {
            GatewayError::NotFound(format!("Session '{}' not found", params.session_key))
        })?;

        // Apply status change if specified
        if let Some(status) = &params.status {
            match status.as_str() {
                "paused" | "active" | "archived" => {
                    session.status = status.clone();
                }
                _ => {
                    return Err(GatewayError::InvalidParams(format!(
                        "Invalid status: {}",
                        status
                    )));
                }
            }
        }

        // Apply agent_id change if specified
        if let Some(agent_id) = params.agent_id {
            session.agent_id = Some(agent_id);
        }

        session.last_activity = Some(chrono::Utc::now());

        Ok(serde_json::json!({
            "session_key": params.session_key,
            "patched": true,
        }))
    }
}

/// Parameters for sessions.delete method.
#[derive(Debug, Deserialize)]
pub struct SessionsDeleteParams {
    /// Session key.
    pub session_key: String,
}

/// Sessions delete method handler.
pub struct SessionsDeleteHandler {
    context: Arc<HandlerContext>,
}

impl SessionsDeleteHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for SessionsDeleteHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: SessionsDeleteParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Sessions delete request for: {}", params.session_key);

        let mut sessions = self.context.sessions.write().await;
        let deleted = sessions.remove(&params.session_key).is_some();

        Ok(serde_json::json!({
            "session_key": params.session_key,
            "deleted": deleted,
        }))
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for SessionsResolveParams {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for SessionsPatchParams {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for SessionsDeleteParams {
    type Error = serde_json::Error;

    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sessions_list_params_default() {
        let params = SessionsListParams::default();
        assert!(params.agent_id.is_none());
        assert!(params.limit.is_none());
    }
}
