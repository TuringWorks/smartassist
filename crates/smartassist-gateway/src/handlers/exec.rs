//! Execution approval RPC method handlers.
//!
//! Handles command execution approval configuration and requests.
//! Includes an in-memory approval queue that allows callers to submit
//! approval requests and block until they are resolved (or time out).

use super::HandlerContext;
use crate::error::GatewayError;
use crate::methods::MethodHandler;
use crate::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, RwLock};
use tracing::debug;

// ---------------------------------------------------------------------------
// ApprovalConfig
// ---------------------------------------------------------------------------

/// Approval configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ApprovalConfig {
    /// Whether approval is required by default.
    pub require_approval: bool,
    /// Allowlist patterns that don't require approval.
    pub allowlist: Vec<String>,
    /// Denylist patterns that always require approval.
    pub denylist: Vec<String>,
    /// Timeout for approval requests in seconds.
    pub timeout_seconds: u64,
}

// ---------------------------------------------------------------------------
// PendingApproval + ApprovalQueue
// ---------------------------------------------------------------------------

/// A pending approval request stored in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingApproval {
    /// Request ID.
    pub id: String,
    /// Command to approve.
    pub command: String,
    /// Working directory.
    pub cwd: Option<String>,
    /// Agent ID.
    pub agent_id: String,
    /// Session key.
    pub session_key: String,
    /// Node ID (if from remote node).
    pub node_id: Option<String>,
    /// Request timestamp (RFC 3339).
    pub requested_at: String,
    /// Expiry timestamp (RFC 3339).
    pub expires_at: String,
}

/// Queue for managing exec approval requests.
///
/// Callers submit a [`PendingApproval`] via [`request`](ApprovalQueue::request)
/// and receive a [`oneshot::Receiver<bool>`] that resolves when another caller
/// invokes [`resolve`](ApprovalQueue::resolve) for the same request ID.
pub struct ApprovalQueue {
    pending: RwLock<HashMap<String, (PendingApproval, oneshot::Sender<bool>)>>,
}

impl ApprovalQueue {
    pub fn new() -> Self {
        Self {
            pending: RwLock::new(HashMap::new()),
        }
    }

    /// Submit an approval request. Returns a receiver that will get the result.
    pub async fn request(&self, approval: PendingApproval) -> oneshot::Receiver<bool> {
        let (tx, rx) = oneshot::channel();
        let id = approval.id.clone();
        let mut pending = self.pending.write().await;
        pending.insert(id, (approval, tx));
        rx
    }

    /// Resolve a pending approval by its request ID.
    pub async fn resolve(&self, request_id: &str, approved: bool) -> std::result::Result<(), String> {
        let mut pending = self.pending.write().await;
        match pending.remove(request_id) {
            Some((_, tx)) => {
                // The receiver may have been dropped (timeout), so ignore send errors.
                let _ = tx.send(approved);
                Ok(())
            }
            None => Err(format!("No pending approval with id: {}", request_id)),
        }
    }

    /// List all pending approvals (without consuming them).
    pub async fn list_pending(&self) -> Vec<PendingApproval> {
        let pending = self.pending.read().await;
        pending.values().map(|(a, _)| a.clone()).collect()
    }
}

