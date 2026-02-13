//! Tool approval workflow.

use crate::error::AgentError;
use crate::Result;
use chrono::{DateTime, Duration, Utc};
use smartassist_core::types::ApprovalId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::{broadcast, RwLock};
use tracing::{debug, warn};

/// Manager for tool approval requests.
pub struct ApprovalManager {
    /// Pending approval requests.
    pending: RwLock<HashMap<ApprovalId, ApprovalRequest>>,

    /// Approval response sender.
    response_tx: broadcast::Sender<ApprovalEvent>,

    /// Default timeout for approvals.
    default_timeout: Duration,

    /// Approval policy.
    policy: RwLock<ApprovalPolicy>,
}

impl Default for ApprovalManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ApprovalManager {
    /// Create a new approval manager.
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        Self {
            pending: RwLock::new(HashMap::new()),
            response_tx: tx,
            default_timeout: Duration::minutes(5),
            policy: RwLock::new(ApprovalPolicy::default()),
        }
    }

    /// Set the default timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Set the approval policy.
    pub async fn set_policy(&self, policy: ApprovalPolicy) {
        let mut p = self.policy.write().await;
        *p = policy;
    }

    /// Request approval for a tool execution.
    pub async fn request(
        &self,
        session_id: String,
        tool_name: String,
        tool_args: serde_json::Value,
        description: String,
    ) -> Result<ApprovalRequest> {
        let policy = self.policy.read().await;

        // Check if auto-approved by policy
        if policy.is_auto_approved(&tool_name, &tool_args) {
            debug!("Tool '{}' auto-approved by policy", tool_name);
            return Ok(ApprovalRequest {
                id: ApprovalId::new(),
                session_id,
                tool_name,
                tool_args,
                description,
                status: ApprovalStatus::Approved,
                created_at: Utc::now(),
                expires_at: Utc::now(),
                response: Some(ApprovalResponse {
                    approved: true,
                    reason: Some("Auto-approved by policy".to_string()),
                    modifications: None,
                    responded_at: Utc::now(),
                }),
            });
        }

        // Check if auto-denied by policy
        if policy.is_auto_denied(&tool_name, &tool_args) {
            debug!("Tool '{}' auto-denied by policy", tool_name);
            return Ok(ApprovalRequest {
                id: ApprovalId::new(),
                session_id,
                tool_name,
                tool_args,
                description,
                status: ApprovalStatus::Denied,
                created_at: Utc::now(),
                expires_at: Utc::now(),
                response: Some(ApprovalResponse {
                    approved: false,
                    reason: Some("Denied by policy".to_string()),
                    modifications: None,
                    responded_at: Utc::now(),
                }),
            });
        }

        // Create pending request
        let now = Utc::now();
        let request = ApprovalRequest {
            id: ApprovalId::new(),
            session_id,
            tool_name,
            tool_args,
            description,
            status: ApprovalStatus::Pending,
            created_at: now,
            expires_at: now + self.default_timeout,
            response: None,
        };

        let mut pending = self.pending.write().await;
        pending.insert(request.id.clone(), request.clone());

        // Broadcast the request event
        let _ = self.response_tx.send(ApprovalEvent::Requested(request.clone()));

        debug!("Created approval request: {}", request.id);
        Ok(request)
    }

    /// Respond to an approval request.
    pub async fn respond(&self, id: &ApprovalId, response: ApprovalResponse) -> Result<()> {
        let mut pending = self.pending.write().await;

        let request = pending
            .get_mut(id)
            .ok_or_else(|| AgentError::Internal(format!("Approval request not found: {}", id)))?;

        if request.status != ApprovalStatus::Pending {
            return Err(AgentError::InvalidState(format!(
                "Approval request {} is not pending",
                id
            )));
        }

        request.status = if response.approved {
            ApprovalStatus::Approved
        } else {
            ApprovalStatus::Denied
        };
        request.response = Some(response.clone());

        // Broadcast the response event
        let _ = self.response_tx.send(ApprovalEvent::Responded {
            id: id.clone(),
            approved: response.approved,
        });

        debug!("Approval request {} responded: approved={}", id, response.approved);
        Ok(())
    }

    /// Get a pending approval request.
    pub async fn get(&self, id: &ApprovalId) -> Option<ApprovalRequest> {
        let pending = self.pending.read().await;
        pending.get(id).cloned()
    }

    /// List pending approval requests for a session.
    pub async fn list_pending(&self, session_id: &str) -> Vec<ApprovalRequest> {
        let pending = self.pending.read().await;
        pending
            .values()
            .filter(|r| r.session_id == session_id && r.status == ApprovalStatus::Pending)
            .cloned()
            .collect()
    }

    /// Wait for an approval response.
    pub async fn wait_for_response(
        &self,
        id: &ApprovalId,
        timeout: Option<Duration>,
    ) -> Result<ApprovalResponse> {
        let timeout = timeout.unwrap_or(self.default_timeout);
        let deadline = Utc::now() + timeout;

        let mut rx = self.response_tx.subscribe();

        loop {
            // Check if already responded
            if let Some(request) = self.get(id).await {
                if let Some(response) = request.response {
                    return Ok(response);
                }
            }

            // Check timeout
            if Utc::now() >= deadline {
                // Mark as expired
                let mut pending = self.pending.write().await;
                if let Some(request) = pending.get_mut(id) {
                    request.status = ApprovalStatus::Expired;
                }
                return Err(AgentError::ApprovalTimeout(id.to_string()));
            }

            // Wait for event
            let wait_time = (deadline - Utc::now())
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(1));

            match tokio::time::timeout(wait_time, rx.recv()).await {
                Ok(Ok(ApprovalEvent::Responded { id: event_id, approved: _ })) => {
                    if &event_id == id {
                        if let Some(request) = self.get(id).await {
                            if let Some(response) = request.response {
                                return Ok(response);
                            }
                        }
                    }
                }
                _ => continue,
            }
        }
    }

    /// Clean up expired requests.
    pub async fn cleanup_expired(&self) {
        let now = Utc::now();
        let mut pending = self.pending.write().await;

        let expired: Vec<ApprovalId> = pending
            .iter()
            .filter(|(_, r)| r.status == ApprovalStatus::Pending && r.expires_at < now)
            .map(|(id, _)| id.clone())
            .collect();

        for id in expired {
            if let Some(request) = pending.get_mut(&id) {
                request.status = ApprovalStatus::Expired;
                warn!("Approval request {} expired", id);
            }
        }
    }

    /// Check if a tool requires approval based on policy (synchronous check).
    pub fn requires_approval(&self, _tool_name: &str, _tool_args: &serde_json::Value) -> bool {
        // For now, return false - actual policy check would need async
        // This is a simplified check; real implementation should be async
        false
    }

    /// Subscribe to approval events.
    pub fn subscribe(&self) -> broadcast::Receiver<ApprovalEvent> {
        self.response_tx.subscribe()
    }
}

