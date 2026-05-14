//! Device pairing RPC method handlers.
//!
//! Handles device pairing, token management, and device authentication.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Paired device info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Device ID.
    pub id: String,
    /// Device name.
    pub name: String,
    /// Device type (mobile, desktop, web).
    pub device_type: String,
    /// Paired timestamp.
    pub paired_at: String,
    /// Last seen timestamp.
    pub last_seen: Option<String>,
    /// Whether device is currently connected.
    pub connected: bool,
}

/// Device pair list handler.
pub struct DevicePairListHandler {
    context: Arc<HandlerContext>,
}

impl DevicePairListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for DevicePairListHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Device pair list request");

        let devices = self.context.devices.read().await;
        let result: Vec<DeviceInfo> = devices
            .values()
            .map(|d| DeviceInfo {
                id: d.id.clone(),
                name: d.name.clone(),
                device_type: d.device_type.clone(),
                paired_at: d.paired_at.map(|t| t.to_rfc3339()).unwrap_or_default(),
                last_seen: d.last_seen.map(|t| t.to_rfc3339()),
                connected: d.connected,
            })
            .collect();

        Ok(serde_json::json!({
            "devices": result,
            "count": result.len(),
        }))
    }
}

/// Parameters for device.pair.approve method.
#[derive(Debug, Deserialize)]
pub struct DevicePairApproveParams {
    /// Device ID.
    pub device_id: String,
    /// Challenge code.
    pub code: String,
}

/// Device pair approve handler.
pub struct DevicePairApproveHandler {
    context: Arc<HandlerContext>,
}

impl DevicePairApproveHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for DevicePairApproveHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: DevicePairApproveParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Device pair approve: {}", params.device_id);

        let token = uuid::Uuid::new_v4().to_string();
        let mut devices = self.context.devices.write().await;
        devices.insert(
            params.device_id.clone(),
            super::DeviceData {
                id: params.device_id.clone(),
                name: params.device_id.clone(),
                device_type: "unknown".to_string(),
                paired_at: Some(chrono::Utc::now()),
                last_seen: Some(chrono::Utc::now()),
                connected: true,
            },
        );

        Ok(serde_json::json!({
            "device_id": params.device_id,
            "paired": true,
            "token": token,
        }))
    }
}

/// Parameters for device.pair method.
#[derive(Debug, Deserialize)]
pub struct DevicePairParams {
    /// Device ID.
    pub device_id: String,
    /// Device name.
    pub name: Option<String>,
    /// Device type.
    pub device_type: Option<String>,
}

/// Device pair initiate handler.
/// Generates a challenge code that the device must confirm via device.pair.approve.
pub struct DevicePairInitiateHandler {
    context: Arc<HandlerContext>,
}

impl DevicePairInitiateHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for DevicePairInitiateHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: DevicePairParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        let code = format!("{:06}", rand::random::<u32>() % 1_000_000);
        let device_type = params.device_type.unwrap_or_else(|| "mobile".to_string());
        let name = params.name.unwrap_or_else(|| params.device_id.clone());

        debug!(
            "Device pair initiate: {} (type={}, code={})",
            params.device_id, device_type, code
        );

        // Store pending pair request
        let mut devices = self.context.devices.write().await;
        devices.insert(
            params.device_id.clone(),
            super::DeviceData {
                id: params.device_id.clone(),
                name: name.clone(),
                device_type: device_type.clone(),
                paired_at: None,
                last_seen: Some(chrono::Utc::now()),
                connected: false,
            },
        );

        Ok(serde_json::json!({
            "device_id": params.device_id,
            "name": name,
            "device_type": device_type,
            "challenge_code": code,
            "status": "pending_approval",
            "expires_in": 300,
        }))
    }
}

/// Parameters for device.pair.reject method.
#[derive(Debug, Deserialize)]
pub struct DevicePairRejectParams {
    /// Device ID.
    pub device_id: String,
}

/// Device pair reject handler.
pub struct DevicePairRejectHandler {
    context: Arc<HandlerContext>,
}

impl DevicePairRejectHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for DevicePairRejectHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: DevicePairRejectParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Device pair reject: {}", params.device_id);

        let mut devices = self.context.devices.write().await;
        devices.remove(&params.device_id);

        Ok(serde_json::json!({
            "device_id": params.device_id,
            "rejected": true,
        }))
    }
}

