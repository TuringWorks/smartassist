//! Canvas workspace RPC method handlers.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::Deserialize;
use smartassist_canvas::CanvasAction;
use std::sync::Arc;
use tracing::debug;

/// Parameters for canvas.create method.
#[derive(Debug, Deserialize)]
pub struct CanvasCreateParams {
    pub id: String,
    pub agent_id: Option<String>,
}

/// Canvas create handler.
pub struct CanvasCreateHandler {
    context: Arc<HandlerContext>,
}

impl CanvasCreateHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CanvasCreateHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CanvasCreateParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        let manager = self
            .context
            .canvas_manager
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Canvas manager not configured".to_string()))?;

        let surface = manager
            .create_surface(&params.id,
                params.agent_id.map(smartassist_core::types::AgentId::new),
                None,
            )
            .await
            .map_err(|e| GatewayError::Internal(format!("Canvas create failed: {}", e)))?;

        debug!("Canvas surface created: {}", surface.id);

        Ok(serde_json::json!({
            "id": surface.id,
            "created": true,
        }))
    }
}

/// Parameters for canvas.delete method.
#[derive(Debug, Deserialize)]
pub struct CanvasDeleteParams {
    pub id: String,
}

/// Canvas delete handler.
pub struct CanvasDeleteHandler {
    context: Arc<HandlerContext>,
}

impl CanvasDeleteHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CanvasDeleteHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CanvasDeleteParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        let manager = self
            .context
            .canvas_manager
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Canvas manager not configured".to_string()))?;

        manager
            .delete_surface(&params.id)
            .await
            .map_err(|e| GatewayError::Internal(format!("Canvas delete failed: {}", e)))?;

        Ok(serde_json::json!({
            "id": params.id,
            "deleted": true,
        }))
    }
}

/// Parameters for canvas.execute method.
#[derive(Debug, Deserialize)]
pub struct CanvasExecuteParams {
    pub id: String,
    pub action: CanvasAction,
}

/// Canvas execute handler.
pub struct CanvasExecuteHandler {
    context: Arc<HandlerContext>,
}

impl CanvasExecuteHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CanvasExecuteHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CanvasExecuteParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        let manager = self
            .context
            .canvas_manager
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Canvas manager not configured".to_string()))?;

        let result = manager
            .execute(&params.id, params.action)
            .await
            .map_err(|e| GatewayError::Internal(format!("Canvas execute failed: {}", e)))?;

        Ok(serde_json::json!({
            "success": result.success,
            "data": result.data,
            "message": result.message,
        }))
    }
}

/// Canvas list handler.
pub struct CanvasListHandler {
    context: Arc<HandlerContext>,
}

impl CanvasListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CanvasListHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let manager = self
            .context
            .canvas_manager
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Canvas manager not configured".to_string()))?;

        let surfaces = manager
            .list_surfaces()
            .await
            .map_err(|e| GatewayError::Internal(format!("Canvas list failed: {}", e)))?;

        let infos: Vec<serde_json::Value> = surfaces
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "agent_id": s.agent_id.map(|a| a.as_str().to_string()),
                    "element_count": s.elements.len(),
                    "subscriber_count": s.subscribers.len(),
                    "created_at": s.created_at.to_rfc3339(),
                    "updated_at": s.updated_at.to_rfc3339(),
                })
            })
            .collect();

        Ok(serde_json::json!({
            "surfaces": infos,
            "count": infos.len(),
        }))
    }
}

/// Parameters for canvas.subscribe method.
#[derive(Debug, Deserialize)]
pub struct CanvasSubscribeParams {
    pub id: String,
    pub client_id: String,
}

/// Canvas subscribe handler.
pub struct CanvasSubscribeHandler {
    context: Arc<HandlerContext>,
}

impl CanvasSubscribeHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for CanvasSubscribeHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: CanvasSubscribeParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        let manager = self
            .context
            .canvas_manager
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Canvas manager not configured".to_string()))?;

        manager
            .subscribe(&params.id, &params.client_id)
            .await
            .map_err(|e| GatewayError::Internal(format!("Canvas subscribe failed: {}", e)))?;

        Ok(serde_json::json!({
            "id": params.id,
            "client_id": params.client_id,
            "subscribed": true,
        }))
    }
}

impl TryFrom<serde_json::Value> for CanvasCreateParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for CanvasDeleteParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for CanvasExecuteParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for CanvasSubscribeParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_canvas_list_without_manager_fails() {
        let ctx = Arc::new(HandlerContext::new());
        let handler = CanvasListHandler::new(ctx);
        let result = handler.call(None).await;
        assert!(result.is_err());
    }
}
