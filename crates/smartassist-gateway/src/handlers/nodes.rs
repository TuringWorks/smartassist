//! Node and device RPC method handlers.
//!
//! Handles pairing, management, and communication with paired devices/nodes.

use super::{HandlerContext, NodeData};
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
    context: Arc<HandlerContext>,
}

impl NodeListHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for NodeListHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: NodeListParams = params
            .map(|v| serde_json::from_value(v).unwrap_or_default())
            .unwrap_or_default();

        debug!("Node list request: online={:?}", params.online);

        let nodes = self.context.nodes.read().await;
        let mut result: Vec<NodeInfo> = nodes
            .values()
            .filter(|n| {
                if let Some(online) = params.online {
                    if n.online != online {
                        return false;
                    }
                }
                if let Some(ref node_type) = params.node_type {
                    if &n.node_type != node_type {
                        return false;
                    }
                }
                true
            })
            .map(|n| NodeInfo {
                id: n.id.clone(),
                name: n.name.clone(),
                node_type: n.node_type.clone(),
                paired: n.paired,
                online: n.online,
                last_seen: n.last_seen.map(|t| t.to_rfc3339()),
            })
            .collect();
        result.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(serde_json::json!({
            "nodes": result,
            "count": result.len(),
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
    context: Arc<HandlerContext>,
}

impl NodeDescribeHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
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

        let nodes = self.context.nodes.read().await;
        let node = nodes.get(&params.node_id).ok_or_else(|| {
            GatewayError::NotFound(format!("Node '{}' not found", params.node_id))
        })?;

        let info = NodeInfo {
            id: node.id.clone(),
            name: node.name.clone(),
            node_type: node.node_type.clone(),
            paired: node.paired,
            online: node.online,
            last_seen: node.last_seen.map(|t| t.to_rfc3339()),
        };

        Ok(serde_json::to_value(info).unwrap())
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
    context: Arc<HandlerContext>,
}

impl NodePairApproveHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
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

        let mut nodes = self.context.nodes.write().await;
        nodes.insert(
            params.node_id.clone(),
            NodeData {
                id: params.node_id.clone(),
                name: params.node_id.clone(),
                node_type: "unknown".to_string(),
                paired: true,
                online: true,
                last_seen: Some(chrono::Utc::now()),
            },
        );

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
    context: Arc<HandlerContext>,
}

impl NodeUnpairHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
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

        let mut nodes = self.context.nodes.write().await;
        let removed = nodes.remove(&params.node_id).is_some();

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "unpaired": removed,
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
    context: Arc<HandlerContext>,
}

impl NodeRenameHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
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

        let mut nodes = self.context.nodes.write().await;
        let renamed = if let Some(node) = nodes.get_mut(&params.node_id) {
            node.name = params.name.clone();
            true
        } else {
            false
        };

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "name": params.name,
            "renamed": renamed,
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
    use crate::handlers::NodeData;

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

    #[tokio::test]
    async fn test_node_list_empty() {
        let ctx = Arc::new(HandlerContext::new());
        let handler = NodeListHandler::new(ctx);
        let result = handler.call(None).await.unwrap();
        assert_eq!(result["count"], 0);
    }

    #[tokio::test]
    async fn test_node_approve_and_list() {
        let ctx = Arc::new(HandlerContext::new());

        // Approve a node
        let approve = NodePairApproveHandler::new(ctx.clone());
        let params = serde_json::json!({
            "node_id": "node-1",
            "pairing_code": "123456"
        });
        let result = approve.call(Some(params)).await.unwrap();
        assert_eq!(result["paired"], true);

        // List should now contain the node
        let list = NodeListHandler::new(ctx.clone());
        let result = list.call(None).await.unwrap();
        assert_eq!(result["count"], 1);
        let nodes = result["nodes"].as_array().unwrap();
        assert_eq!(nodes[0]["id"], "node-1");
        assert_eq!(nodes[0]["paired"], true);
    }

    #[tokio::test]
    async fn test_node_describe_found() {
        let ctx = Arc::new(HandlerContext::new());

        // Insert a node directly
        {
            let mut nodes = ctx.nodes.write().await;
            nodes.insert("node-a".to_string(), NodeData {
                id: "node-a".to_string(),
                name: "Alpha".to_string(),
                node_type: "desktop".to_string(),
                paired: true,
                online: true,
                last_seen: Some(chrono::Utc::now()),
            });
        }

        let handler = NodeDescribeHandler::new(ctx);
        let params = serde_json::json!({"node_id": "node-a"});
        let result = handler.call(Some(params)).await.unwrap();
        assert_eq!(result["id"], "node-a");
        assert_eq!(result["name"], "Alpha");
    }

    #[tokio::test]
    async fn test_node_describe_not_found() {
        let ctx = Arc::new(HandlerContext::new());
        let handler = NodeDescribeHandler::new(ctx);
        let params = serde_json::json!({"node_id": "missing"});
        let result = handler.call(Some(params)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_node_rename() {
        let ctx = Arc::new(HandlerContext::new());

        // Insert a node
        {
            let mut nodes = ctx.nodes.write().await;
            nodes.insert("node-b".to_string(), NodeData {
                id: "node-b".to_string(),
                name: "Old Name".to_string(),
                node_type: "mobile".to_string(),
                paired: true,
                online: false,
                last_seen: None,
            });
        }

        let handler = NodeRenameHandler::new(ctx.clone());
        let params = serde_json::json!({
            "node_id": "node-b",
            "name": "New Name"
        });
        let result = handler.call(Some(params)).await.unwrap();
        assert_eq!(result["renamed"], true);

        // Verify via describe
        let describe = NodeDescribeHandler::new(ctx);
        let params = serde_json::json!({"node_id": "node-b"});
        let result = describe.call(Some(params)).await.unwrap();
        assert_eq!(result["name"], "New Name");
    }

    #[tokio::test]
    async fn test_node_unpair() {
        let ctx = Arc::new(HandlerContext::new());

        // Insert a node
        {
            let mut nodes = ctx.nodes.write().await;
            nodes.insert("node-c".to_string(), NodeData {
                id: "node-c".to_string(),
                name: "Charlie".to_string(),
                node_type: "server".to_string(),
                paired: true,
                online: true,
                last_seen: Some(chrono::Utc::now()),
            });
        }

        let handler = NodeUnpairHandler::new(ctx.clone());
        let params = serde_json::json!({"node_id": "node-c"});
        let result = handler.call(Some(params)).await.unwrap();
        assert_eq!(result["unpaired"], true);

        // List should now be empty
        let list = NodeListHandler::new(ctx);
        let result = list.call(None).await.unwrap();
        assert_eq!(result["count"], 0);
    }
}