impl Default for ApprovalQueue {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// ExecApprovalsGetHandler
// ---------------------------------------------------------------------------

/// Exec approvals get handler.
pub struct ExecApprovalsGetHandler {
    context: Arc<HandlerContext>,
}

impl ExecApprovalsGetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalsGetHandler {
    async fn call(&self, _params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        debug!("Exec approvals get request");

        // Try to read from config first; fall back to sensible defaults.
        if let Some(ref config_lock) = self.context.config {
            let config_value = config_lock.read().await;
            if let Some(exec_config) = config_value.get("exec_approvals") {
                return Ok(exec_config.clone());
            }
        }

        // Default allowlist is intentionally minimal -- `cat` was removed because it
        // can exfiltrate arbitrary files (e.g. `cat /etc/shadow`). `git status` is
        // kept because it is read-only, but `git push/reset/checkout` are on the
        // denylist.
        let config = ApprovalConfig {
            require_approval: true,
            allowlist: vec![
                "ls".to_string(),
                "pwd".to_string(),
                "echo".to_string(),
                "git status".to_string(),
                "git log".to_string(),
                "git diff".to_string(),
            ],
            denylist: vec![
                "rm -rf".to_string(),
                "rm -fr".to_string(),
                "sudo".to_string(),
                "su -".to_string(),
                "doas".to_string(),
                "chmod 777".to_string(),
                "git push".to_string(),
                "git reset".to_string(),
                "git checkout".to_string(),
                "curl".to_string(),
                "wget".to_string(),
                "ssh".to_string(),
                "nc ".to_string(),
                "ncat".to_string(),
            ],
            timeout_seconds: 30,
        };

        Ok(serde_json::to_value(config).unwrap())
    }
}

// ---------------------------------------------------------------------------
// ExecApprovalsSetHandler
// ---------------------------------------------------------------------------

/// Parameters for exec.approvals.set method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalsSetParams {
    /// Whether approval is required by default.
    pub require_approval: Option<bool>,
    /// Allowlist patterns.
    pub allowlist: Option<Vec<String>>,
    /// Denylist patterns.
    pub denylist: Option<Vec<String>>,
    /// Timeout in seconds.
    pub timeout_seconds: Option<u64>,
}

/// Exec approvals set handler.
pub struct ExecApprovalsSetHandler {
    context: Arc<HandlerContext>,
}

impl ExecApprovalsSetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalsSetHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalsSetParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Exec approvals set: {:?}", params.require_approval);

        let config = self
            .context
            .config
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Config not available".to_string()))?;

        let mut config_value = config.write().await;

        // Ensure exec_approvals object exists.
        if config_value.get("exec_approvals").is_none() {
            config_value
                .as_object_mut()
                .ok_or_else(|| GatewayError::Internal("Config is not an object".to_string()))?
                .insert(
                    "exec_approvals".to_string(),
                    serde_json::to_value(ApprovalConfig::default()).unwrap(),
                );
        }

        if let Some(exec_cfg) = config_value.get_mut("exec_approvals") {
            if let Some(v) = params.require_approval {
                exec_cfg["require_approval"] = serde_json::json!(v);
            }
            if let Some(v) = params.allowlist {
                exec_cfg["allowlist"] = serde_json::json!(v);
            }
            if let Some(v) = params.denylist {
                exec_cfg["denylist"] = serde_json::json!(v);
            }
            if let Some(v) = params.timeout_seconds {
                exec_cfg["timeout_seconds"] = serde_json::json!(v);
            }
        }

        // Persist to disk if a config path is configured.
        if let Some(ref path) = self.context.config_path {
            persist_config(&config_value, path).await?;
        }

        Ok(serde_json::json!({
            "updated": true,
        }))
    }
}

// ---------------------------------------------------------------------------
// ExecApprovalsNodeGetHandler
// ---------------------------------------------------------------------------

/// Parameters for exec.approvals.node.get method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalsNodeGetParams {
    /// Node ID.
    pub node_id: String,
}

/// Exec approvals node get handler.
pub struct ExecApprovalsNodeGetHandler {
    context: Arc<HandlerContext>,
}

impl ExecApprovalsNodeGetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalsNodeGetHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalsNodeGetParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Exec approvals node get: {}", params.node_id);

        // Try reading node-specific config from the config store.
        if let Some(ref config_lock) = self.context.config {
            let config_value = config_lock.read().await;
            if let Some(node_cfg) = config_value
                .get("exec_approvals_nodes")
                .and_then(|n| n.get(&params.node_id))
            {
                return Ok(serde_json::json!({
                    "node_id": params.node_id,
                    "config": node_cfg,
                }));
            }
        }

        let config = ApprovalConfig::default();
        Ok(serde_json::json!({
            "node_id": params.node_id,
            "config": config,
        }))
    }
}

// ---------------------------------------------------------------------------
// ExecApprovalsNodeSetHandler
// ---------------------------------------------------------------------------

/// Parameters for exec.approvals.node.set method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalsNodeSetParams {
    /// Node ID.
    pub node_id: String,
    /// Whether approval is required.
    pub require_approval: Option<bool>,
    /// Allowlist patterns.
    pub allowlist: Option<Vec<String>>,
    /// Denylist patterns.
    pub denylist: Option<Vec<String>>,
}