/// An approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Unique request ID.
    pub id: ApprovalId,

    /// Session ID.
    pub session_id: String,

    /// Tool name.
    pub tool_name: String,

    /// Tool arguments.
    pub tool_args: serde_json::Value,

    /// Human-readable description.
    pub description: String,

    /// Current status.
    pub status: ApprovalStatus,

    /// Creation timestamp.
    pub created_at: DateTime<Utc>,

    /// Expiration timestamp.
    pub expires_at: DateTime<Utc>,

    /// Response (if responded).
    pub response: Option<ApprovalResponse>,
}

/// Approval request status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalStatus {
    /// Waiting for response.
    Pending,

    /// Approved.
    Approved,

    /// Denied.
    Denied,

    /// Expired without response.
    Expired,
}

/// Response to an approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalResponse {
    /// Whether approved.
    pub approved: bool,

    /// Reason for the decision.
    pub reason: Option<String>,

    /// Modifications to the tool args (if approved with changes).
    pub modifications: Option<serde_json::Value>,

    /// Response timestamp.
    pub responded_at: DateTime<Utc>,
}

impl ApprovalResponse {
    /// Create an approval response.
    pub fn approve() -> Self {
        Self {
            approved: true,
            reason: None,
            modifications: None,
            responded_at: Utc::now(),
        }
    }

