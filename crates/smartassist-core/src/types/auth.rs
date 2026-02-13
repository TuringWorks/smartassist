//! Authentication and authorization types.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Authentication context for a client.
#[derive(Debug, Clone)]
pub struct AuthContext {
    /// Client identifier.
    pub client_id: String,

    /// Granted scopes.
    pub scopes: HashSet<Scope>,

    /// Identity information (if available).
    pub identity: Option<Identity>,

    /// When authentication occurred.
    pub authenticated_at: DateTime<Utc>,
}

impl AuthContext {
    /// Create a new auth context with full admin access.
    pub fn admin(client_id: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            scopes: [Scope::Admin, Scope::Read, Scope::Write, Scope::Approvals, Scope::Pairing]
                .into_iter()
                .collect(),
            identity: None,
            authenticated_at: Utc::now(),
        }
    }

    /// Create a loopback auth context (localhost).
    pub fn loopback() -> Self {
        Self::admin("loopback")
    }

    /// Create a Tailscale auth context.
    pub fn tailscale(identity: Identity) -> Self {
        let client_id = identity.user_id.clone();
        Self {
            client_id,
            scopes: [Scope::Admin, Scope::Read, Scope::Write, Scope::Approvals, Scope::Pairing]
                .into_iter()
                .collect(),
            identity: Some(identity),
            authenticated_at: Utc::now(),
        }
    }

    /// Check if a scope is granted.
    pub fn has_scope(&self, scope: Scope) -> bool {
        self.scopes.contains(&Scope::Admin) || self.scopes.contains(&scope)
    }

    /// Check if all required scopes are granted.
    pub fn has_all_scopes(&self, required: &[Scope]) -> bool {
        required.iter().all(|s| self.has_scope(*s))
    }
}

/// Authorization scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Full administrative access.
    Admin,

    /// Read-only access (status, logs, history).
    Read,

    /// Write access (send, agent, models).
    Write,

    /// Execution approval access.
    Approvals,

    /// Device/node pairing access.
    Pairing,
}

impl Scope {
    /// Get all scopes.
    pub fn all() -> &'static [Scope] {
        &[Self::Admin, Self::Read, Self::Write, Self::Approvals, Self::Pairing]
    }
}

/// Identity information from authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identity {
    /// User ID.
    pub user_id: String,

    /// Username (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,

    /// Email (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,

    /// Authentication provider.
    pub provider: String,
}

/// Approval request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRequest {
    /// Approval ID.
    pub id: super::ApprovalId,

    /// Command to be executed.
    pub command: String,

    /// Working directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,

    /// Agent ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,

    /// Session key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,

    /// When the request was created.
    pub created_at: DateTime<Utc>,

    /// When the request expires.
    pub expires_at: DateTime<Utc>,
}

/// Response to an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "lowercase")]
pub enum ApprovalResponse {
    /// Request approved.
    Approved,

    /// Request denied.
    Denied,

    /// Request timed out.
    Timeout,
}

impl ApprovalResponse {
    /// Check if approved.
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::Approved)
    }
}

/// Execution security configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecSecurityConfig {
    /// Execution mode.
    #[serde(default)]
    pub mode: ExecMode,

    /// Ask mode.
    #[serde(default)]
    pub ask: AskMode,

    /// Command allowlist patterns.
    #[serde(default)]
    pub allowlist: Vec<String>,

    /// Safe binaries (always allowed).
    #[serde(default)]
    pub safe_bins: Vec<String>,

    /// Approval timeout in seconds.
    #[serde(default = "default_approval_timeout")]
    pub approval_timeout_secs: u64,

    /// Fallback behavior when approval fails.
    #[serde(default)]
    pub ask_fallback: AskFallback,
}

fn default_approval_timeout() -> u64 {
    120
}

/// Execution mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecMode {
    /// All execution blocked.
    #[default]
    Deny,

    /// Only allowlisted commands.
    Allowlist,

    /// All execution allowed.
    Full,
}

/// When to ask for approval.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskMode {
    /// Never ask.
    Off,

    /// Ask when allowlist check fails.
    #[default]
    OnMiss,

    /// Always ask.
    Always,
}

/// Fallback when approval request fails.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AskFallback {
    /// Deny execution.
    #[default]
    Deny,

    /// Allow execution.
    Allow,
}

/// Blocked environment variables for security.
pub const BLOCKED_ENV_VARS: &[&str] = &[
    // Dynamic linker injection
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "DYLD_INSERT_LIBRARIES",
    "DYLD_LIBRARY_PATH",
    // Runtime injection
    "NODE_OPTIONS",
    "NODE_PATH",
    "PYTHONPATH",
    "PYTHONHOME",
    "RUBYLIB",
    "PERL5LIB",
    // Shell injection
    "BASH_ENV",
    "ENV",
    "IFS",
    // Other dangerous
    "GCONV_PATH",
    "SSLKEYLOGFILE",
];

