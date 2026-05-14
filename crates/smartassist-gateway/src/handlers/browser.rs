//! Browser automation RPC method handlers.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::Deserialize;
use smartassist_browser::{BrowserAction, LaunchOptions, Viewport};
use std::sync::Arc;
use tracing::debug;

/// Parameters for browser.launch method.
#[derive(Debug, Deserialize)]
pub struct BrowserLaunchParams {
    pub headless: Option<bool>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub sandboxed: Option<bool>,
}

/// Browser launch handler.
pub struct BrowserLaunchHandler {
    context: Arc<HandlerContext>,
}

impl BrowserLaunchHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for BrowserLaunchHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: BrowserLaunchParams = params
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        let manager = self
            .context
            .browser_manager
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Browser manager not configured".to_string()))?;

        let options = LaunchOptions {
            headless: params.headless.unwrap_or(true),
            viewport: Viewport {
                width: params.width.unwrap_or(1280),
                height: params.height.unwrap_or(720),
            },
            sandboxed: params.sandboxed.unwrap_or(true),
            proxy: None,
            user_agent: None,
            extra_args: Vec::new(),
        };

        let session = manager
            .launch(options)
            .await
            .map_err(|e| GatewayError::Internal(format!("Browser launch failed: {}", e)))?;

        debug!("Browser session launched: {}", session.id);

        Ok(serde_json::json!({
            "session_id": session.id,
            "headless": session.headless,
            "sandboxed": session.sandboxed,
        }))
    }
}

/// Parameters for browser.close method.
#[derive(Debug, Deserialize)]
pub struct BrowserCloseParams {
    pub session_id: String,
}

/// Browser close handler.
pub struct BrowserCloseHandler {
    context: Arc<HandlerContext>,
}

impl BrowserCloseHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for BrowserCloseHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: BrowserCloseParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        let manager = self
            .context
            .browser_manager
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Browser manager not configured".to_string()))?;

        manager
            .close(&params.session_id)
            .await
            .map_err(|e| GatewayError::Internal(format!("Browser close failed: {}", e)))?;

        Ok(serde_json::json!({
            "session_id": params.session_id,
            "closed": true,
        }))
    }
}

/// Parameters for browser.execute method.
#[derive(Debug, Deserialize)]
pub struct BrowserExecuteParams {
    pub session_id: String,
    pub action: BrowserAction,
}

/// Browser execute handler.
pub struct BrowserExecuteHandler {
    context: Arc<HandlerContext>,
}

impl BrowserExecuteHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for BrowserExecuteHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: BrowserExecuteParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        let manager = self
            .context
            .browser_manager
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Browser manager not configured".to_string()))?;

        let result = manager
            .execute(&params.session_id, params.action)
            .await
            .map_err(|e| GatewayError::Internal(format!("Browser execute failed: {}", e)))?;

        let mut response = serde_json::json!({
            "success": result.success,
            "data": result.data,
            "message": result.message,
        });

        if let Some(screenshot) = result.screenshot {
            use base64::engine::general_purpose::STANDARD;
            use base64::Engine;
            response["screenshot_base64"] =
                serde_json::json!(STANDARD.encode(&screenshot));
        }

        Ok(response)
    }
}

/// Browser list handler.
pub struct BrowserListHandler {
    context: Arc<HandlerContext>,
}

impl BrowserListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for BrowserListHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let manager = self
            .context
            .browser_manager
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Browser manager not configured".to_string()))?;

        let sessions = manager
            .list_sessions()
            .await
            .map_err(|e| GatewayError::Internal(format!("Browser list failed: {}", e)))?;

        let infos: Vec<serde_json::Value> = sessions
            .into_iter()
            .map(|s| {
                serde_json::json!({
                    "id": s.id,
                    "url": s.url,
                    "headless": s.headless,
                    "sandboxed": s.sandboxed,
                    "created_at": s.created_at.to_rfc3339(),
                })
            })
            .collect();

        Ok(serde_json::json!({
            "sessions": infos,
            "count": infos.len(),
        }))
    }
}

impl Default for BrowserLaunchParams {
    fn default() -> Self {
        Self {
            headless: None,
            width: None,
            height: None,
            sandboxed: None,
        }
    }
}

impl TryFrom<serde_json::Value> for BrowserCloseParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for BrowserExecuteParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_browser_list_without_manager_fails() {
        let ctx = Arc::new(HandlerContext::new());
        let handler = BrowserListHandler::new(ctx);
        let result = handler.call(None).await;
        assert!(result.is_err());
    }
}