    /// Create a denial response.
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            approved: false,
            reason: Some(reason.into()),
            modifications: None,
            responded_at: Utc::now(),
        }
    }

    /// Add a reason.
    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Add modifications (for approved with changes).
    pub fn with_modifications(mut self, mods: serde_json::Value) -> Self {
        self.modifications = Some(mods);
        self
    }
}

/// Approval event for broadcasting.
#[derive(Debug, Clone)]
pub enum ApprovalEvent {
    /// New request created.
    Requested(ApprovalRequest),

    /// Request responded to.
    Responded {
        /// Request ID.
        id: ApprovalId,
        /// Whether approved.
        approved: bool,
    },
}

/// Approval policy configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ApprovalPolicy {
    /// Tools that are always auto-approved.
    #[serde(default)]
    pub auto_approve: Vec<String>,

    /// Tools that are always auto-denied.
    #[serde(default)]
    pub auto_deny: Vec<String>,

    /// Pattern-based auto-approve rules.
    #[serde(default)]
    pub auto_approve_patterns: Vec<PolicyPattern>,

    /// Pattern-based auto-deny rules.
    #[serde(default)]
    pub auto_deny_patterns: Vec<PolicyPattern>,
}

/// A pattern-based policy rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyPattern {
    /// Tool name pattern (regex).
    pub tool_pattern: String,

    /// Argument patterns (JSON path -> regex).
    #[serde(default)]
    pub arg_patterns: HashMap<String, String>,
}

impl ApprovalPolicy {
    /// Check if a tool is auto-approved.
    pub fn is_auto_approved(&self, tool: &str, _args: &serde_json::Value) -> bool {
        if self.auto_approve.contains(&tool.to_string()) {
            return true;
        }

        // Check patterns
        for pattern in &self.auto_approve_patterns {
            if let Ok(re) = regex::Regex::new(&pattern.tool_pattern) {
                if re.is_match(tool) {
                    // TODO: Check arg patterns
                    return true;
                }
            }
        }

        false
    }

    /// Check if a tool is auto-denied.
    pub fn is_auto_denied(&self, tool: &str, _args: &serde_json::Value) -> bool {
        if self.auto_deny.contains(&tool.to_string()) {
            return true;
        }

        // Check patterns
        for pattern in &self.auto_deny_patterns {
            if let Ok(re) = regex::Regex::new(&pattern.tool_pattern) {
                if re.is_match(tool) {
                    // TODO: Check arg patterns
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_approval_request() {
        let manager = ApprovalManager::new();

        let request = manager
            .request(
                "session1".to_string(),
                "bash".to_string(),
                serde_json::json!({"command": "rm -rf /tmp/test"}),
                "Delete test files".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(request.status, ApprovalStatus::Pending);
    }

    #[tokio::test]
    async fn test_approval_response() {
        let manager = ApprovalManager::new();

        let request = manager
            .request(
                "session1".to_string(),
                "bash".to_string(),
                serde_json::json!({"command": "ls"}),
                "List files".to_string(),
            )
            .await
            .unwrap();

        manager
            .respond(&request.id, ApprovalResponse::approve())
            .await
            .unwrap();

        let updated = manager.get(&request.id).await.unwrap();
        assert_eq!(updated.status, ApprovalStatus::Approved);
    }

    #[tokio::test]
    async fn test_auto_approve_policy() {
        let manager = ApprovalManager::new();
        manager
            .set_policy(ApprovalPolicy {
                auto_approve: vec!["read".to_string()],
                ..Default::default()
            })
            .await;

        let request = manager
            .request(
                "session1".to_string(),
                "read".to_string(),
                serde_json::json!({"path": "/tmp/test.txt"}),
                "Read a file".to_string(),
            )
            .await
            .unwrap();

        assert_eq!(request.status, ApprovalStatus::Approved);
    }
}