/// Exec approvals node set handler.
pub struct ExecApprovalsNodeSetHandler {
    context: Arc<HandlerContext>,
}

impl ExecApprovalsNodeSetHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalsNodeSetHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalsNodeSetParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Exec approvals node set: {}", params.node_id);

        let config = self
            .context
            .config
            .as_ref()
            .ok_or_else(|| GatewayError::Internal("Config not available".to_string()))?;

        let mut config_value = config.write().await;

        // Ensure the exec_approvals_nodes map exists.
        if config_value.get("exec_approvals_nodes").is_none() {
            config_value
                .as_object_mut()
                .ok_or_else(|| GatewayError::Internal("Config is not an object".to_string()))?
                .insert(
                    "exec_approvals_nodes".to_string(),
                    serde_json::json!({}),
                );
        }

        let nodes = config_value
            .get_mut("exec_approvals_nodes")
            .and_then(|n| n.as_object_mut())
            .ok_or_else(|| {
                GatewayError::Internal("exec_approvals_nodes is not an object".to_string())
            })?;

        let node_cfg = nodes
            .entry(params.node_id.clone())
            .or_insert_with(|| serde_json::to_value(ApprovalConfig::default()).unwrap());

        if let Some(v) = params.require_approval {
            node_cfg["require_approval"] = serde_json::json!(v);
        }
        if let Some(v) = params.allowlist {
            node_cfg["allowlist"] = serde_json::json!(v);
        }
        if let Some(v) = params.denylist {
            node_cfg["denylist"] = serde_json::json!(v);
        }

        Ok(serde_json::json!({
            "node_id": params.node_id,
            "updated": true,
        }))
    }
}

// ---------------------------------------------------------------------------
// ExecApprovalRequestHandler
// ---------------------------------------------------------------------------

/// Parameters for exec.approval.request method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalRequestParams {
    /// Command to execute.
    pub command: String,
    /// Working directory.
    pub cwd: Option<String>,
    /// Agent ID.
    pub agent_id: String,
    /// Session key.
    pub session_key: String,
    /// Timeout in milliseconds.
    pub timeout_ms: Option<u64>,
}

/// Exec approval request handler.
///
/// Submits an approval request to the queue and blocks until it is resolved or
/// the timeout expires.
pub struct ExecApprovalRequestHandler {
    context: Arc<HandlerContext>,
}

