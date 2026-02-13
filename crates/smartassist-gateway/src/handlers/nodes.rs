//! Node and device RPC method handlers.
//!
//! Handles pairing, management, and communication with paired devices/nodes.

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::debug;

/// Node info structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    /// Node ID.
    pub id: String,
    /// Node name.
    pub name: String,
    /// Node type.
    pub node_type: String,
    /// Paired status.
    pub paired: bool,
    /// Online status.
    pub online: bool,
    /// Last seen timestamp.
    pub last_seen: Option<String>,
}

/// Parameters for node.list method.
#[derive(Debug, Default, Deserialize)]
pub struct NodeListParams {
    /// Filter by online status.
    pub online: Option<bool>,
    /// Filter by node type.
    pub node_type: Option<String>,
}

/// Node list method handler.
pub struct NodeListHandler {
    _context: Arc<HandlerContext>,
}

impl NodeListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for NodeListHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: NodeListParams = params
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        debug!("Node list request: online={:?}", params.online);

        // TODO: Get nodes from node manager
        let nodes: Vec<NodeInfo> = vec![];

        Ok(serde_json::json!({
            "nodes": nodes,
            "count": nodes.len(),
        }))
    }
}

/// Parameters for node.describe method.
#[derive(Debug, Deserialize)]
pub struct NodeDescribeParams {
    /// Node ID.
    pub node_id: String,
}

/// Node describe method handler.
pub struct NodeDescribeHandler {
    _context: Arc<HandlerContext>,
}

impl NodeDescribeHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for NodeDescribeHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: NodeDescribeParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Node describe request for: {}", params.node_id);

        // TODO: Get node from node manager
        Err(GatewayError::NotFound(format!(
            "Node '{}' not found",
            params.node_id
        )))
    }
}

/// Parameters for node.pair.request method.
#[derive(Debug, Deserialize)]
pub struct NodePairRequestParams {
    /// Node ID to pair.
    pub node_id: String,
    /// Node name.
    pub name: Option<String>,
}

/// Node pair request method handler.
pub struct NodePairRequestHandler {
    _context: Arc<HandlerContext>,
}

impl NodePairRequestHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for NodePairRequestHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: NodePairRequestParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Node pair request for: {}", params.node_id);

        // Generate a pairing code
        let pairing_code = format!("{:06}", rand::random::<u32>() % 1_000_000);

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "pairing_code": pairing_code,
            "expires_at": chrono::Utc::now().checked_add_signed(chrono::Duration::minutes(10))
                .map(|t| t.to_rfc3339()),
        }))
    }
}

/// Parameters for node.pair.approve method.
#[derive(Debug, Deserialize)]
pub struct NodePairApproveParams {
    /// Node ID.
    pub node_id: String,
    /// Pairing code.
    pub pairing_code: String,
}

/// Node pair approve method handler.
pub struct NodePairApproveHandler {
    _context: Arc<HandlerContext>,
}

impl NodePairApproveHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for NodePairApproveHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: NodePairApproveParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!(
            "Node pair approve for: {} with code: {}",
            params.node_id, params.pairing_code
        );

        // TODO: Validate code and complete pairing

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "paired": true,
        }))
    }
}

/// Parameters for node.pair.reject method.
#[derive(Debug, Deserialize)]
pub struct NodePairRejectParams {
    /// Node ID.
    pub node_id: String,
}

/// Node pair reject method handler.
pub struct NodePairRejectHandler {
    _context: Arc<HandlerContext>,
}

impl NodePairRejectHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for NodePairRejectHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: NodePairRejectParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Node pair reject for: {}", params.node_id);

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "rejected": true,
        }))
    }
}

/// Parameters for node.unpair method.
#[derive(Debug, Deserialize)]
pub struct NodeUnpairParams {
    /// Node ID.
    pub node_id: String,
}

/// Node unpair method handler.
pub struct NodeUnpairHandler {
    _context: Arc<HandlerContext>,
}

impl NodeUnpairHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for NodeUnpairHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: NodeUnpairParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Node unpair for: {}", params.node_id);

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "unpaired": true,
        }))
    }
}

/// Parameters for node.rename method.
#[derive(Debug, Deserialize)]
pub struct NodeRenameParams {
    /// Node ID.
    pub node_id: String,
    /// New name.
    pub name: String,
}

/// Node rename method handler.
pub struct NodeRenameHandler {
    _context: Arc<HandlerContext>,
}

impl NodeRenameHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for NodeRenameHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: NodeRenameParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Node rename for {}: {}", params.node_id, params.name);

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "name": params.name,
            "renamed": true,
        }))
    }
}

/// Parameters for node.invoke method.
#[derive(Debug, Deserialize)]
pub struct NodeInvokeParams {
    /// Node ID.
    pub node_id: String,
    /// Command to invoke.
    pub command: String,
    /// Command arguments.
    pub args: Option<serde_json::Value>,
    /// Timeout in milliseconds.
    pub timeout_ms: Option<u64>,
}

/// Node invoke method handler.
pub struct NodeInvokeHandler {
    _context: Arc<HandlerContext>,
}

impl NodeInvokeHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { _context: context }
    }
}

#[async_trait]
impl MethodHandler for NodeInvokeHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: NodeInvokeParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!(
            "Node invoke on {}: {} with args: {:?}",
            params.node_id, params.command, params.args
        );

        // Generate invocation ID
        let invocation_id = uuid::Uuid::new_v4().to_string();

        // TODO: Actually invoke command on node

        Ok(serde_json::json!({
            "invocation_id": invocation_id,
            "node_id": params.node_id,
            "command": params.command,
            "status": "pending",
        }))
    }
}

// TryFrom implementations

impl TryFrom<serde_json::Value> for NodeDescribeParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for NodePairRequestParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for NodePairApproveParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for NodePairRejectParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for NodeUnpairParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for NodeRenameParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for NodeInvokeParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_info_serialization() {
        let node = NodeInfo {
            id: "node-1".to_string(),
            name: "My Node".to_string(),
            node_type: "desktop".to_string(),
            paired: true,
            online: true,
            last_seen: Some(chrono::Utc::now().to_rfc3339()),
        };

        let json = serde_json::to_value(&node).unwrap();
        assert_eq!(json["id"], "node-1");
        assert_eq!(json["paired"], true);
    }
}