/// Blocked environment variable prefixes.
pub const BLOCKED_ENV_PREFIXES: &[&str] = &["DYLD_", "LD_"];

/// Check if an environment variable name is blocked.
pub fn is_env_var_blocked(name: &str) -> bool {
    if BLOCKED_ENV_VARS.contains(&name) {
        return true;
    }
    BLOCKED_ENV_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_context_admin() {
        let ctx = AuthContext::admin("test-client");
        assert_eq!(ctx.client_id, "test-client");
        assert!(ctx.scopes.contains(&Scope::Admin));
        assert!(ctx.scopes.contains(&Scope::Read));
        assert!(ctx.scopes.contains(&Scope::Write));
        assert!(ctx.scopes.contains(&Scope::Approvals));
        assert!(ctx.scopes.contains(&Scope::Pairing));
        assert!(ctx.identity.is_none());
    }

    #[test]
    fn test_auth_context_loopback() {
        let ctx = AuthContext::loopback();
        assert_eq!(ctx.client_id, "loopback");
        assert!(ctx.has_scope(Scope::Admin));
    }

    #[test]
    fn test_auth_context_has_scope_admin_grants_all() {
        let ctx = AuthContext::admin("admin-user");
        // Admin scope implicitly grants access to all scopes.
        assert!(ctx.has_scope(Scope::Read));
        assert!(ctx.has_scope(Scope::Write));
        assert!(ctx.has_scope(Scope::Approvals));
        assert!(ctx.has_scope(Scope::Pairing));
    }

    #[test]
    fn test_auth_context_has_scope_limited() {
        let ctx = AuthContext {
            client_id: "limited-user".to_string(),
            scopes: [Scope::Read].into_iter().collect(),
            identity: None,
            authenticated_at: chrono::Utc::now(),
        };
        assert!(ctx.has_scope(Scope::Read));
        assert!(!ctx.has_scope(Scope::Write));
        assert!(!ctx.has_scope(Scope::Admin));
    }

    #[test]
    fn test_auth_context_has_all_scopes() {
        let ctx = AuthContext::admin("admin");
        assert!(ctx.has_all_scopes(&[Scope::Read, Scope::Write, Scope::Approvals]));

        let limited = AuthContext {
            client_id: "user".to_string(),
            scopes: [Scope::Read].into_iter().collect(),
            identity: None,
            authenticated_at: chrono::Utc::now(),
        };
        assert!(!limited.has_all_scopes(&[Scope::Read, Scope::Write]));
    }

    #[test]
    fn test_scope_all() {
        let all = Scope::all();
        assert_eq!(all.len(), 5);
        assert!(all.contains(&Scope::Admin));
        assert!(all.contains(&Scope::Read));
        assert!(all.contains(&Scope::Write));
        assert!(all.contains(&Scope::Approvals));
        assert!(all.contains(&Scope::Pairing));
    }

    #[test]
    fn test_approval_response_is_approved() {
        assert!(ApprovalResponse::Approved.is_approved());
        assert!(!ApprovalResponse::Denied.is_approved());
        assert!(!ApprovalResponse::Timeout.is_approved());
    }

    #[test]
    fn test_approval_response_serde_roundtrip() {
        let responses = [
            ApprovalResponse::Approved,
            ApprovalResponse::Denied,
            ApprovalResponse::Timeout,
        ];
        for resp in &responses {
            let json = serde_json::to_string(resp).unwrap();
            let parsed: ApprovalResponse = serde_json::from_str(&json).unwrap();
            assert_eq!(*resp, parsed);
        }
    }

    #[test]
    fn test_is_env_var_blocked_exact_match() {
        assert!(is_env_var_blocked("LD_PRELOAD"));
        assert!(is_env_var_blocked("DYLD_INSERT_LIBRARIES"));
        assert!(is_env_var_blocked("NODE_OPTIONS"));
        assert!(is_env_var_blocked("BASH_ENV"));
        assert!(is_env_var_blocked("SSLKEYLOGFILE"));
    }

    #[test]
    fn test_is_env_var_blocked_prefix_match() {
        // DYLD_ prefix matches any var starting with it.
        assert!(is_env_var_blocked("DYLD_ANYTHING"));
        assert!(is_env_var_blocked("DYLD_FRAMEWORK_PATH"));
        // LD_ prefix matches.
        assert!(is_env_var_blocked("LD_SOMETHING"));
    }

    #[test]
    fn test_is_env_var_not_blocked() {
        assert!(!is_env_var_blocked("HOME"));
        assert!(!is_env_var_blocked("PATH"));
        assert!(!is_env_var_blocked("USER"));
        assert!(!is_env_var_blocked("MY_CUSTOM_VAR"));
    }
}