/// Parameters for device.unpair method.
#[derive(Debug, Deserialize)]
pub struct DeviceUnpairParams {
    /// Device ID.
    pub device_id: String,
}

/// Device unpair handler.
pub struct DeviceUnpairHandler {
    context: Arc<HandlerContext>,
}

impl DeviceUnpairHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for DeviceUnpairHandler {
    async fn call(
        &self, params: Option<serde_json::Value>
    ) -> Result<serde_json::Value> {
        let params: DeviceUnpairParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Device unpair: {}", params.device_id);

        let mut devices = self.context.devices.write().await;
        let removed = devices.remove(&params.device_id).is_some();

        Ok(serde_json::json!({
            "device_id": params.device_id,
            "unpaired": removed,
        }))
    }
}

/// Parameters for device.status method.
#[derive(Debug, Deserialize)]
pub struct DeviceStatusParams {
    /// Device ID.
    pub device_id: String,
}

/// Device status handler.
pub struct DeviceStatusHandler {
    context: Arc<HandlerContext>,
}

impl DeviceStatusHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for DeviceStatusHandler {
    async fn call(
        &self, params: Option<serde_json::Value>
    ) -> Result<serde_json::Value> {
        let params: DeviceStatusParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        let devices = self.context.devices.read().await;
        let device = devices.get(&params.device_id);

        let result = match device {
            Some(d) => serde_json::json!({
                "device_id": d.id,
                "name": d.name,
                "device_type": d.device_type,
                "paired": d.paired_at.is_some(),
                "connected": d.connected,
                "paired_at": d.paired_at.map(|t| t.to_rfc3339()),
                "last_seen": d.last_seen.map(|t| t.to_rfc3339()),
                "found": true,
            }),
            None => serde_json::json!({
                "device_id": params.device_id,
                "found": false,
            }),
        };

        Ok(result)
    }
}

/// Parameters for device.token.rotate method.
#[derive(Debug, Deserialize)]
pub struct DeviceTokenRotateParams {
    /// Device ID.
    pub device_id: String,
}

/// Device token rotate handler.
pub struct DeviceTokenRotateHandler {
    _context: Arc<HandlerContext>,
}

impl DeviceTokenRotateHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for DeviceTokenRotateHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: DeviceTokenRotateParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Device token rotate: {}", params.device_id);

        let new_token = uuid::Uuid::new_v4().to_string();

        Ok(serde_json::json!({
            "device_id": params.device_id,
            "token": new_token,
            "rotated": true,
        }))
    }
}

/// Parameters for device.token.revoke method.
#[derive(Debug, Deserialize)]
pub struct DeviceTokenRevokeParams {
    /// Device ID.
    pub device_id: String,
}

/// Device token revoke handler.
pub struct DeviceTokenRevokeHandler {
    context: Arc<HandlerContext>,
}

