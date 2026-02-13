//! Audit logging types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// An audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Timestamp.
    pub timestamp: DateTime<Utc>,

    /// Event details.
    pub event: AuditEvent,

    /// Hostname (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hostname: Option<String>,
}

impl AuditEntry {
    /// Create a new audit entry.
    pub fn new(event: AuditEvent) -> Self {
        Self {
            timestamp: Utc::now(),
            event,
            hostname: hostname::get().ok().map(|h| h.to_string_lossy().to_string()),
        }
    }
}

/// An audit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    /// Event type.
    pub event_type: AuditEventType,

    /// Actor who triggered the event.
    pub actor: String,

    /// Session ID (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,

    /// Request ID (for tracing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,

    /// Event outcome.
    pub outcome: AuditOutcome,

    /// Additional details.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub details: Value,
}

impl AuditEvent {
    /// Create a new audit event.
    pub fn new(event_type: AuditEventType, actor: impl Into<String>, outcome: AuditOutcome) -> Self {
        Self {
            event_type,
            actor: actor.into(),
            session_id: None,
            request_id: None,
            outcome,
            details: Value::Null,
        }
    }

    /// Set session ID.
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set request ID.
    pub fn with_request(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Set details.
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }
}

/// Type of audit event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuditEventType {
    // Execution events
    /// Command execution requested.
    ExecCommandRequested {
        command: String,
        #[serde(default)]
        sandbox: bool,
    },

    /// Command execution approved.
    ExecCommandApproved { approval_id: String },

    /// Command execution denied.
    ExecCommandDenied {
        approval_id: String,
        reason: String,
    },

    /// Command execution completed.
    ExecCommandCompleted {
        exit_code: i32,
        duration_ms: u64,
    },

    // Authentication events
    /// Authentication succeeded.
    AuthSuccess {
        method: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        identity: Option<String>,
    },

    /// Authentication failed.
    AuthFailure { method: String, reason: String },

    // Channel events
    /// Channel login.
    ChannelLogin { channel: String, account: String },

    /// Channel logout.
    ChannelLogout { channel: String, account: String },

    /// Message sent.
    MessageSent { channel: String, target: String },

    // Security events
    /// Sandbox violation detected.
    SandboxViolation {
        violation_type: String,
        details: String,
    },

    /// Injection attempt detected.
    InjectionAttempt { pattern: String, source: String },

    /// Path traversal attempt.
    PathTraversalAttempt { path: String },

    /// Blocked environment variable.
    BlockedEnvVar { var_name: String },

    // Configuration events
    /// Configuration changed.
    ConfigChanged {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        old_value: Option<String>,
    },

    /// Credential accessed.
    CredentialAccessed { credential_id: String },

    // Session events
    /// Session created.
    SessionCreated { session_key: String },

    /// Session reset.
    SessionReset {
        session_key: String,
        reason: String,
    },

    // Agent events
    /// Agent invoked.
    AgentInvoked {
        agent_id: String,
        model: String,
    },

    /// Subagent spawned.
    SubagentSpawned {
        parent_agent: String,
        child_agent: String,
    },

    /// Tool executed.
    ToolExecuted {
        tool_name: String,
        #[serde(default)]
        success: bool,
    },
}

/// Outcome of an audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AuditOutcome {
    /// Operation succeeded.
    Success,

    /// Operation failed.
    Failure,

    /// Operation was denied.
    Denied,

    /// Operation timed out.
    Timeout,
}

/// Audit configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditConfig {
    /// Whether audit logging is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Path to audit log file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_path: Option<std::path::PathBuf>,

    /// Events to log.
    #[serde(default)]
    pub events: AuditEventFilter,
}

/// Filter for which events to audit.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditEventFilter {
    /// Log execution events.
    #[serde(default = "default_true")]
    pub exec: bool,

    /// Log authentication events.
    #[serde(default = "default_true")]
    pub auth: bool,

    /// Log channel events.
    #[serde(default = "default_true")]
    pub channel: bool,

    /// Log security events.
    #[serde(default = "default_true")]
    pub security: bool,

    /// Log configuration events.
    #[serde(default = "default_true")]
    pub config: bool,

    /// Log session events.
    #[serde(default)]
    pub session: bool,

    /// Log agent events.
    #[serde(default)]
    pub agent: bool,
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(
            AuditEventType::SessionCreated {
                session_key: "sess-1".to_string(),
            },
            "user-1",
            AuditOutcome::Success,
        );
        assert_eq!(event.actor, "user-1");
        assert_eq!(event.outcome, AuditOutcome::Success);
        assert!(event.session_id.is_none());
        assert!(event.request_id.is_none());
        assert!(event.details.is_null());
    }

    #[test]
    fn test_audit_event_with_session() {
        let event = AuditEvent::new(
            AuditEventType::ToolExecuted {
                tool_name: "read".to_string(),
                success: true,
            },
            "agent",
            AuditOutcome::Success,
        )
        .with_session("session-abc");

        assert_eq!(event.session_id.as_deref(), Some("session-abc"));
    }

    #[test]
    fn test_audit_event_with_request() {
        let event = AuditEvent::new(
            AuditEventType::ToolExecuted {
                tool_name: "exec".to_string(),
                success: false,
            },
            "agent",
            AuditOutcome::Failure,
        )
        .with_request("req-xyz");

        assert_eq!(event.request_id.as_deref(), Some("req-xyz"));
    }

    #[test]
    fn test_audit_event_with_details() {
        let details = serde_json::json!({"key": "value"});
        let event = AuditEvent::new(
            AuditEventType::ConfigChanged {
                key: "model".to_string(),
                old_value: Some("old".to_string()),
            },
            "admin",
            AuditOutcome::Success,
        )
        .with_details(details.clone());

        assert_eq!(event.details, details);
    }

    #[test]
    fn test_audit_event_type_serde_roundtrip() {
        let event_type = AuditEventType::ExecCommandRequested {
            command: "ls -la".to_string(),
            sandbox: true,
        };
        let json = serde_json::to_string(&event_type).unwrap();
        let parsed: AuditEventType = serde_json::from_str(&json).unwrap();
        match parsed {
            AuditEventType::ExecCommandRequested { command, sandbox } => {
                assert_eq!(command, "ls -la");
                assert!(sandbox);
            }
            _ => panic!("Wrong variant after deserialization"),
        }
    }

    #[test]
    fn test_audit_outcome_serde_roundtrip() {
        let outcomes = [
            AuditOutcome::Success,
            AuditOutcome::Failure,
            AuditOutcome::Denied,
            AuditOutcome::Timeout,
        ];
        for outcome in &outcomes {
            let json = serde_json::to_string(outcome).unwrap();
            let parsed: AuditOutcome = serde_json::from_str(&json).unwrap();
            assert_eq!(*outcome, parsed);
        }
    }

    #[test]
    fn test_audit_entry_creation() {
        let event = AuditEvent::new(
            AuditEventType::AuthSuccess {
                method: "password".to_string(),
                identity: Some("admin".to_string()),
            },
            "system",
            AuditOutcome::Success,
        );
        let entry = AuditEntry::new(event);
        assert_eq!(entry.event.actor, "system");
        // hostname may or may not be available in test environment, just check it exists.
    }
}
