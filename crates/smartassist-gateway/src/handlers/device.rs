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
    _context: Arc<HandlerContext>,
}

impl DevicePairListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for DevicePairListHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Device pair list request");

        // TODO: Get devices from device manager
        let devices: Vec<DeviceInfo> = vec![];

        Ok(serde_json::json!({
            "devices": devices,
            "count": devices.len(),
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
    _context: Arc<HandlerContext>,
}

impl DevicePairApproveHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
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

        // TODO: Validate code and complete pairing

        Ok(serde_json::json!({
            "device_id": params.device_id,
            "paired": true,
            "token": uuid::Uuid::new_v4().to_string(),
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
    _context: Arc<HandlerContext>,
}

impl DevicePairRejectHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
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

        Ok(serde_json::json!({
            "device_id": params.device_id,
            "rejected": true,
        }))
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
    _context: Arc<HandlerContext>,
}

impl DeviceTokenRevokeHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
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

        Ok(serde_json::json!({
            "device_id": params.device_id,
            "revoked": true,
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
}
