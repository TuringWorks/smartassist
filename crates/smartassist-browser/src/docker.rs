//! Docker-isolated browser backend.
//!
//! Spins up a headless Chrome inside a Docker container with
//! configurable sandboxing, network isolation, and VNC access.

use super::{BrowserBackend, BrowserAction, BrowserActionResult, BrowserError, BrowserSession, LaunchOptions};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Docker-backed browser backend.
pub struct DockerBrowserBackend {
    sessions: RwLock<HashMap<String, BrowserSession>>,
}

impl DockerBrowserBackend {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl BrowserBackend for DockerBrowserBackend {
    async fn launch(&self, options: LaunchOptions) -> Result<String, BrowserError> {
        // TODO: Start Docker container with Chrome + VNC
        let id = uuid::Uuid::new_v4().to_string();
        let session = BrowserSession {
            id: id.clone(),
            url: None,
            viewport: options.viewport,
            headless: options.headless,
            sandboxed: options.sandboxed,
            created_at: chrono::Utc::now(),
        };
        self.sessions.write().await.insert(id.clone(), session);
        tracing::info!("Docker browser session {} started", id);
        Ok(id)
    }

    async fn close(&self, session_id: &str) -> Result<(), BrowserError> {
        // TODO: Stop Docker container
        self.sessions.write().await.remove(session_id);
        tracing::info!("Docker browser session {} stopped", session_id);
        Ok(())
    }

    async fn execute(
        &self,
        session_id: &str,
        action: BrowserAction,
    ) -> Result<BrowserActionResult, BrowserError> {
        let _ = session_id;
        let _ = action;
        // TODO: Proxy command to container via HTTP/WebSocket
        Err(BrowserError::NotInitialized)
    }

    async fn list_sessions(&self) -> Result<Vec<BrowserSession>, BrowserError> {
        Ok(self.sessions.read().await.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_docker_backend_launch() {
        let backend = DockerBrowserBackend::new();
        let id = backend
            .launch(LaunchOptions::default())
            .await
            .expect("launch should succeed");
        assert!(!id.is_empty());

        let sessions = backend.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_docker_backend_execute_returns_not_initialized() {
        let backend = DockerBrowserBackend::new();
        let id = backend.launch(LaunchOptions::default()).await.unwrap();
        let result = backend
            .execute(
                &id,
                BrowserAction::Navigate {
                    url: "https://example.com".to_string(),
                },
            )
            .await;
        assert!(matches!(result, Err(BrowserError::NotInitialized)));
    }
}