impl ExecApprovalRequestHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalRequestHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalRequestParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!("Exec approval request: {}", params.command);

        let request_id = uuid::Uuid::new_v4().to_string();
        let timeout_ms = params.timeout_ms.unwrap_or(300_000); // default 300s
        let now = chrono::Utc::now();
        let expires_at = now + chrono::Duration::milliseconds(timeout_ms as i64);

        let approval = PendingApproval {
            id: request_id.clone(),
            command: params.command.clone(),
            cwd: params.cwd,
            agent_id: params.agent_id,
            session_key: params.session_key,
            node_id: None,
            requested_at: now.to_rfc3339(),
            expires_at: expires_at.to_rfc3339(),
        };

        // Enqueue and get a receiver for the resolution.
        let rx = self.context.approval_queue.request(approval).await;

        // Wait for the approval to be resolved, or time out.
        match tokio::time::timeout(Duration::from_millis(timeout_ms), rx).await {
            Ok(Ok(approved)) => Ok(serde_json::json!({
                "request_id": request_id,
                "approved": approved,
                "timed_out": false,
            })),
            Ok(Err(_)) => {
                // Sender was dropped without sending (should not happen normally).
                Ok(serde_json::json!({
                    "request_id": request_id,
                    "approved": false,
                    "timed_out": false,
                    "error": "approval sender dropped",
                }))
            }
            Err(_) => {
                // Timed out -- clean up the pending entry.
                let _ = self.context.approval_queue.resolve(&request_id, false).await;
                Ok(serde_json::json!({
                    "request_id": request_id,
                    "approved": false,
                    "timed_out": true,
                }))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// ExecApprovalResolveHandler
// ---------------------------------------------------------------------------

/// Parameters for exec.approval.resolve method.
#[derive(Debug, Deserialize)]
pub struct ExecApprovalResolveParams {
    /// Request ID.
    pub request_id: String,
    /// Whether to approve.
    pub approved: bool,
    /// Optional reason for rejection.
    pub reason: Option<String>,
}

/// Exec approval resolve handler.
pub struct ExecApprovalResolveHandler {
    context: Arc<HandlerContext>,
}

impl ExecApprovalResolveHandler {
    pub fn new(context: Arc<HandlerContext>) -> Self {
        Self { context }
    }
}

#[async_trait]
impl MethodHandler for ExecApprovalResolveHandler {
    async fn call(&self, params: Option<serde_json::Value>) -> Result<serde_json::Value> {
        let params: ExecApprovalResolveParams = params
            .ok_or_else(|| GatewayError::InvalidParams("Missing parameters".to_string()))?
            .try_into()
            .map_err(|e: serde_json::Error| GatewayError::InvalidParams(e.to_string()))?;

        debug!(
            "Exec approval resolve: {} = {}",
            params.request_id, params.approved
        );

        self.context
            .approval_queue
            .resolve(&params.request_id, params.approved)
            .await
            .map_err(|e| GatewayError::NotFound(e))?;

        Ok(serde_json::json!({
            "request_id": params.request_id,
            "approved": params.approved,
            "resolved": true,
        }))
    }
}

// ---------------------------------------------------------------------------
// Config persistence helper (shared with config.rs)
// ---------------------------------------------------------------------------

/// Persist a config value to disk using atomic write (write-to-tmp then rename).
pub(crate) async fn persist_config(
    value: &serde_json::Value,
    path: &std::path::Path,
) -> Result<()> {
    let json = serde_json::to_string_pretty(value)
        .map_err(|e| GatewayError::Internal(format!("Failed to serialize config: {}", e)))?;
    let tmp = path.with_extension("tmp");
    tokio::fs::write(&tmp, json.as_bytes())
        .await
        .map_err(|e| GatewayError::Internal(format!("Failed to write temp config: {}", e)))?;
    tokio::fs::rename(&tmp, path)
        .await
        .map_err(|e| GatewayError::Internal(format!("Failed to rename config: {}", e)))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// TryFrom implementations
// ---------------------------------------------------------------------------

impl TryFrom<serde_json::Value> for ExecApprovalsSetParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ExecApprovalsNodeGetParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ExecApprovalsNodeSetParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ExecApprovalRequestParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

impl TryFrom<serde_json::Value> for ExecApprovalResolveParams {
    type Error = serde_json::Error;
    fn try_from(value: serde_json::Value) -> std::result::Result<Self, Self::Error> {
        serde_json::from_value(value)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approval_config_default() {
        let config = ApprovalConfig::default();
        assert!(!config.require_approval);
        assert!(config.allowlist.is_empty());
    }

    #[test]
    fn test_pending_approval_serialization() {
        let approval = PendingApproval {
            id: "req-1".to_string(),
            command: "rm -rf /tmp/test".to_string(),
            cwd: Some("/home/user".to_string()),
            agent_id: "agent-1".to_string(),
            session_key: "session-1".to_string(),
            node_id: None,
            requested_at: chrono::Utc::now().to_rfc3339(),
            expires_at: chrono::Utc::now().to_rfc3339(),
        };

        let json = serde_json::to_value(&approval).unwrap();
        assert_eq!(json["command"], "rm -rf /tmp/test");
    }

    #[tokio::test]
    async fn test_approval_queue_request_resolve() {
        let queue = ApprovalQueue::new();

        let approval = PendingApproval {
            id: "test-1".to_string(),
            command: "ls".to_string(),
            cwd: None,
            agent_id: "agent".to_string(),
            session_key: "sess".to_string(),
            node_id: None,
            requested_at: chrono::Utc::now().to_rfc3339(),
            expires_at: chrono::Utc::now().to_rfc3339(),
        };

        let rx = queue.request(approval).await;

        // Verify it appears in the pending list.
        let pending = queue.list_pending().await;
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, "test-1");

        // Resolve it.
        queue.resolve("test-1", true).await.unwrap();

        let result = rx.await.unwrap();
        assert!(result);

        // Pending list should now be empty.
        let pending = queue.list_pending().await;
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_approval_queue_resolve_unknown() {
        let queue = ApprovalQueue::new();
        let err = queue.resolve("nonexistent", true).await;
        assert!(err.is_err());
    }
}
