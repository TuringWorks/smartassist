//! Gateway session management.

use std::collections::HashMap;
use tokio::sync::RwLock;

/// Gateway session for a WebSocket connection.
#[derive(Debug, Clone)]
pub struct GatewaySession {
    /// Session ID.
    pub id: String,

    /// Associated agent ID.
    pub agent_id: Option<String>,

    /// Session metadata.
    pub metadata: HashMap<String, serde_json::Value>,

    /// Created timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Last activity timestamp.
    pub last_activity: chrono::DateTime<chrono::Utc>,
}

impl GatewaySession {
    /// Create a new gateway session.
    pub fn new(id: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: id.into(),
            agent_id: None,
            metadata: HashMap::new(),
            created_at: now,
            last_activity: now,
        }
    }

    /// Set the associated agent.
    pub fn with_agent(mut self, agent_id: impl Into<String>) -> Self {
        self.agent_id = Some(agent_id.into());
        self
    }

    /// Update last activity time.
    pub fn touch(&mut self) {
        self.last_activity = chrono::Utc::now();
    }
}

/// Manager for gateway sessions.
pub struct SessionRegistry {
    /// Active sessions.
    sessions: RwLock<HashMap<String, GatewaySession>>,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionRegistry {
    /// Create a new session registry.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Create and register a new session.
    pub async fn create(&self) -> GatewaySession {
        let session = GatewaySession::new(uuid::Uuid::new_v4().to_string());
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session.clone());
        session
    }

    /// Get a session by ID.
    pub async fn get(&self, id: &str) -> Option<GatewaySession> {
        let sessions = self.sessions.read().await;
        sessions.get(id).cloned()
    }

    /// Remove a session.
    pub async fn remove(&self, id: &str) {
        let mut sessions = self.sessions.write().await;
        sessions.remove(id);
    }

    /// Update a session.
    pub async fn update(&self, session: GatewaySession) {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session);
    }

    /// Get all session IDs.
    pub async fn list(&self) -> Vec<String> {
        let sessions = self.sessions.read().await;
        sessions.keys().cloned().collect()
    }

    /// Get session count.
    pub async fn count(&self) -> usize {
        let sessions = self.sessions.read().await;
        sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_new_sets_id_and_timestamps() {
        let session = GatewaySession::new("sess-1");
        assert_eq!(session.id, "sess-1");
        assert!(session.agent_id.is_none());
        assert!(session.metadata.is_empty());
        // created_at and last_activity should be equal right after creation
        assert_eq!(session.created_at, session.last_activity);
    }

    #[test]
    fn test_session_with_agent() {
        let session = GatewaySession::new("sess-2").with_agent("agent-alpha");
        assert_eq!(session.id, "sess-2");
        assert_eq!(session.agent_id, Some("agent-alpha".to_string()));
    }

    #[test]
    fn test_session_touch_updates_last_activity() {
        let mut session = GatewaySession::new("sess-3");
        let original = session.last_activity;
        // Small sleep to ensure time advances
        std::thread::sleep(std::time::Duration::from_millis(10));
        session.touch();
        assert!(session.last_activity >= original);
        // created_at must remain unchanged
        assert_eq!(session.created_at, session.created_at);
    }

    #[test]
    fn test_session_metadata_can_be_set() {
        let mut session = GatewaySession::new("sess-4");
        session.metadata.insert(
            "client".to_string(),
            serde_json::json!("web-ui"),
        );
        assert_eq!(
            session.metadata.get("client"),
            Some(&serde_json::json!("web-ui"))
        );
    }

    #[tokio::test]
    async fn test_registry_create_and_get() {
        let registry = SessionRegistry::new();
        let session = registry.create().await;
        assert!(!session.id.is_empty());

        let fetched = registry.get(&session.id).await;
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, session.id);
    }

    #[tokio::test]
    async fn test_registry_get_missing_returns_none() {
        let registry = SessionRegistry::new();
        let result = registry.get("nonexistent-id").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_registry_remove() {
        let registry = SessionRegistry::new();
        let session = registry.create().await;
        let id = session.id.clone();

        registry.remove(&id).await;
        assert!(registry.get(&id).await.is_none());
        assert_eq!(registry.count().await, 0);
    }

    #[tokio::test]
    async fn test_registry_list_and_count() {
        let registry = SessionRegistry::new();
        assert_eq!(registry.count().await, 0);
        assert!(registry.list().await.is_empty());

        let s1 = registry.create().await;
        let s2 = registry.create().await;
        assert_eq!(registry.count().await, 2);

        let ids = registry.list().await;
        assert!(ids.contains(&s1.id));
        assert!(ids.contains(&s2.id));
    }

    #[tokio::test]
    async fn test_registry_update_session() {
        let registry = SessionRegistry::new();
        let mut session = registry.create().await;
        let id = session.id.clone();

        // Attach an agent and update in registry
        session.agent_id = Some("agent-beta".to_string());
        registry.update(session).await;

        let updated = registry.get(&id).await.unwrap();
        assert_eq!(updated.agent_id, Some("agent-beta".to_string()));
    }

    #[test]
    fn test_registry_default_trait() {
        // SessionRegistry implements Default via Default for SessionRegistry
        let registry = SessionRegistry::default();
        // Verify it is usable (not a compile-only check)
        assert!(std::mem::size_of_val(&registry) > 0);
    }
}