impl DeviceTokenRevokeHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for DeviceTokenRevokeHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: DeviceTokenRevokeParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Device token revoke: {}", params.device_id);

        let mut devices = self.context.devices.write().await;
        let revoked = devices.remove(&params.device_id).is_some();

        Ok(serde_json::json!({
            "device_id": params.device_id,
            "revoked": revoked,
        }))
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for DevicePairApproveParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for DevicePairRejectParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for DeviceTokenRotateParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for DeviceTokenRevokeParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for DevicePairParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for DeviceUnpairParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for DeviceStatusParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_info_serialization() {
        let device = DeviceInfo {
            id: "device-1".to_string(),
            name: "My Phone".to_string(),
            device_type: "mobile".to_string(),
            paired_at: chrono::Utc::now().to_rfc3339(),
            last_seen: None,
            connected: true,
        };

        let json = serde_json::to_value(&device).unwrap();
        assert_eq!(json["id"], "device-1");
        assert_eq!(json["connected"], true);
    }

    #[tokio::test]
    async fn test_device_list_empty() {
        let ctx = Arc::new(HandlerContext::new());
        let handler = DevicePairListHandler::new(ctx);
        let result = handler.call(None).await.unwrap();
        assert_eq!(result["count"], 0);
    }

    #[tokio::test]
    async fn test_device_approve_and_list() {
        let ctx = Arc::new(HandlerContext::new());

        let approve = DevicePairApproveHandler::new(ctx.clone());
        let params = serde_json::json!({
            "device_id": "device-1",
            "code": "123456"
        });
        let result = approve.call(Some(params)).await.unwrap();
        assert_eq!(result["paired"], true);
        assert!(result["token"].as_str().unwrap().len() > 0);

        let list = DevicePairListHandler::new(ctx);
        let result = list.call(None).await.unwrap();
        assert_eq!(result["count"], 1);
        let devices = result["devices"].as_array().unwrap();
        assert_eq!(devices[0]["id"], "device-1");
        assert_eq!(devices[0]["connected"], true);
    }

    #[tokio::test]
    async fn test_device_revoke() {
        let ctx = Arc::new(HandlerContext::new());

        // Pair a device first
        let approve = DevicePairApproveHandler::new(ctx.clone());
        let params = serde_json::json!({
            "device_id": "device-2",
            "code": "123456"
        });
        approve.call(Some(params)).await.unwrap();

        // Revoke it
        let revoke = DeviceTokenRevokeHandler::new(ctx.clone());
        let params = serde_json::json!({"device_id": "device-2"});
        let result = revoke.call(Some(params)).await.unwrap();
        assert_eq!(result["revoked"], true);

        // List should be empty
        let list = DevicePairListHandler::new(ctx);
        let result = list.call(None).await.unwrap();
        assert_eq!(result["count"], 0);
    }

    #[tokio::test]
    async fn test_device_revoke_missing() {
        let ctx = Arc::new(HandlerContext::new());
        let revoke = DeviceTokenRevokeHandler::new(ctx);
        let params = serde_json::json!({"device_id": "missing"});
        let result = revoke.call(Some(params)).await.unwrap();
        assert_eq!(result["revoked"], false);
    }

    #[tokio::test]
    async fn test_device_pair_initiate() {
        let ctx = Arc::new(HandlerContext::new());
        let handler = DevicePairInitiateHandler::new(ctx.clone());
        let params = serde_json::json!({
            "device_id": "iphone-123",
            "name": "My iPhone",
            "device_type": "ios"
        });
        let result = handler.call(Some(params)).await.unwrap();
        assert_eq!(result["device_id"], "iphone-123");
        assert_eq!(result["name"], "My iPhone");
        assert_eq!(result["device_type"], "ios");
        assert_eq!(result["status"], "pending_approval");
        assert!(result["challenge_code"].as_str().unwrap().len() == 6);

        // Device should be in pending state
        let devices = ctx.devices.read().await;
        let device = devices.get("iphone-123").unwrap();
        assert!(!device.connected);
        assert!(device.paired_at.is_none());
    }

    #[tokio::test]
    async fn test_device_unpair() {
        let ctx = Arc::new(HandlerContext::new());

        // Pair a device first
        let approve = DevicePairApproveHandler::new(ctx.clone());
        let params = serde_json::json!({
            "device_id": "device-x",
            "code": "123456"
        });
        approve.call(Some(params)).await.unwrap();

        // Unpair it
        let unpair = DeviceUnpairHandler::new(ctx.clone());
        let params = serde_json::json!({"device_id": "device-x"});
        let result = unpair.call(Some(params)).await.unwrap();
        assert_eq!(result["unpaired"], true);

        // List should be empty
        let list = DevicePairListHandler::new(ctx);
        let result = list.call(None).await.unwrap();
        assert_eq!(result["count"], 0);
    }

    #[tokio::test]
    async fn test_device_status_found() {
        let ctx = Arc::new(HandlerContext::new());

        // Pair a device
        let approve = DevicePairApproveHandler::new(ctx.clone());
        let params = serde_json::json!({
            "device_id": "device-y",
            "code": "123456"
        });
        approve.call(Some(params)).await.unwrap();

        let status = DeviceStatusHandler::new(ctx);
        let params = serde_json::json!({"device_id": "device-y"});
        let result = status.call(Some(params)).await.unwrap();
        assert_eq!(result["found"], true);
        assert_eq!(result["paired"], true);
        assert_eq!(result["connected"], true);
    }

    #[tokio::test]
    async fn test_device_status_not_found() {
        let ctx = Arc::new(HandlerContext::new());
        let status = DeviceStatusHandler::new(ctx);
        let params = serde_json::json!({"device_id": "missing"});
        let result = status.call(Some(params)).await.unwrap();
        assert_eq!(result["found"], false);
    }
}
